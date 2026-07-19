//! The refresh lifecycle state machine, extracted from the AppKit shell so
//! every transition is assertable under plain `cargo test` (issue #24).
//!
//! Shape: a reducer. The shell translates each Objective-C callback into an
//! [`Event`], calls [`RefreshEngine::step`], and performs the returned
//! [`Effect`]s in order. The engine owns all lifecycle state — menu
//! visibility, cadence counters, scan generations, render caches — and calls
//! out only through three injected ports ([`Samplers`], [`ScanRunner`],
//! [`LaunchAtLogin`]). Timers, rendering, persistence, and the pasteboard are
//! modeled as returned effects, not injected callables, so tests assert on
//! plain data.
//!
//! Interface contract the shell must honor:
//! - Effects are performed strictly in returned order.
//! - `ScheduleDrain` replaces any pending drain timer (at most one exists;
//!   the latest schedule wins). The engine relies on this when a composite
//!   event emits a later drain after a refresh already scheduled one.
//! - No engine borrow may be held while performing effects: `PopUpMenu`
//!   re-enters `menuWillOpen:` synchronously, which dispatches a new event.
//!
//! Time enters as data, not as a clock port: the only wall-clock comparison
//! (the app-delta baseline freshness window) uses scan-completion timestamps
//! carried on [`AppScanResult`], so tests fabricate `Instant`s instead of
//! faking a clock. Every cadence is tick-counted and driven by events.

use crate::login_item::LaunchAtLoginStatus;
use crate::model::{CpuModuleState, CpuSnapshot, GpuModuleState, GpuSnapshot, MemorySnapshot};
use crate::process_cpu::{ProcessCpuSnapshot, ProcessCpuUsage};
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};
use crate::settings::Settings;
use crate::trend::{app_rows_with_deltas, MemoryTrend, MemoryTrendTracker};
use std::io;
use std::time::{Duration, Instant};

pub(crate) const APP_REFRESH_INTERVAL_TICKS: u8 = 6;
const APP_DELTA_BASELINE_MAX_AGE: Duration = Duration::from_secs(90);
const MENU_OPEN_DRAIN_DELAY: Duration = Duration::from_millis(150);
const CPU_PROCESS_DRAIN_DELAY: Duration = Duration::from_millis(300);

/// Everything the outside world can tell the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Event {
    /// App launch, after settings load: first render plus timer reconcile.
    Startup,
    /// A refresh request: `manual` for the Refresh menu item and internal
    /// refreshes, non-manual for the 5 s repeating timer.
    Tick {
        manual: bool,
    },
    /// The one-shot drain timer fired: pick up scan results, re-render from
    /// cache. Never samples, never advances a cadence.
    DrainTimerFired,
    MenuWillOpen,
    MenuDidClose,
    Toggle(Setting),
    /// The post-toggle menu-reopen one-shot fired (Show App Usage flow).
    ReopenMenuTimerFired,
    CopyDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Setting {
    AutoRefresh,
    ShowAppUsage,
    ShowCpu,
    ShowGpu,
    LaunchAtLogin,
}

/// A persisted settings write the shell must apply to its store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingChange {
    AutoRefresh(bool),
    ShowAppUsage(bool),
    ShowCpu(bool),
    ShowGpu(bool),
}

/// Everything the engine can ask the shell to do, in performance order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Effect {
    Render(Render),
    /// Arm the single one-shot drain timer, replacing any pending one.
    ScheduleDrain {
        delay: Duration,
    },
    CancelDrain,
    /// Arm the 5 s repeating refresh timer.
    ArmRefreshTimer,
    CancelRefreshTimer,
    PersistSetting(SettingChange),
    /// Arm the short one-shot that later delivers `ReopenMenuTimerFired`.
    ScheduleMenuReopen,
    PopUpMenu,
    /// Build the diagnostic report from this state and copy it.
    CopyDiagnostics {
        launch_at_login: LaunchAtLoginStatus,
        memory: Option<MemorySnapshot>,
        apps: AppMemorySnapshot,
    },
}

/// One render call on the tray, mirroring `TrayController` 1:1.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Render {
    Gauge {
        snapshot: MemorySnapshot,
        trend: MemoryTrend,
    },
    Menu {
        snapshot: MemorySnapshot,
        cpu: CpuModuleState,
        gpu: GpuModuleState,
        apps: AppMemorySnapshot,
        cpu_processes: ProcessCpuSnapshot,
        launch_at_login: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
    },
    SettingsState {
        launch_at_login: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        show_app_usage: bool,
        show_cpu: bool,
        show_gpu: bool,
    },
    Placeholder {
        launch_at_login: LaunchAtLoginStatus,
    },
}

/// Synchronous kernel reads. Stateful and call-counted: `sample_memory`
/// advances the sampler's internal swap cadence, and the CPU sampler keeps a
/// delta baseline that `reset_cpu` clears — so when the engine calls these is
/// observable behavior, pinned by the tests below.
pub(crate) trait Samplers {
    fn sample_memory(&mut self) -> io::Result<MemorySnapshot>;
    fn sample_cpu(&mut self) -> io::Result<Option<CpuSnapshot>>;
    fn sample_gpu(&mut self) -> io::Result<Option<GpuSnapshot>>;
    fn reset_cpu(&mut self);
}

pub(crate) struct AppScanResult {
    pub(crate) generation: u64,
    pub(crate) completed_at: Instant,
    pub(crate) rows: io::Result<Vec<AppMemoryUsage>>,
}

pub(crate) struct CpuProcessScanResult {
    pub(crate) generation: u64,
    pub(crate) rows: io::Result<Vec<ProcessCpuUsage>>,
}

/// Async process scans. `start_*` is fire-and-forget (the production adapter
/// spawns a utility-QoS worker); `poll_*` is a non-blocking single-result
/// read. The engine decides when to start and polls until empty; results stay
/// queued in the adapter until the engine asks, so a skipped refresh drops
/// nothing.
pub(crate) trait ScanRunner {
    fn start_app_scan(&mut self, generation: u64);
    fn start_cpu_process_scan(&mut self, generation: u64);
    fn poll_app_scan(&mut self) -> Option<AppScanResult>;
    fn poll_cpu_process_scan(&mut self) -> Option<CpuProcessScanResult>;
}

/// Launch-at-login service. `toggle` attempts the flip and returns the
/// resulting status — the production adapter folds an XPC error into a fresh
/// `status()` read, so the engine only ever sees a status.
pub(crate) trait LaunchAtLogin {
    fn status(&mut self) -> LaunchAtLoginStatus;
    fn toggle(&mut self) -> LaunchAtLoginStatus;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoRefreshTimerAction {
    Arm,
    Cancel,
    Keep,
}

fn auto_refresh_timer_action(enabled: bool, timer_active: bool) -> AutoRefreshTimerAction {
    match (enabled, timer_active) {
        (true, false) => AutoRefreshTimerAction::Arm,
        (false, true) => AutoRefreshTimerAction::Cancel,
        _ => AutoRefreshTimerAction::Keep,
    }
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

pub(crate) struct RefreshEngine<S, R, L> {
    samplers: S,
    scans: R,
    launch: L,
    auto_refresh_enabled: bool,
    show_app_usage: bool,
    show_cpu: bool,
    show_gpu: bool,
    menu_open: bool,
    ticks_until_app_refresh: u8,
    app_scan_in_flight: bool,
    app_scan_generation: u64,
    cpu_process_scan_in_flight: bool,
    cpu_process_scan_generation: u64,
    launch_at_login_status: LaunchAtLoginStatus,
    last_snapshot: Option<MemorySnapshot>,
    last_cpu_state: CpuModuleState,
    last_gpu_state: GpuModuleState,
    app_memory: AppMemorySnapshot,
    last_app_rows: Vec<AppMemoryUsage>,
    last_app_sample_at: Option<Instant>,
    cpu_processes: ProcessCpuSnapshot,
    trend_tracker: MemoryTrendTracker,
    /// The engine's belief about the repeating refresh timer, reconciled via
    /// `auto_refresh_timer_action` so arm/cancel effects fire only on change.
    refresh_timer_armed: bool,
}

impl<S: Samplers, R: ScanRunner, L: LaunchAtLogin> RefreshEngine<S, R, L> {
    pub(crate) fn new(samplers: S, scans: R, mut launch: L, settings: Settings) -> Self {
        let launch_at_login_status = launch.status();
        let app_memory = if settings.show_app_usage {
            AppMemorySnapshot::Loading
        } else {
            AppMemorySnapshot::Hidden
        };
        Self {
            samplers,
            scans,
            launch,
            auto_refresh_enabled: settings.auto_refresh_enabled,
            show_app_usage: settings.show_app_usage,
            show_cpu: settings.show_cpu,
            show_gpu: settings.show_gpu,
            menu_open: false,
            ticks_until_app_refresh: 0,
            app_scan_in_flight: false,
            app_scan_generation: 0,
            cpu_process_scan_in_flight: false,
            cpu_process_scan_generation: 0,
            launch_at_login_status,
            last_snapshot: None,
            last_cpu_state: CpuModuleState::Loading,
            last_gpu_state: GpuModuleState::Unavailable,
            app_memory,
            last_app_rows: Vec::new(),
            last_app_sample_at: None,
            cpu_processes: ProcessCpuSnapshot::Hidden,
            trend_tracker: MemoryTrendTracker::new(),
            refresh_timer_armed: false,
        }
    }

    /// The single entry point: feed one event, perform the returned effects
    /// in order.
    pub(crate) fn step(&mut self, event: Event) -> Vec<Effect> {
        let mut out = Vec::new();
        match event {
            Event::Startup => {
                self.refresh(true, &mut out);
                self.sync_auto_refresh_timer(&mut out);
            }
            Event::Tick { manual } => self.refresh(manual, &mut out),
            Event::DrainTimerFired => self.drain_and_rerender(&mut out),
            Event::MenuWillOpen => {
                self.menu_open = true;
                if self.show_cpu {
                    self.cpu_processes = ProcessCpuSnapshot::Loading;
                }
                // The status read is an XPC round trip; menu open is the only
                // moment the answer is visible, so it is (re)read here rather
                // than per tick.
                self.launch_at_login_status = self.launch.status();
                self.refresh(true, &mut out);
                // Emitted after the refresh so this drain replaces any slower
                // CPU-process drain the refresh scheduled: the latest wins.
                out.push(Effect::ScheduleDrain {
                    delay: MENU_OPEN_DRAIN_DELAY,
                });
            }
            Event::MenuDidClose => {
                self.menu_open = false;
                self.ticks_until_app_refresh = 0;
                self.samplers.reset_cpu();
                self.clear_cpu_processes();
                // Nothing to drain into once the dropdown is gone.
                out.push(Effect::CancelDrain);
            }
            Event::Toggle(setting) => self.toggle(setting, &mut out),
            Event::ReopenMenuTimerFired => {
                if self.show_app_usage {
                    self.refresh(true, &mut out);
                    out.push(Effect::PopUpMenu);
                }
            }
            Event::CopyDiagnostics => {
                out.push(Effect::CopyDiagnostics {
                    launch_at_login: self.launch_at_login_status,
                    memory: self.last_snapshot,
                    apps: self.app_memory.clone(),
                });
            }
        }
        out
    }

    fn toggle(&mut self, setting: Setting, out: &mut Vec<Effect>) {
        match setting {
            Setting::AutoRefresh => {
                self.auto_refresh_enabled = !self.auto_refresh_enabled;
                out.push(Effect::PersistSetting(SettingChange::AutoRefresh(
                    self.auto_refresh_enabled,
                )));
                self.sync_auto_refresh_timer(out);
                self.refresh(true, out);
            }
            Setting::ShowAppUsage => {
                self.show_app_usage = !self.show_app_usage;
                out.push(Effect::PersistSetting(SettingChange::ShowAppUsage(
                    self.show_app_usage,
                )));
                if self.show_app_usage {
                    self.app_memory = AppMemorySnapshot::Loading;
                    self.ticks_until_app_refresh = 0;
                } else {
                    self.clear_app_usage();
                }
                self.refresh(true, out);
                if self.show_app_usage {
                    out.push(Effect::ScheduleMenuReopen);
                }
            }
            Setting::ShowCpu => {
                self.show_cpu = !self.show_cpu;
                out.push(Effect::PersistSetting(SettingChange::ShowCpu(
                    self.show_cpu,
                )));
                self.samplers.reset_cpu();
                if self.show_cpu && self.menu_open {
                    self.cpu_processes = ProcessCpuSnapshot::Loading;
                } else {
                    self.clear_cpu_processes();
                }
                self.refresh(true, out);
            }
            Setting::ShowGpu => {
                self.show_gpu = !self.show_gpu;
                out.push(Effect::PersistSetting(SettingChange::ShowGpu(
                    self.show_gpu,
                )));
                self.refresh(true, out);
            }
            Setting::LaunchAtLogin => {
                self.launch_at_login_status = self.launch.toggle();
                self.refresh(true, out);
            }
        }
    }

    fn refresh(&mut self, manual: bool, out: &mut Vec<Effect>) {
        if !manual && !self.auto_refresh_enabled {
            return;
        }
        out.push(Effect::Render(self.settings_state_render()));
        self.drain_app_scan_results();
        let cpu_processes_updated = self.drain_cpu_process_scan_results();
        match self.samplers.sample_memory() {
            Ok(snapshot) => {
                self.last_snapshot = Some(snapshot);
                let trend = self.trend_tracker.record(snapshot.used_bytes);
                if self.show_app_usage {
                    let (should_scan, next_ticks) =
                        app_scan_decision(self.menu_open, manual, self.ticks_until_app_refresh);
                    if should_scan {
                        self.start_app_scan();
                    }
                    self.ticks_until_app_refresh = next_ticks;
                } else {
                    self.clear_app_usage();
                }

                out.push(Effect::Render(Render::Gauge { snapshot, trend }));
                if self.menu_open {
                    let cpu = self.sample_cpu_if_visible();
                    self.last_cpu_state = cpu;
                    if self.show_cpu && !cpu_processes_updated {
                        self.start_cpu_process_scan();
                    }
                    if should_schedule_cpu_process_drain(
                        self.show_cpu,
                        cpu_processes_updated,
                        self.cpu_process_scan_in_flight,
                    ) {
                        out.push(Effect::ScheduleDrain {
                            delay: CPU_PROCESS_DRAIN_DELAY,
                        });
                    }
                    let gpu = self.sample_gpu_if_visible();
                    self.last_gpu_state = gpu;
                    out.push(Effect::Render(self.menu_render(snapshot, cpu, gpu)));
                }
            }
            Err(err) => {
                eprintln!("memory sample failed: {err}");
                out.push(Effect::Render(Render::Placeholder {
                    launch_at_login: self.launch_at_login_status,
                }));
            }
        }
    }

    /// Pick up async scan results and re-render the dropdown from the values
    /// the last real refresh sampled.
    ///
    /// This deliberately does NOT resample, record a trend sample, or advance
    /// any cadence counter. It runs on a 150–300 ms one-shot timer, and
    /// routing it through the full `refresh` path meant every drain aged the
    /// tick-counted cadences: the 125 s trend window collapsed to about 7 s
    /// and the 30 s app/swap cadences to under 2 s whenever the menu was open.
    /// Those counters are meant to track wall-clock time, and a drain
    /// represents no elapsed time.
    fn drain_and_rerender(&mut self, out: &mut Vec<Effect>) {
        if !self.menu_open {
            return;
        }

        self.drain_app_scan_results();
        let cpu_processes_updated = self.drain_cpu_process_scan_results();

        if should_schedule_cpu_process_drain(
            self.show_cpu,
            cpu_processes_updated,
            self.cpu_process_scan_in_flight,
        ) {
            out.push(Effect::ScheduleDrain {
                delay: CPU_PROCESS_DRAIN_DELAY,
            });
        }

        let Some(snapshot) = self.last_snapshot else {
            return;
        };
        out.push(Effect::Render(self.menu_render(
            snapshot,
            self.last_cpu_state,
            self.last_gpu_state,
        )));
    }

    fn sync_auto_refresh_timer(&mut self, out: &mut Vec<Effect>) {
        match auto_refresh_timer_action(self.auto_refresh_enabled, self.refresh_timer_armed) {
            AutoRefreshTimerAction::Arm => {
                self.refresh_timer_armed = true;
                out.push(Effect::ArmRefreshTimer);
            }
            AutoRefreshTimerAction::Cancel => {
                self.refresh_timer_armed = false;
                out.push(Effect::CancelRefreshTimer);
            }
            AutoRefreshTimerAction::Keep => {}
        }
    }

    fn sample_cpu_if_visible(&mut self) -> CpuModuleState {
        if !should_sample_cpu(self.menu_open, self.show_cpu) {
            return if self.show_cpu {
                CpuModuleState::Loading
            } else {
                CpuModuleState::Disabled
            };
        }

        match self.samplers.sample_cpu() {
            Ok(Some(snapshot)) => CpuModuleState::Available(snapshot),
            Ok(None) => CpuModuleState::Loading,
            Err(error) => {
                eprintln!("CPU sample failed: {error}");
                CpuModuleState::Unavailable
            }
        }
    }

    fn sample_gpu_if_visible(&mut self) -> GpuModuleState {
        if !should_sample_gpu(self.menu_open, self.show_gpu) {
            return if self.show_gpu {
                GpuModuleState::Unavailable
            } else {
                GpuModuleState::Disabled
            };
        }

        match self.samplers.sample_gpu() {
            Ok(Some(snapshot)) => GpuModuleState::Available(snapshot),
            Ok(None) => GpuModuleState::Unavailable,
            Err(error) => {
                eprintln!("GPU sample failed: {error}");
                GpuModuleState::Unavailable
            }
        }
    }

    fn start_app_scan(&mut self) {
        if self.app_scan_in_flight {
            return;
        }
        self.app_scan_in_flight = true;
        self.scans.start_app_scan(self.app_scan_generation);
    }

    fn start_cpu_process_scan(&mut self) {
        if self.cpu_process_scan_in_flight {
            return;
        }
        self.cpu_process_scan_in_flight = true;
        self.scans
            .start_cpu_process_scan(self.cpu_process_scan_generation);
    }

    fn drain_app_scan_results(&mut self) {
        while let Some(result) = self.scans.poll_app_scan() {
            if result.generation != self.app_scan_generation {
                continue;
            }
            self.app_scan_in_flight = false;
            self.app_memory = match result.rows {
                Ok(rows) => {
                    let previous_rows = previous_app_rows_if_fresh(
                        self.last_app_sample_at,
                        result.completed_at,
                        &self.last_app_rows,
                    );
                    let ranked = app_rows_with_deltas(rows, &previous_rows);
                    self.last_app_rows = ranked.clone();
                    self.last_app_sample_at = Some(result.completed_at);
                    AppMemorySnapshot::Loaded(ranked)
                }
                Err(err) => {
                    eprintln!("process memory scan failed: {err}");
                    self.last_app_rows.clear();
                    self.last_app_sample_at = None;
                    AppMemorySnapshot::Unavailable
                }
            };
        }
    }

    fn drain_cpu_process_scan_results(&mut self) -> bool {
        let mut accepted = false;
        while let Some(result) = self.scans.poll_cpu_process_scan() {
            if !should_accept_cpu_process_result(
                result.generation,
                self.cpu_process_scan_generation,
                self.menu_open,
                self.show_cpu,
            ) {
                continue;
            }
            self.cpu_process_scan_in_flight = false;
            let (next, updated) = match result.rows {
                Ok(rows) => merge_cpu_process_rows(rows),
                Err(error) => {
                    eprintln!("process CPU scan failed: {error}");
                    (ProcessCpuSnapshot::Unavailable, true)
                }
            };
            accepted = updated;
            self.cpu_processes = next;
        }
        accepted
    }

    fn clear_app_usage(&mut self) {
        self.app_memory = AppMemorySnapshot::Hidden;
        self.last_app_rows.clear();
        self.last_app_sample_at = None;
        self.app_scan_in_flight = false;
        self.app_scan_generation = self.app_scan_generation.wrapping_add(1);
    }

    fn clear_cpu_processes(&mut self) {
        self.cpu_processes = ProcessCpuSnapshot::Hidden;
        self.cpu_process_scan_in_flight = false;
        self.cpu_process_scan_generation = self.cpu_process_scan_generation.wrapping_add(1);
    }

    fn settings_state_render(&self) -> Render {
        Render::SettingsState {
            launch_at_login: self.launch_at_login_status,
            auto_refresh_enabled: self.auto_refresh_enabled,
            show_app_usage: self.show_app_usage,
            show_cpu: self.show_cpu,
            show_gpu: self.show_gpu,
        }
    }

    fn menu_render(
        &self,
        snapshot: MemorySnapshot,
        cpu: CpuModuleState,
        gpu: GpuModuleState,
    ) -> Render {
        Render::Menu {
            snapshot,
            cpu,
            gpu,
            apps: self.app_memory.clone(),
            cpu_processes: self.cpu_processes.clone(),
            launch_at_login: self.launch_at_login_status,
            auto_refresh_enabled: self.auto_refresh_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PressureSource;
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;

    // --- pure decision tables -------------------------------------------

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
    fn auto_refresh_timer_tracks_the_setting() {
        assert_eq!(
            auto_refresh_timer_action(true, false),
            AutoRefreshTimerAction::Arm
        );
        assert_eq!(
            auto_refresh_timer_action(true, true),
            AutoRefreshTimerAction::Keep
        );
        assert_eq!(
            auto_refresh_timer_action(false, true),
            AutoRefreshTimerAction::Cancel
        );
        assert_eq!(
            auto_refresh_timer_action(false, false),
            AutoRefreshTimerAction::Keep
        );
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

    // --- fakes -----------------------------------------------------------

    #[derive(Clone, Default)]
    struct SamplerCounts {
        memory: Rc<Cell<usize>>,
        cpu: Rc<Cell<usize>>,
        gpu: Rc<Cell<usize>>,
        cpu_resets: Rc<Cell<usize>>,
    }

    struct FakeSamplers {
        counts: SamplerCounts,
        memory_fails: bool,
        used_bytes: Rc<Cell<u64>>,
    }

    impl FakeSamplers {
        fn healthy() -> (Self, SamplerCounts, Rc<Cell<u64>>) {
            let counts = SamplerCounts::default();
            let used_bytes = Rc::new(Cell::new(1_000_000_000));
            (
                Self {
                    counts: counts.clone(),
                    memory_fails: false,
                    used_bytes: used_bytes.clone(),
                },
                counts,
                used_bytes,
            )
        }

        fn failing() -> (Self, SamplerCounts) {
            let counts = SamplerCounts::default();
            (
                Self {
                    counts: counts.clone(),
                    memory_fails: true,
                    used_bytes: Rc::new(Cell::new(0)),
                },
                counts,
            )
        }
    }

    impl Samplers for FakeSamplers {
        fn sample_memory(&mut self) -> io::Result<MemorySnapshot> {
            self.counts.memory.set(self.counts.memory.get() + 1);
            if self.memory_fails {
                Err(io::Error::other("no host stats"))
            } else {
                Ok(snapshot(self.used_bytes.get()))
            }
        }

        fn sample_cpu(&mut self) -> io::Result<Option<CpuSnapshot>> {
            self.counts.cpu.set(self.counts.cpu.get() + 1);
            Ok(Some(CpuSnapshot {
                user_percent: 10,
                system_percent: 5,
                efficiency_percent: None,
                performance_percent: None,
            }))
        }

        fn sample_gpu(&mut self) -> io::Result<Option<GpuSnapshot>> {
            self.counts.gpu.set(self.counts.gpu.get() + 1);
            Ok(Some(GpuSnapshot {
                utilization_percent: 20,
            }))
        }

        fn reset_cpu(&mut self) {
            self.counts.cpu_resets.set(self.counts.cpu_resets.get() + 1);
        }
    }

    #[derive(Default)]
    struct ScanState {
        app_starts: Vec<u64>,
        cpu_starts: Vec<u64>,
        app_results: VecDeque<AppScanResult>,
        cpu_results: VecDeque<CpuProcessScanResult>,
    }

    #[derive(Clone, Default)]
    struct FakeScans(Rc<RefCell<ScanState>>);

    impl FakeScans {
        fn push_app(&self, generation: u64, completed_at: Instant, rows: Vec<AppMemoryUsage>) {
            self.0.borrow_mut().app_results.push_back(AppScanResult {
                generation,
                completed_at,
                rows: Ok(rows),
            });
        }

        fn push_cpu(&self, generation: u64, rows: Vec<ProcessCpuUsage>) {
            self.0
                .borrow_mut()
                .cpu_results
                .push_back(CpuProcessScanResult {
                    generation,
                    rows: Ok(rows),
                });
        }

        fn app_starts(&self) -> Vec<u64> {
            self.0.borrow().app_starts.clone()
        }

        fn cpu_starts(&self) -> Vec<u64> {
            self.0.borrow().cpu_starts.clone()
        }
    }

    impl ScanRunner for FakeScans {
        fn start_app_scan(&mut self, generation: u64) {
            self.0.borrow_mut().app_starts.push(generation);
        }

        fn start_cpu_process_scan(&mut self, generation: u64) {
            self.0.borrow_mut().cpu_starts.push(generation);
        }

        fn poll_app_scan(&mut self) -> Option<AppScanResult> {
            self.0.borrow_mut().app_results.pop_front()
        }

        fn poll_cpu_process_scan(&mut self) -> Option<CpuProcessScanResult> {
            self.0.borrow_mut().cpu_results.pop_front()
        }
    }

    struct FakeLaunch;

    impl LaunchAtLogin for FakeLaunch {
        fn status(&mut self) -> LaunchAtLoginStatus {
            LaunchAtLoginStatus::Disabled
        }

        fn toggle(&mut self) -> LaunchAtLoginStatus {
            LaunchAtLoginStatus::Enabled
        }
    }

    // --- helpers ---------------------------------------------------------

    type TestEngine = RefreshEngine<FakeSamplers, FakeScans, FakeLaunch>;

    fn engine_on() -> (TestEngine, SamplerCounts, FakeScans) {
        let (samplers, counts, _) = FakeSamplers::healthy();
        let scans = FakeScans::default();
        let engine = RefreshEngine::new(samplers, scans.clone(), FakeLaunch, Settings::default());
        (engine, counts, scans)
    }

    fn snapshot(used_bytes: u64) -> MemorySnapshot {
        MemorySnapshot {
            used_bytes,
            total_bytes: 8_000_000_000,
            used_percent: 50,
            pressure_percent: 20,
            pressure_source: PressureSource::Kernel,
            app_memory_bytes: 0,
            wired_bytes: 0,
            compressed_bytes: 0,
            free_bytes: 0,
            swap_used_bytes: 0,
            available_bytes: 0,
        }
    }

    fn usage(name: &str) -> AppMemoryUsage {
        AppMemoryUsage {
            name: name.to_string(),
            group_key: format!("/Applications/{name}.app"),
            footprint_bytes: 1,
            pids: vec![1],
            delta_bytes: None,
        }
    }

    fn cpu_row(name: &str, utilization_percent: u16) -> ProcessCpuUsage {
        ProcessCpuUsage {
            name: name.to_string(),
            utilization_percent,
        }
    }

    fn scheduled_drains(effects: &[Effect]) -> Vec<Duration> {
        effects
            .iter()
            .filter_map(|e| match e {
                Effect::ScheduleDrain { delay } => Some(*delay),
                _ => None,
            })
            .collect()
    }

    fn menu_renders(effects: &[Effect]) -> usize {
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Render(Render::Menu { .. })))
            .count()
    }

    // --- lifecycle transitions ------------------------------------------

    #[test]
    fn startup_renders_and_arms_the_refresh_timer() {
        let (mut engine, counts, _) = engine_on();

        let effects = engine.step(Event::Startup);

        assert_eq!(counts.memory.get(), 1);
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::Render(Render::Gauge { .. }))));
        assert!(effects.contains(&Effect::ArmRefreshTimer));
        // Menu is closed at startup: no dropdown work.
        assert_eq!(menu_renders(&effects), 0);
        assert_eq!(counts.cpu.get(), 0);
        assert_eq!(counts.gpu.get(), 0);
    }

    #[test]
    fn tick_with_auto_refresh_off_is_a_no_op_but_manual_always_runs() {
        let (mut engine, counts, _) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::Toggle(Setting::AutoRefresh)); // off

        let effects = engine.step(Event::Tick { manual: false });
        assert!(effects.is_empty());
        let sampled_before = counts.memory.get();

        let effects = engine.step(Event::Tick { manual: true });
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::Render(Render::Gauge { .. }))));
        assert_eq!(counts.memory.get(), sampled_before + 1);
    }

    #[test]
    fn toggling_auto_refresh_cancels_and_rearms_the_timer_once() {
        let (mut engine, _, _) = engine_on();
        let effects = engine.step(Event::Startup);
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, Effect::ArmRefreshTimer))
                .count(),
            1
        );

        let effects = engine.step(Event::Toggle(Setting::AutoRefresh));
        assert!(effects.contains(&Effect::CancelRefreshTimer));
        assert!(effects.contains(&Effect::PersistSetting(SettingChange::AutoRefresh(false))));

        let effects = engine.step(Event::Toggle(Setting::AutoRefresh));
        assert!(effects.contains(&Effect::ArmRefreshTimer));
    }

    #[test]
    fn menu_open_samples_modules_and_schedules_the_menu_drain_last() {
        let (mut engine, counts, scans) = engine_on();
        engine.step(Event::Startup);

        let effects = engine.step(Event::MenuWillOpen);

        assert_eq!(counts.memory.get(), 2);
        assert_eq!(counts.cpu.get(), 1);
        assert_eq!(counts.gpu.get(), 1);
        assert_eq!(scans.app_starts(), vec![0]);
        assert_eq!(scans.cpu_starts(), vec![0]);
        assert_eq!(menu_renders(&effects), 1);
        // The 150 ms menu-open drain is emitted last, so it replaces the
        // 300 ms CPU-process drain the refresh scheduled: the latest wins.
        let drains = scheduled_drains(&effects);
        assert_eq!(*drains.last().unwrap(), Duration::from_millis(150));
    }

    #[test]
    fn menu_close_freezes_the_cadence_and_cancels_the_drain() {
        let (mut engine, counts, scans) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::MenuWillOpen);

        let effects = engine.step(Event::MenuDidClose);
        assert!(effects.contains(&Effect::CancelDrain));
        assert_eq!(counts.cpu_resets.get(), 1);

        // Closed-menu ticks sample memory for the gauge but do no dropdown
        // work: no CPU/GPU samples, no scans, no menu renders.
        let before = (counts.cpu.get(), counts.gpu.get(), scans.app_starts().len());
        for _ in 0..10 {
            let effects = engine.step(Event::Tick { manual: false });
            assert_eq!(menu_renders(&effects), 0);
        }
        assert_eq!(
            (counts.cpu.get(), counts.gpu.get(), scans.app_starts().len()),
            before
        );
    }

    // --- the #18 regressions, pinned at the interface --------------------

    #[test]
    fn drains_never_resample_and_never_advance_cadences() {
        let (mut engine, counts, scans) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::MenuWillOpen);
        let memory_before = counts.memory.get();
        let app_starts_before = scans.app_starts().len();

        for _ in 0..20 {
            let effects = engine.step(Event::DrainTimerFired);
            // Re-renders from cache while the scans are still in flight.
            assert_eq!(menu_renders(&effects), 1);
        }

        assert_eq!(counts.memory.get(), memory_before, "a drain resampled");
        assert_eq!(counts.cpu.get(), 1);
        assert_eq!(counts.gpu.get(), 1);
        assert_eq!(scans.app_starts().len(), app_starts_before);
    }

    #[test]
    fn empty_cpu_process_result_is_a_complete_answer_and_does_not_retry() {
        let (mut engine, _, scans) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::MenuWillOpen);
        scans.push_cpu(0, Vec::new());

        let effects = engine.step(Event::DrainTimerFired);

        assert!(scheduled_drains(&effects).is_empty());
        assert_eq!(scans.cpu_starts(), vec![0], "empty result restarted a scan");
        // The empty answer replaces the Loading state and reaches the render.
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::Render(Render::Menu { cpu_processes: ProcessCpuSnapshot::Loaded(rows), .. })
                if rows.is_empty()
        )));

        // Further drains stay quiet; only the next real refresh tick starts
        // a new scan, which is the normal 5 s cadence rather than a retry.
        let effects = engine.step(Event::DrainTimerFired);
        assert!(scheduled_drains(&effects).is_empty());
        assert_eq!(scans.cpu_starts(), vec![0]);
        engine.step(Event::Tick { manual: false });
        assert_eq!(scans.cpu_starts(), vec![0, 0]);
    }

    #[test]
    fn drains_while_a_scan_is_in_flight_reschedule_the_slow_drain() {
        let (mut engine, _, _) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::MenuWillOpen);

        // No result yet: the drain re-arms itself at the CPU-process delay.
        let effects = engine.step(Event::DrainTimerFired);
        assert_eq!(scheduled_drains(&effects), vec![Duration::from_millis(300)]);
    }

    #[test]
    fn stale_generation_results_are_dropped() {
        let (mut engine, _, scans) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::MenuWillOpen);

        // Toggling CPU off and back on bumps the generation past the in-flight
        // scan's tag; its late result must not repopulate the rows.
        engine.step(Event::Toggle(Setting::ShowCpu));
        engine.step(Event::Toggle(Setting::ShowCpu));
        scans.push_cpu(0, vec![cpu_row("Editor", 12)]);

        let effects = engine.step(Event::DrainTimerFired);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::Render(Render::Menu {
                cpu_processes: ProcessCpuSnapshot::Loading,
                ..
            })
        )));
    }

    #[test]
    fn app_scan_cadence_fires_every_six_open_ticks() {
        let (mut engine, _, scans) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::MenuWillOpen); // manual refresh: scan 1, countdown 5
        let now = Instant::now();
        scans.push_app(0, now, vec![usage("Zen")]);
        engine.step(Event::DrainTimerFired);
        assert_eq!(scans.app_starts().len(), 1);

        for _ in 0..5 {
            engine.step(Event::Tick { manual: false });
        }
        assert_eq!(scans.app_starts().len(), 1, "cadence fired early");
        engine.step(Event::Tick { manual: false });
        assert_eq!(scans.app_starts().len(), 2);
    }

    // --- freshness window (fabricated timestamps stand in for a clock) ---

    #[test]
    fn app_deltas_use_the_previous_scan_only_while_fresh() {
        let (mut engine, _, scans) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::MenuWillOpen);
        let base = Instant::now();

        let mut first = usage("Zen");
        first.footprint_bytes = 1_000;
        scans.push_app(0, base, vec![first.clone()]);
        engine.step(Event::DrainTimerFired);

        // A second scan inside the 90 s window computes a delta.
        let mut second = usage("Zen");
        second.footprint_bytes = 3_000;
        engine.step(Event::Tick { manual: true }); // restarts the scan
        scans.push_app(0, base + Duration::from_secs(30), vec![second.clone()]);
        let effects = engine.step(Event::DrainTimerFired);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::Render(Render::Menu { apps: AppMemorySnapshot::Loaded(rows), .. })
                if rows[0].delta_bytes == Some(2_000)
        )));

        // A third scan past the window suppresses the stale baseline.
        engine.step(Event::Tick { manual: true });
        scans.push_app(0, base + Duration::from_secs(200), vec![usage("Zen")]);
        let effects = engine.step(Event::DrainTimerFired);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::Render(Render::Menu { apps: AppMemorySnapshot::Loaded(rows), .. })
                if rows[0].delta_bytes.is_none()
        )));
    }

    // --- error paths -----------------------------------------------------

    #[test]
    fn memory_sample_failure_renders_the_placeholder() {
        let (samplers, _) = FakeSamplers::failing();
        let mut engine = RefreshEngine::new(
            samplers,
            FakeScans::default(),
            FakeLaunch,
            Settings::default(),
        );

        let effects = engine.step(Event::Tick { manual: true });
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::Render(Render::Placeholder { .. }))));
        assert_eq!(menu_renders(&effects), 0);
    }

    // --- toggle flows ----------------------------------------------------

    #[test]
    fn enabling_app_usage_schedules_the_menu_reopen_last() {
        let (mut engine, _, _) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::Toggle(Setting::ShowAppUsage)); // off

        let effects = engine.step(Event::Toggle(Setting::ShowAppUsage)); // on
        assert_eq!(effects.last(), Some(&Effect::ScheduleMenuReopen));

        let effects = engine.step(Event::ReopenMenuTimerFired);
        assert_eq!(effects.last(), Some(&Effect::PopUpMenu));
    }

    #[test]
    fn reopen_timer_is_ignored_once_app_usage_is_hidden_again() {
        let (mut engine, _, _) = engine_on();
        engine.step(Event::Startup);
        engine.step(Event::Toggle(Setting::ShowAppUsage)); // off

        assert!(engine.step(Event::ReopenMenuTimerFired).is_empty());
    }

    #[test]
    fn toggling_show_cpu_resets_the_sampler_and_gates_the_rows() {
        let (mut engine, counts, _) = engine_on();
        engine.step(Event::Startup);

        let effects = engine.step(Event::Toggle(Setting::ShowCpu)); // off
        assert_eq!(counts.cpu_resets.get(), 1);
        assert!(effects.contains(&Effect::PersistSetting(SettingChange::ShowCpu(false))));

        // With CPU hidden and the menu open, no CPU sampling happens.
        engine.step(Event::MenuWillOpen);
        assert_eq!(counts.cpu.get(), 0);
    }

    #[test]
    fn toggling_launch_at_login_rereads_status_through_the_port() {
        let (mut engine, _, _) = engine_on();
        engine.step(Event::Startup);

        let effects = engine.step(Event::Toggle(Setting::LaunchAtLogin));
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::Render(Render::SettingsState {
                launch_at_login: LaunchAtLoginStatus::Enabled,
                ..
            })
        )));
    }

    #[test]
    fn copy_diagnostics_carries_the_cached_state() {
        let (mut engine, _, _) = engine_on();
        engine.step(Event::Startup);

        let effects = engine.step(Event::CopyDiagnostics);
        assert!(matches!(
            effects.as_slice(),
            [Effect::CopyDiagnostics {
                memory: Some(_),
                ..
            }]
        ));
    }
}
