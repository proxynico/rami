use crate::app_control::quit_app_group;
use crate::diagnostics::{build_diagnostic_report, current_report_input};
use crate::lock::AppLock;
use crate::login_item::{LaunchAtLoginController, LaunchAtLoginStatus};
use crate::memory::MemorySampler;
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
    app_scan_sender: Sender<AppScanResult>,
    app_scan_receiver: Receiver<AppScanResult>,
    app_scan_in_flight: Cell<bool>,
    app_scan_generation: Cell<u64>,
    refresh_target: Retained<AnyObject>,
    launch_at_login: LaunchAtLoginController,
    launch_at_login_status: Cell<LaunchAtLoginStatus>,
    auto_refresh_enabled: Cell<bool>,
    show_app_usage: Cell<bool>,
    settings: SettingsStore,
    app_memory: RefCell<AppMemorySnapshot>,
    last_snapshot: RefCell<Option<crate::model::MemorySnapshot>>,
    last_app_rows: RefCell<Vec<AppMemoryUsage>>,
    trend_tracker: RefCell<MemoryTrendTracker>,
    last_app_sample_at: Cell<Option<Instant>>,
    ticks_until_app_refresh: Cell<u8>,
    menu_open: Cell<bool>,
}

const APP_REFRESH_INTERVAL_TICKS: u8 = 6;
const APP_DELTA_BASELINE_MAX_AGE: Duration = Duration::from_secs(90);
const APP_BASELINE_ROW_LIMIT: usize = 25;
const MENU_REOPEN_DELAY_SECONDS: f64 = 0.05;
const MENU_OPEN_DRAIN_DELAY_SECONDS: f64 = 0.15;

struct AppScanResult {
    generation: u64,
    completed_at: Instant,
    rows: io::Result<Vec<AppMemoryUsage>>,
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

/// Resolve the app to quit by its stable `group_key` rather than a menu position.
/// A background scan may reorder or drop rows between the menu being shown and the
/// click landing; matching on identity guarantees we quit the app the user picked,
/// or nothing if it is gone.
fn find_quittable<'a>(rows: &'a [AppMemoryUsage], key: &str) -> Option<&'a AppMemoryUsage> {
    rows.iter().find(|row| row.can_quit && row.group_key == key)
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
    fn find_quittable_matches_identity_regardless_of_position() {
        let rows = vec![usage("Chrome"), usage("Zen"), usage("Cursor")];
        // The same key resolves to the same app no matter where it sits in the list,
        // so a reorder between menu render and click cannot quit the wrong app.
        assert_eq!(
            find_quittable(&rows, "/Applications/Zen.app").map(|u| u.name.as_str()),
            Some("Zen")
        );
        let reordered = vec![usage("Zen"), usage("Cursor"), usage("Chrome")];
        assert_eq!(
            find_quittable(&reordered, "/Applications/Zen.app").map(|u| u.name.as_str()),
            Some("Zen")
        );
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
    fn find_quittable_skips_unquittable_and_missing_keys() {
        let mut rows = vec![usage("rami")];
        rows[0].can_quit = false;
        assert!(find_quittable(&rows, "/Applications/rami.app").is_none());
        assert!(find_quittable(&rows, "/Applications/Ghost.app").is_none());
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
        let mtm = MainThreadMarker::new().expect("refreshes must stay on the main thread");
        self.drain_app_scan_results();
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

                let apps = self.app_memory.borrow();
                let history = self.trend_tracker.borrow().samples();
                self.tray.set_snapshot(
                    snapshot,
                    trend,
                    &apps,
                    &history,
                    self.launch_at_login_status.get(),
                    self.auto_refresh_enabled.get(),
                    mtm,
                );
            }
            Err(err) => {
                eprintln!("memory sample failed: {err}");
                self.tray
                    .set_placeholder(self.launch_at_login_status.get(), mtm);
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
            let rows = ProcessMemorySampler::new().sample(APP_BASELINE_ROW_LIMIT);
            let _ = sender.send(AppScanResult {
                generation,
                completed_at: Instant::now(),
                rows,
            });
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

    fn quit_app_with_key(&self, key: &str) {
        let usage = match &*self.app_memory.borrow() {
            AppMemorySnapshot::Loaded(rows) => find_quittable(rows, key).cloned(),
            _ => None,
        };
        if let Some(usage) = usage {
            if let Err(err) = quit_app_group(&usage) {
                eprintln!("failed to quit {}: {err}", usage.name);
            }
            self.refresh(true);
        }
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
        self.tray.set_show_app_usage(on);
        self.refresh(true);
        if on {
            self.reopen_menu_soon();
        }
    }

    fn menu_will_open(&self) {
        self.menu_open.set(true);
        // The status read is an XPC round trip; menu open is the only moment
        // the answer is visible, so it is (re)read here rather than per tick.
        self.launch_at_login_status
            .set(self.launch_at_login.status());
        self.refresh(true);
        self.schedule_menu_open_drain();
    }

    fn menu_did_close(&self) {
        self.menu_open.set(false);
        self.ticks_until_app_refresh.set(0);
    }

    /// One-shot timer that drains the scan started by `menuWillOpen:` while the
    /// menu is still tracking. It must live in NSRunLoopCommonModes or it would
    /// only fire after the menu closes.
    fn schedule_menu_open_drain(&self) {
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
                MENU_OPEN_DRAIN_DELAY_SECONDS,
                &self.refresh_target,
                sel!(refreshNow:),
                None,
                false,
            )
        };
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
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

        #[unsafe(method(copyDiagnostics:))]
        fn copy_diagnostics(&self, _sender: &AnyObject) {
            with_app_state(|state| state.copy_diagnostic_report());
        }

        #[unsafe(method(checkForUpdates:))]
        fn check_for_updates(&self, _sender: &AnyObject) {
            open_releases_page();
        }

        #[unsafe(method(reopenMenu:))]
        fn reopen_menu(&self, _sender: &AnyObject) {
            with_app_state(|state| state.reopen_menu_if_app_usage_visible());
        }

        #[unsafe(method(quitApp:))]
        fn quit_app(&self, sender: &AnyObject) {
            let key: Option<Retained<NSString>> = unsafe { msg_send![sender, representedObject] };
            let Some(key) = key else {
                return;
            };
            with_app_state(|state| state.quit_app_with_key(&key.to_string()));
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
        let initial_app_memory = if settings.show_app_usage {
            AppMemorySnapshot::Loading
        } else {
            AppMemorySnapshot::Hidden
        };
        let state = Rc::new(AppState {
            tray,
            sampler: MemorySampler::new()?,
            app_scan_sender,
            app_scan_receiver,
            app_scan_in_flight: Cell::new(false),
            app_scan_generation: Cell::new(0),
            refresh_target: refresh_target.clone(),
            launch_at_login,
            launch_at_login_status: Cell::new(launch_at_login_status),
            auto_refresh_enabled: Cell::new(settings.auto_refresh_enabled),
            show_app_usage: Cell::new(settings.show_app_usage),
            settings: settings_store,
            app_memory: RefCell::new(initial_app_memory),
            last_snapshot: RefCell::new(None),
            last_app_rows: RefCell::new(Vec::new()),
            trend_tracker: RefCell::new(MemoryTrendTracker::new()),
            last_app_sample_at: Cell::new(None),
            ticks_until_app_refresh: Cell::new(0),
            menu_open: Cell::new(false),
        });
        install_app_state(&state);
        // Reflect the persisted "show apps" choice in the menu checkbox before first paint.
        state.tray.set_show_app_usage(settings.show_app_usage);
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
