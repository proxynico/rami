use crate::tray::TrayController;
use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

pub struct App {
    app: Retained<NSApplication>,
    tray: TrayController,
}

impl App {
    pub fn new() -> Self {
        let mtm = MainThreadMarker::new().expect("app must start on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        let accessory_mode_set = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        assert!(accessory_mode_set, "failed to enter accessory activation policy");
        let tray = TrayController::new(mtm);
        app.finishLaunching();

        Self { app, tray }
    }

    pub fn run(&mut self) {
        let mtm = MainThreadMarker::new().expect("app run must stay on the main thread");
        self.tray.set_placeholder(mtm);
        self.app.run();
    }
}
