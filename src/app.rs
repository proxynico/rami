use crate::login_item::{LaunchAtLoginController, LaunchAtLoginStatus};
use crate::lock::AppLock;
use crate::memory::MemorySampler;
use crate::process_memory::{AppMemorySnapshot, ProcessMemorySampler};
use crate::tray::TrayController;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{NSObject, NSObjectProtocol, NSTimer};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::{Rc, Weak};

thread_local! {
    static APP_STATE: RefCell<Option<Weak<AppState>>> = const { RefCell::new(None) };
}

struct AppState {
    tray: TrayController,
    sampler: MemorySampler,
    process_sampler: ProcessMemorySampler,
    launch_at_login: LaunchAtLoginController,
    launch_at_login_status: Cell<LaunchAtLoginStatus>,
    auto_refresh_enabled: Cell<bool>,
    show_app_usage: Cell<bool>,
    app_memory: RefCell<AppMemorySnapshot>,
    ticks_until_app_refresh: Cell<u8>,
}

const APP_REFRESH_INTERVAL_TICKS: u8 = 6;
const TOP_APP_ROWS: usize = 5;

impl AppState {
    fn refresh(&self, manual: bool) {
        if !manual && !self.auto_refresh_enabled.get() {
            return;
        }
        let mtm = MainThreadMarker::new().expect("refreshes must stay on the main thread");
        let launch_at_login_status = self.launch_at_login_status.get();

        if self.show_app_usage.get() {
            let should_scan = manual || self.ticks_until_app_refresh.get() == 0;
            if should_scan {
                self.sample_apps();
                self.ticks_until_app_refresh
                    .set(APP_REFRESH_INTERVAL_TICKS.saturating_sub(1));
            } else {
                self.ticks_until_app_refresh
                    .set(self.ticks_until_app_refresh.get() - 1);
            }
        }

        if let Ok(snapshot) = self.sampler.sample() {
            let apps = self.app_memory.borrow();
            self.tray.set_snapshot(
                snapshot,
                &apps,
                launch_at_login_status,
                self.auto_refresh_enabled.get(),
                mtm,
            );
        }
    }

    fn sample_apps(&self) {
        let next = match self.process_sampler.sample(TOP_APP_ROWS) {
            Ok(rows) => AppMemorySnapshot::Loaded(rows),
            Err(_) => AppMemorySnapshot::Unavailable,
        };
        *self.app_memory.borrow_mut() = next;
    }

    fn toggle_launch_at_login(&self) {
        match self.launch_at_login.toggle() {
            Ok(status) => self.launch_at_login_status.set(status),
            Err(_) => self
                .launch_at_login_status
                .set(self.launch_at_login.status()),
        }
        self.refresh(true);
    }

    fn toggle_auto_refresh(&self) {
        self.auto_refresh_enabled
            .set(!self.auto_refresh_enabled.get());
        self.refresh(true);
    }

    fn toggle_show_app_usage(&self) {
        let on = !self.show_app_usage.get();
        self.show_app_usage.set(on);
        if on {
            *self.app_memory.borrow_mut() = AppMemorySnapshot::Loading;
            self.ticks_until_app_refresh.set(0);
        } else {
            *self.app_memory.borrow_mut() = AppMemorySnapshot::Hidden;
        }
        self.tray.set_show_app_usage(on);
        self.refresh(true);
    }
}

fn install_app_state(state: &Rc<AppState>) {
    APP_STATE.with(|slot| {
        *slot.borrow_mut() = Some(Rc::downgrade(state));
    });
}

fn refresh_current_app() {
    let state = APP_STATE.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade));
    if let Some(state) = state {
        state.refresh(true);
    }
}

fn timer_refresh_current_app() {
    let state = APP_STATE.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade));
    if let Some(state) = state {
        state.refresh(false);
    }
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

        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, _sender: &AnyObject) {
            let state = APP_STATE.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade));
            if let Some(state) = state {
                state.toggle_launch_at_login();
            }
        }

        #[unsafe(method(toggleAutoRefresh:))]
        fn toggle_auto_refresh(&self, _sender: &AnyObject) {
            let state = APP_STATE.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade));
            if let Some(state) = state {
                state.toggle_auto_refresh();
            }
        }

        #[unsafe(method(toggleShowAppUsage:))]
        fn toggle_show_app_usage(&self, _sender: &AnyObject) {
            let state = APP_STATE.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade));
            if let Some(state) = state {
                state.toggle_show_app_usage();
            }
        }
    }

    unsafe impl NSObjectProtocol for RefreshTarget {}
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
        let refresh_target: Retained<AnyObject> = refresh_target.into();
        let tray = TrayController::new(mtm, refresh_target.clone());
        let launch_at_login = LaunchAtLoginController::new();
        let launch_at_login_status = launch_at_login.status();
        let state = Rc::new(AppState {
            tray,
            sampler: MemorySampler::new()?,
            process_sampler: ProcessMemorySampler::new(),
            launch_at_login,
            launch_at_login_status: Cell::new(launch_at_login_status),
            auto_refresh_enabled: Cell::new(true),
            show_app_usage: Cell::new(false),
            app_memory: RefCell::new(AppMemorySnapshot::Hidden),
            ticks_until_app_refresh: Cell::new(0),
        });
        install_app_state(&state);
        app.finishLaunching();
        state.refresh(true);
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                5.0,
                &refresh_target,
                sel!(refreshOnTimer:),
                None,
                true,
            )
        };

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
