//! The AppKit shell around [`crate::engine::RefreshEngine`].
//!
//! Everything here is an adapter: Objective-C callbacks translate into engine
//! [`Event`]s, and the returned [`Effect`]s are performed against AppKit —
//! timers, tray rendering, settings persistence, the pasteboard. No lifecycle
//! decisions live in this file; if a change touches when to sample, scan,
//! drain, or render, it belongs in `engine.rs` where it is testable.

use crate::cpu::CpuSampler;
use crate::diagnostics::{build_diagnostic_report, current_report_input};
use crate::engine::{
    AppScanResult, CpuProcessScanResult, Effect, Event, LaunchAtLogin, RefreshEngine, Render,
    Samplers, ScanRunner, Setting, SettingChange, REFRESH_TICK_SECONDS,
};
use crate::gpu::GpuSampler;
use crate::lock::AppLock;
use crate::login_item::{LaunchAtLoginController, LaunchAtLoginStatus};
use crate::memory::MemorySampler;
use crate::model::{CpuSnapshot, GpuSnapshot, MemorySnapshot};
use crate::process_cpu::{ProcessCpuSampler, PROCESS_CPU_ROW_LIMIT};
use crate::process_memory::ProcessMemorySampler;
use crate::settings::SettingsStore;
use crate::tray::TrayController;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuDelegate, NSPasteboard,
    NSPasteboardTypeString,
};
use objc2_foundation::{
    NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes, NSString, NSTimer,
};
use std::cell::RefCell;
use std::io;
use std::rc::{Rc, Weak};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

thread_local! {
    static APP_STATE: RefCell<Option<Weak<AppState>>> = const { RefCell::new(None) };
}

const MENU_REOPEN_DELAY_SECONDS: f64 = 0.05;
const APP_BASELINE_ROW_LIMIT: usize = 25;

/// The three synchronous kernel samplers behind the engine's `Samplers` port.
struct SystemSamplers {
    memory: MemorySampler,
    cpu: CpuSampler,
    gpu: GpuSampler,
}

impl Samplers for SystemSamplers {
    fn sample_memory(&mut self) -> io::Result<MemorySnapshot> {
        self.memory.sample()
    }

    fn sample_cpu(&mut self) -> io::Result<Option<CpuSnapshot>> {
        self.cpu.sample()
    }

    fn sample_gpu(&mut self) -> io::Result<Option<GpuSnapshot>> {
        self.gpu.sample()
    }

    fn reset_cpu(&mut self) {
        self.cpu.reset();
    }
}

/// Worker threads and channels behind the engine's `ScanRunner` port. Results
/// stay queued in the channels until the engine polls, so nothing is lost
/// when a refresh is skipped.
struct ThreadScanRunner {
    app_sender: Sender<AppScanResult>,
    app_receiver: Receiver<AppScanResult>,
    cpu_sender: Sender<CpuProcessScanResult>,
    cpu_receiver: Receiver<CpuProcessScanResult>,
}

impl ThreadScanRunner {
    fn new() -> Self {
        let (app_sender, app_receiver) = mpsc::channel();
        let (cpu_sender, cpu_receiver) = mpsc::channel();
        Self {
            app_sender,
            app_receiver,
            cpu_sender,
            cpu_receiver,
        }
    }
}

impl ScanRunner for ThreadScanRunner {
    fn start_app_scan(&mut self, generation: u64) {
        let sender = self.app_sender.clone();
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

    fn start_cpu_process_scan(&mut self, generation: u64) {
        let sender = self.cpu_sender.clone();
        thread::spawn(move || {
            // SAFETY: this only changes the current worker thread's QoS preference.
            unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0) };
            let rows = ProcessCpuSampler::new().sample(PROCESS_CPU_ROW_LIMIT);
            let _ = sender.send(CpuProcessScanResult { generation, rows });
        });
    }

    fn poll_app_scan(&mut self) -> Option<AppScanResult> {
        self.app_receiver.try_recv().ok()
    }

    fn poll_cpu_process_scan(&mut self) -> Option<CpuProcessScanResult> {
        self.cpu_receiver.try_recv().ok()
    }
}

/// The SMAppService XPC controller behind the engine's `LaunchAtLogin` port.
/// A failed toggle folds into a fresh status read so the engine only ever
/// sees a status.
struct LoginItemPort(LaunchAtLoginController);

impl LaunchAtLogin for LoginItemPort {
    fn status(&mut self) -> LaunchAtLoginStatus {
        self.0.status()
    }

    fn toggle(&mut self) -> LaunchAtLoginStatus {
        match self.0.toggle() {
            Ok(status) => status,
            Err(err) => {
                eprintln!("failed to toggle launch at login: {err}");
                self.0.status()
            }
        }
    }
}

type ProductionEngine = RefreshEngine<SystemSamplers, ThreadScanRunner, LoginItemPort>;

struct AppState {
    engine: RefCell<ProductionEngine>,
    tray: TrayController,
    settings: SettingsStore,
    refresh_target: Retained<AnyObject>,
    /// The single in-flight menu-open drain timer. Held so a new drain can
    /// cancel the previous one instead of stacking overlapping timers — the
    /// engine's `ScheduleDrain` effect relies on this replace semantics.
    menu_open_drain_timer: RefCell<Option<Retained<NSTimer>>>,
    refresh_timer: RefCell<Option<Retained<NSTimer>>>,
}

impl AppState {
    /// Run one engine step and perform its effects. The engine borrow is
    /// released before effects run: `PopUpMenu` re-enters `menuWillOpen:`
    /// synchronously, which dispatches a new event through this same path.
    fn dispatch(&self, event: Event) {
        let effects = self.engine.borrow_mut().step(event);
        self.apply(effects);
    }

    fn apply(&self, effects: Vec<Effect>) {
        let mtm = MainThreadMarker::new().expect("effects must run on the main thread");
        for effect in effects {
            match effect {
                Effect::Render(render) => self.render(render, mtm),
                Effect::ScheduleDrain { delay } => {
                    self.schedule_menu_open_drain(delay.as_secs_f64())
                }
                Effect::CancelDrain => self.cancel_menu_open_drain(),
                Effect::ArmRefreshTimer => self.arm_refresh_timer(),
                Effect::CancelRefreshTimer => self.cancel_refresh_timer(),
                Effect::PersistSetting(change) => match change {
                    SettingChange::AutoRefresh(value) => {
                        self.settings.set_auto_refresh_enabled(value)
                    }
                    SettingChange::ShowAppUsage(value) => self.settings.set_show_app_usage(value),
                    SettingChange::ShowCpu(value) => self.settings.set_show_cpu(value),
                    SettingChange::ShowGpu(value) => self.settings.set_show_gpu(value),
                },
                Effect::ScheduleMenuReopen => self.schedule_one_shot(sel!(reopenMenu:)),
                Effect::PopUpMenu => self.tray.pop_up_menu(),
                Effect::CopyDiagnostics {
                    launch_at_login,
                    memory,
                    apps,
                } => {
                    let report = build_diagnostic_report(current_report_input(
                        launch_at_login,
                        memory,
                        &apps,
                    ));
                    copy_to_pasteboard(&report);
                }
            }
        }
    }

    fn render(&self, render: Render, mtm: MainThreadMarker) {
        match render {
            Render::Gauge { snapshot, trend } => self.tray.set_gauge_snapshot(snapshot, trend, mtm),
            Render::Menu {
                snapshot,
                cpu,
                gpu,
                apps,
                cpu_processes,
                launch_at_login,
                auto_refresh_enabled,
            } => self.tray.set_menu_snapshot(
                snapshot,
                cpu,
                gpu,
                &apps,
                &cpu_processes,
                launch_at_login,
                auto_refresh_enabled,
                mtm,
            ),
            Render::SettingsState {
                launch_at_login,
                auto_refresh_enabled,
                show_app_usage,
                show_cpu,
                show_gpu,
            } => self.tray.set_settings_state(
                launch_at_login,
                auto_refresh_enabled,
                show_app_usage,
                show_cpu,
                show_gpu,
            ),
            Render::Placeholder { launch_at_login } => {
                self.tray.set_placeholder(launch_at_login, mtm)
            }
        }
    }

    fn arm_refresh_timer(&self) {
        // NSRunLoopCommonModes keeps scheduled refreshes active while the
        // menu is tracking; the default mode would freeze the dropdown.
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                REFRESH_TICK_SECONDS,
                &self.refresh_target,
                sel!(refreshOnTimer:),
                None,
                true,
            )
        };
        // Let the OS coalesce this timer with other wakeups. Kept to 10%
        // of the interval: the menu-bar gauge updates on every tick, and
        // the tick also drives the trend window and the app/swap cadences,
        // so a large tolerance would reintroduce the cadence drift #18
        // fixed — from the other direction.
        timer.setTolerance(0.5);
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
        *self.refresh_timer.borrow_mut() = Some(timer);
    }

    fn cancel_refresh_timer(&self) {
        if let Some(timer) = self.refresh_timer.borrow_mut().take() {
            timer.invalidate();
        }
    }

    /// One-shot timer that drains scan results while the menu is still
    /// tracking. It must live in NSRunLoopCommonModes or it would only fire
    /// after the menu closes. Replaces any pending drain: the engine emits
    /// its schedules in order and the latest must win.
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

    /// One-shot at the short reopen delay, targeting a `RefreshTarget`
    /// selector. Used to let the current menu-tracking session end before
    /// popping a menu back up.
    fn schedule_one_shot(&self, selector: Sel) {
        let _timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                MENU_REOPEN_DELAY_SECONDS,
                &self.refresh_target,
                selector,
                None,
                false,
            )
        };
    }
}

fn copy_to_pasteboard(report: &str) {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    let string_type = unsafe { NSPasteboardTypeString };
    if !pasteboard.setString_forType(&NSString::from_str(report), string_type) {
        eprintln!("failed to copy diagnostics to pasteboard");
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

fn dispatch_event(event: Event) {
    with_app_state(|state| state.dispatch(event));
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct RefreshTarget;

    impl RefreshTarget {
        #[unsafe(method(refreshNow:))]
        fn refresh_now(&self, _sender: &AnyObject) {
            dispatch_event(Event::Tick { manual: true });
        }

        #[unsafe(method(refreshOnTimer:))]
        fn refresh_on_timer(&self, _sender: &AnyObject) {
            dispatch_event(Event::Tick { manual: false });
        }

        #[unsafe(method(drainScanResults:))]
        fn drain_scan_results(&self, _sender: &AnyObject) {
            dispatch_event(Event::DrainTimerFired);
        }

        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, _sender: &AnyObject) {
            dispatch_event(Event::Toggle(Setting::LaunchAtLogin));
        }

        #[unsafe(method(toggleAutoRefresh:))]
        fn toggle_auto_refresh(&self, _sender: &AnyObject) {
            dispatch_event(Event::Toggle(Setting::AutoRefresh));
        }

        #[unsafe(method(toggleShowAppUsage:))]
        fn toggle_show_app_usage(&self, _sender: &AnyObject) {
            dispatch_event(Event::Toggle(Setting::ShowAppUsage));
        }

        #[unsafe(method(toggleShowCpu:))]
        fn toggle_show_cpu(&self, _sender: &AnyObject) {
            dispatch_event(Event::Toggle(Setting::ShowCpu));
        }

        #[unsafe(method(toggleShowGpu:))]
        fn toggle_show_gpu(&self, _sender: &AnyObject) {
            dispatch_event(Event::Toggle(Setting::ShowGpu));
        }

        #[unsafe(method(copyDiagnostics:))]
        fn copy_diagnostics(&self, _sender: &AnyObject) {
            dispatch_event(Event::CopyDiagnostics);
        }

        #[unsafe(method(checkForUpdates:))]
        fn check_for_updates(&self, _sender: &AnyObject) {
            open_releases_page();
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: &AnyObject) {
            with_app_state(|state| state.schedule_one_shot(sel!(openSettingsMenu:)));
        }

        #[unsafe(method(openSettingsMenu:))]
        fn open_settings_menu(&self, _sender: &AnyObject) {
            with_app_state(|state| state.tray.pop_up_settings_menu());
        }

        #[unsafe(method(reopenMenu:))]
        fn reopen_menu(&self, _sender: &AnyObject) {
            dispatch_event(Event::ReopenMenuTimerFired);
        }

    }

    unsafe impl NSObjectProtocol for RefreshTarget {}

    unsafe impl NSMenuDelegate for RefreshTarget {
        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self, _menu: &NSMenu) {
            dispatch_event(Event::MenuWillOpen);
        }

        #[unsafe(method(menuDidClose:))]
        fn menu_did_close(&self, _menu: &NSMenu) {
            dispatch_event(Event::MenuDidClose);
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
        let settings_store = SettingsStore::new();
        let settings = settings_store.load();
        let engine = RefreshEngine::new(
            SystemSamplers {
                memory: MemorySampler::new()?,
                cpu: CpuSampler::new(),
                gpu: GpuSampler::new(),
            },
            ThreadScanRunner::new(),
            LoginItemPort(LaunchAtLoginController::new()),
            settings,
        );
        let state = Rc::new(AppState {
            engine: RefCell::new(engine),
            tray,
            settings: settings_store,
            refresh_target: refresh_target.clone(),
            menu_open_drain_timer: RefCell::new(None),
            refresh_timer: RefCell::new(None),
        });
        install_app_state(&state);
        app.finishLaunching();
        state.dispatch(Event::Startup);

        Ok(Some(Self {
            app,
            _lock: lock,
            _state: state,
            _refresh_target: refresh_target,
        }))
    }

    pub fn run(&mut self) {
        self.app.run();
    }
}
