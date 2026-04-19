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
        let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        let tray = TrayController::new();
        app.finishLaunching();

        Self { app, tray }
    }

    pub fn run(&mut self) {
        self.tray.set_placeholder();
        self.app.run();
    }
}
