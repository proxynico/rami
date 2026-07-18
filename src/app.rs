use crate::cpu::CpuSampler;
use crate::diagnostics::{build_diagnostic_report, current_report_input};
use crate::gpu::GpuSampler;
use crate::lock::AppLock;
use crate::login_item::{LaunchAtLoginController, LaunchAtLoginStatus};
use crate::memory::MemorySampler;
use crate::model::{CpuModuleState, GpuModuleState};
use crate::process_cpu::{
    ProcessCpuSampler, ProcessCpuSnapshot, ProcessCpuUsage, PROCESS_CPU_ROW_LIMIT,
};
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage, ProcessMemorySampler};
use crate::settings::SettingsStore;
use crate::tray::TrayController;
use crate::trend::{app_rows_with_deltas, MemoryTrendTracker};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuDelegate, NSPasteboard,
    NSPasteboardTypeString,
};
use objc2_foundation::{
    NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes, NSString, NSTimer,
};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::{Rc, Weak};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

thread_local! {
    static APP_STATE: RefCell<Option<Weak<AppState>>> = const { RefCell::new(None) };
}

struct AppState {
    tray: TrayController,
    sampler: MemorySampler,
    cpu_sampler: RefCell<CpuSampler>,
    gpu_sampler: GpuSampler,
    app_scan_sender: Sender<AppScanResult>,
    app_scan_receiver: Receiver<AppScanResult>,
    app_scan_in_flight: Cell<bool>,
    app_scan_generation: Cell<u64>,
    cpu_process_scan_sender: Sender<CpuProcessScanResult>,
    cpu_process_scan_receiver: Receiver<CpuProcessScanResult>,
    cpu_process_scan_in_flight: Cell<bool>,
    cpu_process_scan_generation: Cell<u64>,
    refresh_target: Retained<AnyObject>,
    launch_at_login: LaunchAtLoginController,
    launch_at_login_status: Cell<LaunchAtLoginStatus>,
    auto_refresh_enabled: Cell<bool>,
    show_app_usage: Cell<bool>,
    show_cpu: Cell<bool>,
    show_gpu: Cell<bool>,
    settings: SettingsStore,
    app_memory: RefCell<AppMemorySnapshot>,
    cpu_processes: RefCell<ProcessCpuSnapshot>,
    last_snapshot: RefCell<Option<crate::model::MemorySnapshot>>,
    last_app_rows: RefCell<Vec<AppMemoryUsage>>,
    trend_tracker: RefCell<MemoryTrendTracker>,
    last_app_sample_at: Cell<Option<Instant>>,
    ticks_until_app_refresh: Cell<u8>,
    menu_open: Cell<bool>,
    /// Last sampled CPU/GPU module state, so a drain can re-render the menu
    /// without resampling. Both are `Copy`.
    last_cpu_state: Cell<CpuModuleState>,
    last_gpu_state: Cell<GpuModuleState>,
    /// The single in-flight menu-open drain timer. Held so a new drain can
    /// cancel the previous one instead of stacking overlapping timers.
    menu_open_drain_timer: RefCell<Option<Retained<NSTimer>>>,
}

const APP_REFRESH_INTERVAL_TICKS: u8 = 6;
const APP_DELTA_BASELINE_MAX_AGE: Duration = Duration::from_secs(90);
const APP_BASELINE_ROW_LIMIT: usize = 25;
const MENU_REOPEN_DELAY_SECONDS: f64 = 0.05;
const MENU_OPEN_DRAIN_DELAY_SECONDS: f64 = 0.15;
const CPU_PROCESS_DRAIN_DELAY_SECONDS: f64 = 0.3;

struct AppScanResult {
    generation: u64,
    completed_at: Instant,
    rows: io::Result<Vec<AppMemoryUsage>>,
}

struct CpuProcessScanResult {
    generation: u64,
    rows: io::Result<Vec<ProcessCpuUsage>>,
}

fn previous_app_rows_if_fresh(
    last_sample_at: Option<Instant>,
    now: Instant,
    rows: &[AppMemoryUsage],
) -> Vec<AppMemoryUsage> {
    if last_sample_at
        .map(|sampled_at| now.duration_since(sampled_at) <= APP_DELTA_BASELINE_MAX_AGE)
        .unwrap_or(false)
    {
        rows.to_vec()
    } else {
        Vec::new()
    }
}

/// Decide whether this tick starts an app scan and what the countdown becomes.
/// Scans only run while the menu is open; while it is closed the countdown is
/// frozen so an idle app does no dropdown work at all.
fn app_scan_decision(menu_open: bool, manual: bool, ticks_until_refresh: u8) -> (bool, u8) {
    if !menu_open {
        return (false, ticks_until_refresh);
    }
    if manual || ticks_until_refresh == 0 {
        (true, APP_REFRESH_INTERVAL_TICKS.saturating_sub(1))
    } else {
        (false, ticks_until_refresh - 1)
    }
}

fn should_sample_cpu(menu_open: bool, show_cpu: bool) -> bool {
    menu_open && show_cpu
}

fn should_sample_gpu(menu_open: bool, show_gpu: bool) -> bool {
    menu_open && show_gpu
}

fn should_accept_cpu_process_result(
    result_generation: u64,
    current_generation: u64,
    menu_open: bool,
    show_cpu: bool,
) -> bool {
    result_generation == current_generation && menu_open && show_cpu
}

fn should_schedule_cpu_process_drain(
    show_cpu: bool,
    result_updated: bool,
    scan_in_flight: bool,
) -> bool {
    show_cpu && !result_updated && scan_in_flight
}

/// An empty row set is a complete answer, not a failed scan.
///
/// `ProcessCpuSampler::sample` takes its own before/after readings inside one
/// call, so it never needs a warm-up round: "no rows" means nothing crossed the
/// 0% threshold during its 200 ms window. Reporting that as "not updated" made
/// `refresh` start another scan *and* schedule another drain, which re-entered
/// this path roughly three times a second for as long as the menu stayed open —
/// two full process-table sweeps per cycle.
///
/// This previously kept the last rows on screen to avoid flicker. It no longer
/// does: rami is a monitor, and showing a busy process list for a machine that
/// has since gone idle is worse than showing none.
fn merge_cpu_process_rows(rows: Vec<ProcessCpuUsage>) -> (ProcessCpuSnapshot, bool) {
    (ProcessCpuSnapshot::Loaded(rows), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_app_rows_are_suppressed_when_stale() {
        let now = Instant::now();
        let rows = vec![usage("Zen")];

        assert_eq!(
            previous_app_rows_if_fresh(Some(now - Duration::from_secs(30)), now, &rows),
            rows
        );
        assert!(previous_app_rows_if_fresh(
            Some(now - APP_DELTA_BASELINE_MAX_AGE - Duration::from_secs(1)),
            now,
            &rows
        )
        .is_empty());
        assert!(previous_app_rows_if_fresh(None, now, &rows).is_empty());
    }

    #[test]
    fn app_scans_are_gated_on_menu_visibility() {
        // Menu closed: never scan, never advance the countdown — even manually.
        assert_eq!(app_scan_decision(false, false, 3), (false, 3));
        assert_eq!(app_scan_decision(false, true, 0), (false, 0));

        // Menu open: manual or an expired countdown scans and resets the cadence.
        assert_eq!(
            app_scan_decision(true, true, 3),
            (true, APP_REFRESH_INTERVAL_TICKS - 1)
        );
        assert_eq!(
            app_scan_decision(true, false, 0),
            (true, APP_REFRESH_INTERVAL_TICKS - 1)
        );
        assert_eq!(app_scan_decision(true, false, 2), (false, 1));
    }

    #[test]
    fn cpu_sampling_is_gated_by_visibility_and_its_own_setting() {
        assert!(!should_sample_cpu(false, true));
        assert!(!should_sample_cpu(true, false));
        assert!(should_sample_cpu(true, true));
    }

    #[test]
    fn gpu_sampling_is_gated_by_visibility_and_its_own_setting() {
        assert!(!should_sample_gpu(false, true));
        assert!(!should_sample_gpu(true, false));
        assert!(should_sample_gpu(true, true));
    }

    #[test]
    fn cpu_process_results_require_the_current_visible_generation() {
        assert!(should_accept_cpu_process_result(7, 7, true, true));
        assert!(!should_accept_cpu_process_result(6, 7, true, true));
        assert!(!should_accept_cpu_process_result(7, 7, false, true));
        assert!(!should_accept_cpu_process_result(7, 7, true, false));
    }

    #[test]
    fn slow_cpu_process_scans_keep_polling_until_the_result_is_drained() {
        assert!(should_schedule_cpu_process_drain(true, false, true));
        assert!(!should_schedule_cpu_process_drain(true, true, false));
        assert!(!should_schedule_cpu_process_drain(false, false, true));
    }

    #[test]
    fn empty_cpu_process_sample_is_a_real_answer_and_does_not_retry() {
        // Regression: reporting an empty scan as "not updated" made refresh()
        // start another scan and schedule another drain, looping at ~3 Hz for as
        // long as the menu stayed open. An empty result is a complete answer.
        let (next, updated) = merge_cpu_process_rows(Vec::new());

        assert_eq!(next, ProcessCpuSnapshot::Loaded(Vec::new()));
        assert!(updated);
        // The drain is only rescheduled while a result is still outstanding.
        assert!(!should_schedule_cpu_process_drain(true, updated, true));
    }

    #[test]
    fn populated_cpu_process_sample_replaces_the_rows() {
        let rows = vec![ProcessCpuUsage {
            name: "Editor".to_string(),
            utilization_percent: 12,
        }];

        let (next, updated) = merge_cpu_process_rows(rows.clone());

        assert_eq!(next, ProcessCpuSnapshot::Loaded(rows));
        assert!(updated);
    }

    fn usage(name: &str) -> AppMemoryUsage {
        AppMemoryUsage {
            name: name.to_string(),
            group_key: format!("/Applications/{name}.app"),
            footprint_bytes: 1,
            pids: vec![1],
            can_quit: true,
            delta_bytes: None,
        }
    }
}

impl AppState {
    fn refresh(&self, manual: bool) {
        if !manual && !self.auto_refresh_enabled.get() {
            return;
        }
        self.sync_settings_menu();
        let mtm = MainThreadMarker::new().expect("refreshes must stay on the main thread");
        self.drain_app_scan_results();
        let cpu_processes_updated = self.drain_cpu_process_scan_results();
        match self.sampler.sample() {
            Ok(snapshot) => {
                *self.last_snapshot.borrow_mut() = Some(snapshot);
                let trend = self.trend_tracker.borrow_mut().record(snapshot.used_bytes);
                let app_sampling_enabled = self.show_app_usage.get();
                if app_sampling_enabled {
                    let (should_scan, next_ticks) = app_scan_decision(
                        self.menu_open.get(),
                        manual,
                        self.ticks_until_app_refresh.get(),
                    );
                    if should_scan {
                        self.start_app_scan();
                    }
                    self.ticks_until_app_refresh.set(next_ticks);
                } else {
                    self.clear_app_usage();
                }

                self.tray.set_gauge_snapshot(snapshot, trend, mtm);
                if self.menu_open.get() {
                    let cpu = self.sample_cpu_if_visible();
                    self.last_cpu_state.set(cpu);
                    if self.show_cpu.get() && !cpu_processes_updated {
                        self.start_cpu_process_scan();
                    }
                    if should_schedule_cpu_process_drain(
                        self.show_cpu.get(),
                        cpu_processes_updated,
                        self.cpu_process_scan_in_flight.get(),
                    ) {
                        self.schedule_menu_open_drain(CPU_PROCESS_DRAIN_DELAY_SECONDS);
                    }
                    let gpu = self.sample_gpu_if_visible();
                    self.last_gpu_state.set(gpu);
                    let apps = self.app_memory.borrow();
                    let cpu_processes = self.cpu_processes.borrow();
                    let history = self.trend_tracker.borrow().samples();
                    self.tray.set_menu_snapshot(
                        snapshot,
                        cpu,
                        gpu,
                        &apps,
                        &cpu_processes,
                        &history,
                        self.launch_at_login_status.get(),
                        self.auto_refresh_enabled.get(),
                        mtm,
                    );
                }
            }
            Err(err) => {
                eprintln!("memory sample failed: {err}");
                self.tray
                    .set_placeholder(self.launch_at_login_status.get(), mtm);
            }
        }
    }

    /// Pick up async scan results and re-render the dropdown from the values the
    /// last real refresh sampled.
    ///
    /// This deliberately does NOT resample, record a trend sample, or advance any
    /// cadence counter. It runs on a 150–300 ms one-shot timer, and routing it
    /// through the full `refresh` path meant every drain aged the tick-counted
    /// cadences: the 125 s trend window collapsed to about 7 s and the 30 s
    /// app/swap cadences to under 2 s whenever the menu was open. Those counters
    /// are meant to track wall-clock time, and a drain represents no elapsed time.
    fn drain_and_rerender(&self) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if !self.menu_open.get() {
            return;
        }

        self.drain_app_scan_results();
        let cpu_processes_updated = self.drain_cpu_process_scan_results();

        if should_schedule_cpu_process_drain(
            self.show_cpu.get(),
            cpu_processes_updated,
            self.cpu_process_scan_in_flight.get(),
        ) {
            self.schedule_menu_open_drain(CPU_PROCESS_DRAIN_DELAY_SECONDS);
        }

        let Some(snapshot) = *self.last_snapshot.borrow() else {
            return;
        };
        let apps = self.app_memory.borrow();
        let cpu_processes = self.cpu_processes.borrow();
        let history = self.trend_tracker.borrow().samples();
        self.tray.set_menu_snapshot(
            snapshot,
            self.last_cpu_state.get(),
            self.last_gpu_state.get(),
            &apps,
            &cpu_processes,
            &history,
            self.launch_at_login_status.get(),
            self.auto_refresh_enabled.get(),
            mtm,
        );
    }

    fn sync_settings_menu(&self) {
        self.tray.set_settings_state(
            self.launch_at_login_status.get(),
            self.auto_refresh_enabled.get(),
            self.show_app_usage.get(),
            self.show_cpu.get(),
            self.show_gpu.get(),
        );
    }

    fn sample_cpu_if_visible(&self) -> CpuModuleState {
        if !should_sample_cpu(self.menu_open.get(), self.show_cpu.get()) {
            return if self.show_cpu.get() {
                CpuModuleState::Loading
            } else {
                CpuModuleState::Disabled
            };
        }

        let next = match self.cpu_sampler.borrow_mut().sample() {
            Ok(Some(snapshot)) => CpuModuleState::Available(snapshot),
            Ok(None) => CpuModuleState::Loading,
            Err(error) => {
                eprintln!("CPU sample failed: {error}");
                CpuModuleState::Unavailable
            }
        };
        next
    }

    fn sample_gpu_if_visible(&self) -> GpuModuleState {
        if !should_sample_gpu(self.menu_open.get(), self.show_gpu.get()) {
            return if self.show_gpu.get() {
                GpuModuleState::Unavailable
            } else {
                GpuModuleState::Disabled
            };
        }

        match self.gpu_sampler.sample() {
            Ok(Some(snapshot)) => GpuModuleState::Available(snapshot),
            Ok(None) => GpuModuleState::Unavailable,
            Err(error) => {
                eprintln!("GPU sample failed: {error}");
                GpuModuleState::Unavailable
            }
        }
    }

    fn start_app_scan(&self) {
        if self.app_scan_in_flight.replace(true) {
            return;
        }
        let sender = self.app_scan_sender.clone();
        let generation = self.app_scan_generation.get();
        thread::spawn(move || {
            // Background scan should prefer efficiency cores.
            // SAFETY: plain FFI call on the current thread; the ignored status
            // only affects QoS preference, never memory safety.
            unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0) };
            let rows = ProcessMemorySampler::new().sample(APP_BASELINE_ROW_LIMIT);
            let _ = sender.send(AppScanResult {
                generation,
                completed_at: Instant::now(),
                rows,
            });
        });
    }

    fn start_cpu_process_scan(&self) {
        if self.cpu_process_scan_in_flight.replace(true) {
            return;
        }
        let sender = self.cpu_process_scan_sender.clone();
        let generation = self.cpu_process_scan_generation.get();
        thread::spawn(move || {
            // SAFETY: this only changes the current worker thread's QoS preference.
            unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0) };
            let rows = ProcessCpuSampler::new().sample(PROCESS_CPU_ROW_LIMIT);
            let _ = sender.send(CpuProcessScanResult { generation, rows });
        });
    }

    fn drain_app_scan_results(&self) {
        while let Ok(result) = self.app_scan_receiver.try_recv() {
            if result.generation != self.app_scan_generation.get() {
                continue;
            }
            self.app_scan_in_flight.set(false);
            let next = match result.rows {
                Ok(rows) => {
                    let previous_rows = previous_app_rows_if_fresh(
                        self.last_app_sample_at.get(),
                        result.completed_at,
                        &self.last_app_rows.borrow(),
                    );
                    let ranked = app_rows_with_deltas(rows, &previous_rows);
                    *self.last_app_rows.borrow_mut() = ranked.clone();
                    self.last_app_sample_at.set(Some(result.completed_at));
                    AppMemorySnapshot::Loaded(ranked)
                }
                Err(err) => {
                    eprintln!("process memory scan failed: {err}");
                    self.last_app_rows.borrow_mut().clear();
                    self.last_app_sample_at.set(None);
                    AppMemorySnapshot::Unavailable
                }
            };
            *self.app_memory.borrow_mut() = next;
        }
    }

    fn clear_app_usage(&self) {
        *self.app_memory.borrow_mut() = AppMemorySnapshot::Hidden;
        self.last_app_rows.borrow_mut().clear();
        self.last_app_sample_at.set(None);
        self.app_scan_in_flight.set(false);
        self.app_scan_generation
            .set(self.app_scan_generation.get().wrapping_add(1));
    }

    fn drain_cpu_process_scan_results(&self) -> bool {
        let mut accepted = false;
        while let Ok(result) = self.cpu_process_scan_receiver.try_recv() {
            if !should_accept_cpu_process_result(
                result.generation,
                self.cpu_process_scan_generation.get(),
                self.menu_open.get(),
                self.show_cpu.get(),
            ) {
                continue;
            }
            self.cpu_process_scan_in_flight.set(false);
            let (next, updated) = match result.rows {
                Ok(rows) => merge_cpu_process_rows(rows),
                Err(error) => {
                    eprintln!("process CPU scan failed: {error}");
                    (ProcessCpuSnapshot::Unavailable, true)
                }
            };
            accepted = updated;
            *self.cpu_processes.borrow_mut() = next;
        }
        accepted
    }

    fn prepare_cpu_processes(&self) {
        *self.cpu_processes.borrow_mut() = ProcessCpuSnapshot::Loading;
    }

    fn clear_cpu_processes(&self) {
        *self.cpu_processes.borrow_mut() = ProcessCpuSnapshot::Hidden;
        self.cpu_process_scan_in_flight.set(false);
        self.cpu_process_scan_generation
            .set(self.cpu_process_scan_generation.get().wrapping_add(1));
    }

    fn toggle_launch_at_login(&self) {
        match self.launch_at_login.toggle() {
            Ok(status) => self.launch_at_login_status.set(status),
            Err(err) => {
                eprintln!("failed to toggle launch at login: {err}");
                self.launch_at_login_status
                    .set(self.launch_at_login.status());
            }
        }
        self.refresh(true);
    }

    fn toggle_auto_refresh(&self) {
        let enabled = !self.auto_refresh_enabled.get();
        self.auto_refresh_enabled.set(enabled);
        self.settings.set_auto_refresh_enabled(enabled);
        self.refresh(true);
    }

    fn toggle_show_app_usage(&self) {
        let on = !self.show_app_usage.get();
        self.show_app_usage.set(on);
        self.settings.set_show_app_usage(on);
        if on {
            *self.app_memory.borrow_mut() = AppMemorySnapshot::Loading;
            self.ticks_until_app_refresh.set(0);
        } else {
            self.clear_app_usage();
        }
        self.refresh(true);
        if on {
            self.reopen_menu_soon();
        }
    }

    fn toggle_show_cpu(&self) {
        let enabled = !self.show_cpu.get();
        self.show_cpu.set(enabled);
        self.settings.set_show_cpu(enabled);
        self.cpu_sampler.borrow_mut().reset();
        if enabled && self.menu_open.get() {
            self.prepare_cpu_processes();
        } else {
            self.clear_cpu_processes();
        }
        self.refresh(true);
    }

    fn toggle_show_gpu(&self) {
        let enabled = !self.show_gpu.get();
        self.show_gpu.set(enabled);
        self.settings.set_show_gpu(enabled);
        self.refresh(true);
    }

    fn menu_will_open(&self) {
        self.menu_open.set(true);
        if self.show_cpu.get() {
            self.prepare_cpu_processes();
        }
        // The status read is an XPC round trip; menu open is the only moment
        // the answer is visible, so it is (re)read here rather than per tick.
        self.launch_at_login_status
            .set(self.launch_at_login.status());
        self.refresh(true);
        self.schedule_menu_open_drain(MENU_OPEN_DRAIN_DELAY_SECONDS);
    }

    fn menu_did_close(&self) {
        self.menu_open.set(false);
        self.ticks_until_app_refresh.set(0);
        self.cpu_sampler.borrow_mut().reset();
        self.clear_cpu_processes();
        // Nothing to drain into once the dropdown is gone.
        self.cancel_menu_open_drain();
    }

    /// One-shot timer that drains the scan started by `menuWillOpen:` while the
    /// menu is still tracking. It must live in NSRunLoopCommonModes or it would
    /// only fire after the menu closes.
    /// Only one drain may be in flight. `menuWillOpen:` schedules a 150 ms drain
    /// and the refresh it triggers could schedule a 300 ms one, so without
    /// cancelling, overlapping timers stacked up and each ran independently.
    fn schedule_menu_open_drain(&self, delay_seconds: f64) {
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                delay_seconds,
                &self.refresh_target,
                sel!(drainScanResults:),
                None,
                false,
            )
        };
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
        if let Some(previous) = self
            .menu_open_drain_timer
            .borrow_mut()
            .replace(timer.clone())
        {
            previous.invalidate();
        }
    }

    fn cancel_menu_open_drain(&self) {
        if let Some(timer) = self.menu_open_drain_timer.borrow_mut().take() {
            timer.invalidate();
        }
    }

    fn reopen_menu_soon(&self) {
        let _timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                MENU_REOPEN_DELAY_SECONDS,
                &self.refresh_target,
                sel!(reopenMenu:),
                None,
                false,
            )
        };
    }

    fn open_settings_menu_soon(&self) {
        let _timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                MENU_REOPEN_DELAY_SECONDS,
                &self.refresh_target,
                sel!(openSettingsMenu:),
                None,
                false,
            )
        };
    }

    fn reopen_menu_if_app_usage_visible(&self) {
        if self.show_app_usage.get() {
            self.refresh(true);
            self.tray.pop_up_menu();
        }
    }

    fn copy_diagnostic_report(&self) {
        let apps = self.app_memory.borrow();
        let report = build_diagnostic_report(current_report_input(
            self.launch_at_login_status.get(),
            *self.last_snapshot.borrow(),
            &apps,
        ));
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        let string_type = unsafe { NSPasteboardTypeString };
        if !pasteboard.setString_forType(&NSString::from_str(&report), string_type) {
            eprintln!("failed to copy diagnostics to pasteboard");
        }
    }
}

fn install_app_state(state: &Rc<AppState>) {
    APP_STATE.with(|slot| {
        *slot.borrow_mut() = Some(Rc::downgrade(state));
    });
}

/// Open the latest release page in the user's default browser. Uses `open`
/// rather than NSWorkspace/NSURL to avoid pulling in extra framework
/// features; the repository URL comes from Cargo.toml.
fn open_releases_page() {
    let url = format!("{}/releases/latest", env!("CARGO_PKG_REPOSITORY"));
    if let Err(err) = std::process::Command::new("open").arg(&url).status() {
        eprintln!("failed to open releases page: {err}");
    }
}

fn with_app_state(f: impl FnOnce(&AppState)) {
    if let Some(state) = APP_STATE.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade)) {
        f(&state);
    }
}

fn refresh_current_app() {
    with_app_state(|state| state.refresh(true));
}

fn timer_refresh_current_app() {
    with_app_state(|state| state.refresh(false));
}

fn drain_current_app() {
    with_app_state(|state| state.drain_and_rerender());
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct RefreshTarget;

    impl RefreshTarget {
        #[unsafe(method(refreshNow:))]
        fn refresh_now(&self, _sender: &AnyObject) {
            refresh_current_app();
        }

        #[unsafe(method(refreshOnTimer:))]
        fn refresh_on_timer(&self, _sender: &AnyObject) {
            timer_refresh_current_app();
        }

        #[unsafe(method(drainScanResults:))]
        fn drain_scan_results(&self, _sender: &AnyObject) {
            drain_current_app();
        }

        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, _sender: &AnyObject) {
            with_app_state(|state| state.toggle_launch_at_login());
        }

        #[unsafe(method(toggleAutoRefresh:))]
        fn toggle_auto_refresh(&self, _sender: &AnyObject) {
            with_app_state(|state| state.toggle_auto_refresh());
        }

        #[unsafe(method(toggleShowAppUsage:))]
        fn toggle_show_app_usage(&self, _sender: &AnyObject) {
            with_app_state(|state| state.toggle_show_app_usage());
        }

        #[unsafe(method(toggleShowCpu:))]
        fn toggle_show_cpu(&self, _sender: &AnyObject) {
            with_app_state(|state| state.toggle_show_cpu());
        }

        #[unsafe(method(toggleShowGpu:))]
        fn toggle_show_gpu(&self, _sender: &AnyObject) {
            with_app_state(|state| state.toggle_show_gpu());
        }

        #[unsafe(method(copyDiagnostics:))]
        fn copy_diagnostics(&self, _sender: &AnyObject) {
            with_app_state(|state| state.copy_diagnostic_report());
        }

        #[unsafe(method(checkForUpdates:))]
        fn check_for_updates(&self, _sender: &AnyObject) {
            open_releases_page();
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: &AnyObject) {
            with_app_state(|state| state.open_settings_menu_soon());
        }

        #[unsafe(method(openSettingsMenu:))]
        fn open_settings_menu(&self, _sender: &AnyObject) {
            with_app_state(|state| state.tray.pop_up_settings_menu());
        }

        #[unsafe(method(reopenMenu:))]
        fn reopen_menu(&self, _sender: &AnyObject) {
            with_app_state(|state| state.reopen_menu_if_app_usage_visible());
        }

    }

    unsafe impl NSObjectProtocol for RefreshTarget {}

    unsafe impl NSMenuDelegate for RefreshTarget {
        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self, _menu: &NSMenu) {
            with_app_state(|state| state.menu_will_open());
        }

        #[unsafe(method(menuDidClose:))]
        fn menu_did_close(&self, _menu: &NSMenu) {
            with_app_state(|state| state.menu_did_close());
        }
    }
);

impl RefreshTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        unsafe { msg_send![this, init] }
    }
}

pub struct App {
    app: Retained<NSApplication>,
    _lock: AppLock,
    _state: Rc<AppState>,
    _refresh_target: Retained<AnyObject>,
    _timer: Retained<NSTimer>,
}

impl App {
    pub fn new() -> io::Result<Option<Self>> {
        let Some(lock) = AppLock::acquire()? else {
            return Ok(None);
        };

        let mtm = MainThreadMarker::new().expect("app must start on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        let refresh_target = RefreshTarget::new(mtm);
        let tray = TrayController::new(mtm, refresh_target.clone().into());
        // NSMenu holds its delegate weakly; `App` retains the target for the
        // app lifetime, so the delegate stays valid.
        tray.set_menu_delegate(ProtocolObject::from_ref(&*refresh_target));
        let refresh_target: Retained<AnyObject> = refresh_target.into();
        let launch_at_login = LaunchAtLoginController::new();
        let launch_at_login_status = launch_at_login.status();
        let settings_store = SettingsStore::new();
        let settings = settings_store.load();
        let (app_scan_sender, app_scan_receiver) = mpsc::channel();
        let (cpu_process_scan_sender, cpu_process_scan_receiver) = mpsc::channel();
        let initial_app_memory = if settings.show_app_usage {
            AppMemorySnapshot::Loading
        } else {
            AppMemorySnapshot::Hidden
        };
        let state = Rc::new(AppState {
            tray,
            sampler: MemorySampler::new()?,
            cpu_sampler: RefCell::new(CpuSampler::new()),
            gpu_sampler: GpuSampler::new(),
            app_scan_sender,
            app_scan_receiver,
            app_scan_in_flight: Cell::new(false),
            app_scan_generation: Cell::new(0),
            cpu_process_scan_sender,
            cpu_process_scan_receiver,
            cpu_process_scan_in_flight: Cell::new(false),
            cpu_process_scan_generation: Cell::new(0),
            refresh_target: refresh_target.clone(),
            launch_at_login,
            launch_at_login_status: Cell::new(launch_at_login_status),
            auto_refresh_enabled: Cell::new(settings.auto_refresh_enabled),
            show_app_usage: Cell::new(settings.show_app_usage),
            show_cpu: Cell::new(settings.show_cpu),
            show_gpu: Cell::new(settings.show_gpu),
            settings: settings_store,
            app_memory: RefCell::new(initial_app_memory),
            cpu_processes: RefCell::new(ProcessCpuSnapshot::Hidden),
            last_snapshot: RefCell::new(None),
            last_app_rows: RefCell::new(Vec::new()),
            trend_tracker: RefCell::new(MemoryTrendTracker::new()),
            last_app_sample_at: Cell::new(None),
            ticks_until_app_refresh: Cell::new(0),
            menu_open: Cell::new(false),
            last_cpu_state: Cell::new(CpuModuleState::Loading),
            last_gpu_state: Cell::new(GpuModuleState::Unavailable),
            menu_open_drain_timer: RefCell::new(None),
        });
        install_app_state(&state);
        app.finishLaunching();
        state.refresh(true);
        // NSRunLoopCommonModes so ticks keep firing while the menu is tracking;
        // the default mode would freeze the dropdown whenever it is open.
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                5.0,
                &refresh_target,
                sel!(refreshOnTimer:),
                None,
                true,
            )
        };
        timer.setTolerance(2.0);
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };

        Ok(Some(Self {
            app,
            _lock: lock,
            _state: state,
            _refresh_target: refresh_target,
            _timer: timer,
        }))
    }

    pub fn run(&mut self) {
        self.app.run();
    }
}
