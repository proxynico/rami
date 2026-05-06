use crate::app_control::quit_app_group;
use crate::lock::AppLock;
use crate::login_item::{LaunchAtLoginController, LaunchAtLoginStatus};
use crate::memory::MemorySampler;
use crate::model::MemoryPressure;
use crate::notification::{
    deliver_high_pressure_notification, high_pressure_notification_text,
    should_notify_high_pressure,
};
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage, ProcessMemorySampler};
use crate::tray::TrayController;
use crate::trend::{app_rows_with_deltas, likely_culprit, MemoryTrendTracker};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{NSObject, NSObjectProtocol, NSTimer};
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
    app_memory: RefCell<AppMemorySnapshot>,
    last_app_rows: RefCell<Vec<AppMemoryUsage>>,
    trend_tracker: RefCell<MemoryTrendTracker>,
    last_app_sample_at: Cell<Option<Instant>>,
    last_pressure: Cell<MemoryPressure>,
    last_high_pressure_notification: Cell<Option<Instant>>,
    ticks_until_app_refresh: Cell<u8>,
}

const APP_REFRESH_INTERVAL_TICKS: u8 = 6;
const APP_DELTA_BASELINE_MAX_AGE: Duration = Duration::from_secs(90);
const APP_BASELINE_ROW_LIMIT: usize = 25;
const MENU_REOPEN_DELAY_SECONDS: f64 = 0.05;

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
                let trend = self.trend_tracker.borrow_mut().record(snapshot.used_bytes);
                let previous_pressure = self.last_pressure.get();
                let pressure_sampling = !matches!(snapshot.pressure, MemoryPressure::Normal);
                let pressure_just_rose = !matches!(
                    previous_pressure,
                    MemoryPressure::Elevated | MemoryPressure::High
                ) && pressure_sampling;
                let high_pressure_just_started = !matches!(previous_pressure, MemoryPressure::High)
                    && matches!(snapshot.pressure, MemoryPressure::High);
                let app_sampling_enabled = self.show_app_usage.get() || pressure_sampling;
                if app_sampling_enabled {
                    let should_scan = manual
                        || self.ticks_until_app_refresh.get() == 0
                        || pressure_just_rose
                        || high_pressure_just_started;
                    if should_scan {
                        self.start_app_scan();
                        self.ticks_until_app_refresh
                            .set(APP_REFRESH_INTERVAL_TICKS.saturating_sub(1));
                    } else {
                        self.ticks_until_app_refresh
                            .set(self.ticks_until_app_refresh.get() - 1);
                    }
                } else {
                    self.clear_app_usage();
                }

                self.maybe_notify_high_pressure(previous_pressure, snapshot.pressure);
                self.last_pressure.set(snapshot.pressure);
                let apps = self.app_memory.borrow();
                let launch_at_login_status = self.launch_at_login_status.get();
                self.tray.set_snapshot(
                    snapshot,
                    trend,
                    &apps,
                    launch_at_login_status,
                    self.auto_refresh_enabled.get(),
                    mtm,
                );
            }
            Err(_) => {
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
                Err(_) => {
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

    fn maybe_notify_high_pressure(&self, previous: MemoryPressure, current: MemoryPressure) {
        let now = Instant::now();
        if !should_notify_high_pressure(
            previous,
            current,
            self.last_high_pressure_notification.get(),
            now,
        ) {
            return;
        }
        let apps = self.app_memory.borrow();
        let culprit = match &*apps {
            AppMemorySnapshot::Loaded(rows) => likely_culprit(rows),
            _ => None,
        };
        let body = high_pressure_notification_text(culprit.as_ref());
        deliver_high_pressure_notification(&body);
        self.last_high_pressure_notification.set(Some(now));
    }

    fn quit_app_at_index(&self, index: usize) {
        let usage = match &*self.app_memory.borrow() {
            AppMemorySnapshot::Loaded(rows) => rows.get(index).cloned(),
            _ => None,
        };
        if let Some(usage) = usage.filter(|usage| usage.can_quit) {
            let _ = quit_app_group(&usage);
            self.refresh(true);
        }
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
            self.clear_app_usage();
        }
        self.tray.set_show_app_usage(on);
        self.refresh(true);
        if on {
            self.reopen_menu_soon();
        }
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

        #[unsafe(method(reopenMenu:))]
        fn reopen_menu(&self, _sender: &AnyObject) {
            let state = APP_STATE.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade));
            if let Some(state) = state {
                state.reopen_menu_if_app_usage_visible();
            }
        }

        #[unsafe(method(quitAppAtIndex:))]
        fn quit_app_at_index(&self, sender: &AnyObject) {
            let tag: isize = unsafe { msg_send![sender, tag] };
            if tag < 0 {
                return;
            }
            let state = APP_STATE.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade));
            if let Some(state) = state {
                state.quit_app_at_index(tag as usize);
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
        let (app_scan_sender, app_scan_receiver) = mpsc::channel();
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
            auto_refresh_enabled: Cell::new(true),
            show_app_usage: Cell::new(false),
            app_memory: RefCell::new(AppMemorySnapshot::Hidden),
            last_app_rows: RefCell::new(Vec::new()),
            trend_tracker: RefCell::new(MemoryTrendTracker::new()),
            last_app_sample_at: Cell::new(None),
            last_pressure: Cell::new(MemoryPressure::Normal),
            last_high_pressure_notification: Cell::new(None),
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
