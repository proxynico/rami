use crate::lock::AppLock;
use crate::memory::MemorySampler;
use crate::tray::TrayController;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{NSObject, NSObjectProtocol, NSTimer};
use std::cell::RefCell;
use std::io;
use std::rc::{Rc, Weak};

thread_local! {
    static APP_STATE: RefCell<Option<Weak<AppState>>> = const { RefCell::new(None) };
}

struct AppState {
    tray: TrayController,
    sampler: MemorySampler,
}

impl AppState {
    fn refresh(&self) {
        let mtm = MainThreadMarker::new().expect("refreshes must stay on the main thread");
        match self.sampler.sample() {
            Ok(snapshot) => self.tray.set_snapshot(snapshot, mtm),
            Err(err) => eprintln!("failed to refresh RAM snapshot: {err}"),
        }
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
        state.refresh();
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
        let accessory_mode_set = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        assert!(
            accessory_mode_set,
            "failed to enter accessory activation policy"
        );
        let refresh_target = RefreshTarget::new(mtm);
        let refresh_target: Retained<AnyObject> = refresh_target.into();
        let tray = TrayController::new(mtm, refresh_target.clone());
        let state = Rc::new(AppState {
            tray,
            sampler: MemorySampler::new()?,
        });
        install_app_state(&state);
        app.finishLaunching();
        state.refresh();
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                5.0,
                &refresh_target,
                sel!(refreshNow:),
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
