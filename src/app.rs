use crate::login_item::{LaunchAtLoginController, LaunchAtLoginStatus};
use crate::lock::AppLock;
use crate::memory::MemorySampler;
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
    launch_at_login: LaunchAtLoginController,
    launch_at_login_status: Cell<LaunchAtLoginStatus>,
    auto_refresh_enabled: Cell<bool>,
}

impl AppState {
    fn refresh(&self, manual: bool) {
        if !manual && !self.auto_refresh_enabled.get() {
            return;
        }
        let mtm = MainThreadMarker::new().expect("refreshes must stay on the main thread");
        let launch_at_login_status = self.launch_at_login_status.get();
        match self.sampler.sample() {
            Ok(snapshot) => self.tray.set_snapshot(
                snapshot,
                launch_at_login_status,
                self.auto_refresh_enabled.get(),
                mtm,
            ),
            Err(err) => eprintln!("failed to refresh RAM snapshot: {err}"),
        }
    }

    fn toggle_launch_at_login(&self) {
        match self.launch_at_login.toggle() {
            Ok(status) => self.launch_at_login_status.set(status),
            Err(err) => {
                eprintln!("failed to update launch at login: {err}");
                self.launch_at_login_status.set(self.launch_at_login.status());
            }
        }
        self.refresh(true);
    }

    fn toggle_auto_refresh(&self) {
        self.auto_refresh_enabled
            .set(!self.auto_refresh_enabled.get());
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
            launch_at_login,
            launch_at_login_status: Cell::new(launch_at_login_status),
            auto_refresh_enabled: Cell::new(true),
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
