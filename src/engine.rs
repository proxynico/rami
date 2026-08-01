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
//! - Port adapters must not pump the main run loop: `step` runs under the
//!   shell's engine borrow, so a port whose call let a scheduled timer fire
//!   would re-enter dispatch and panic on the second borrow. The current
//!   adapters (mach/sysctl reads, channel operations, synchronous SMAppService
//!   XPC) all block without servicing the run loop.
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

/// Real-world seconds per `Tick { manual: false }`. The shell's repeating
/// timer must use this period: every tick-counted cadence below — the 6-tick
/// app scan (~30 s), the sampler's swap cadence, and the 25-sample trend
/// window (~125 s) — encodes wall-clock time as a multiple of it.
pub(crate) const REFRESH_TICK_SECONDS: f64 = 5.0;

const APP_REFRESH_INTERVAL_TICKS: u8 = 6;
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
        /// The trend window for the memory-history sparkline. Recorded every
        /// real refresh regardless of menu state, so it is warm on open.
        history: Vec<u64>,
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
    /// This deliberately does NOT resample memory, record a trend sample, or
    /// advance any cadence counter. It runs on a 150–300 ms one-shot timer,
    /// and routing it through the full `refresh` path meant every drain aged
    /// the tick-counted cadences: the 125 s trend window collapsed to about
    /// 7 s and the 30 s app/swap cadences to under 2 s whenever the menu was
    /// open. Those counters are meant to track wall-clock time, and a drain
    /// represents no elapsed time.
    ///
    /// The one exception is the CPU host split while it reads Loading: the
    /// sample that ran on menu open (or on re-enabling Show CPU) only records
    /// the delta baseline, so without a second reading the User/System rows
    /// would sit on Loading until the next tick — indefinitely with
    /// Auto-Refresh off. The drain's 150–300 ms delay is a valid delta
    /// window, and host CPU sampling advances no cadence, so warming it up
    /// here is safe.
    fn drain_and_rerender(&mut self, out: &mut Vec<Effect>) {
        if !self.menu_open {
            return;
        }

        self.drain_app_scan_results();
        let cpu_processes_updated = self.drain_cpu_process_scan_results();

        if matches!(self.last_cpu_state, CpuModuleState::Loading) {
            self.last_cpu_state = self.sample_cpu_if_visible();
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
            history: self.trend_tracker.window(),
            launch_at_login: self.launch_at_login_status,
            auto_refresh_enabled: self.auto_refresh_enabled,
        }
    }
}

#[cfg(test)]
mod tests;
