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
    /// Mirrors `CpuTracker`: the first CPU sample after construction or
    /// `reset_cpu` has no delta baseline and reports `None`.
    cpu_has_baseline: bool,
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
                cpu_has_baseline: false,
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
                cpu_has_baseline: false,
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
        if !self.cpu_has_baseline {
            self.cpu_has_baseline = true;
            return Ok(None);
        }
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
        self.cpu_has_baseline = false;
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
    assert_eq!(counts.gpu.get(), 0);
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
    // One open sample (baseline, no delta yet) plus exactly one warm-up
    // resample on the first drain; once the split is Available, further
    // drains render from cache.
    assert_eq!(counts.cpu.get(), 2);
    assert_eq!(counts.gpu.get(), 0);
    assert_eq!(scans.app_starts().len(), app_starts_before);
}

#[test]
fn menu_open_drain_warms_up_the_cpu_split_even_without_auto_refresh() {
    let (mut engine, _, _) = engine_on();
    engine.step(Event::Startup);
    engine.step(Event::Toggle(Setting::AutoRefresh)); // off: no future tick

    // The open itself only records the delta baseline, so the first
    // render legitimately shows Loading.
    let effects = engine.step(Event::MenuWillOpen);
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Render(Render::Menu {
            cpu: CpuModuleState::Loading,
            ..
        })
    )));

    // The 150 ms menu-open drain must deliver the split instead of
    // leaving the module on Loading until a manual Refresh.
    let effects = engine.step(Event::DrainTimerFired);
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Render(Render::Menu {
            cpu: CpuModuleState::Available(_),
            ..
        })
    )));
}

#[test]
fn reenabling_cpu_while_open_recovers_the_split_via_the_drain() {
    let (mut engine, _, _) = engine_on();
    engine.step(Event::Startup);
    engine.step(Event::MenuWillOpen);
    engine.step(Event::Toggle(Setting::ShowCpu)); // off

    let effects = engine.step(Event::Toggle(Setting::ShowCpu)); // on
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Render(Render::Menu {
            cpu: CpuModuleState::Loading,
            ..
        })
    )));

    let effects = engine.step(Event::DrainTimerFired);
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Render(Render::Menu {
            cpu: CpuModuleState::Available(_),
            ..
        })
    )));
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
fn populated_cpu_process_result_replaces_the_rows_in_the_render() {
    let (mut engine, _, scans) = engine_on();
    engine.step(Event::Startup);
    engine.step(Event::MenuWillOpen);
    let rows = vec![cpu_row("Editor", 12)];
    scans.push_cpu(0, rows.clone());

    let effects = engine.step(Event::DrainTimerFired);

    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Render(Render::Menu { cpu_processes: ProcessCpuSnapshot::Loaded(got), .. })
            if *got == rows
    )));
    assert!(scheduled_drains(&effects).is_empty());
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
