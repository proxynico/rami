//! Constructs the `TrayController`: the status item, the item pools the
//! dropdown is rebuilt from, and the Settings submenu. Pure construction —
//! all state updates stay in `tray/mod.rs`.

use super::layout::MenuShape;
use super::render::{make_action_icon, make_stat_item};
use super::style::{APP_ROW_POOL, BREAKDOWN_ROW_POOL};
use super::{RowItem, TrayController};
use crate::format::{placeholder_dropdown_model, Accent};
use crate::history_view::MemoryHistoryView;
use crate::login_item::LaunchAtLoginStatus;
use crate::memory_map_view::MemoryMapView;
use crate::model::MemoryPressure;
use crate::presentation::MenuMetrics;
use crate::pressure_view::PressureView;
use crate::trend::MemoryTrend;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSCellImagePosition, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem,
    NSStatusBar,
};
use objc2_foundation::NSString;
use std::cell::{Cell, RefCell};

/// One enabled command row: title, optional selector/target, optional
/// SF Symbol icon. Callers adjust state or enablement afterwards where a
/// row deviates from that default.
fn make_command_item(
    mtm: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    target: Option<&AnyObject>,
    icon: Option<&str>,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(""),
        )
    };
    if let Some(target) = target {
        unsafe {
            item.setTarget(Some(target));
        }
    }
    item.setEnabled(true);
    if let Some(name) = icon {
        if let Some(img) = make_action_icon(name) {
            item.setImage(Some(&img));
        }
    }
    item
}

pub(super) fn build_controller(
    mtm: MainThreadMarker,
    refresh_target: Retained<AnyObject>,
) -> TrayController {
    let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let empty = NSString::from_str("");
    let target = Some(&*refresh_target);

    let metrics = MenuMetrics::STANDARD;
    let map_item = make_stat_item(mtm);
    let map_view = MemoryMapView::new(mtm, metrics);
    unsafe {
        let _: () = msg_send![&map_item, setView: &*map_view];
    }
    let pressure_item = make_stat_item(mtm);
    let pressure_view = PressureView::new(mtm, metrics);
    unsafe {
        let _: () = msg_send![&pressure_item, setView: &*pressure_view];
    }
    let history_item = make_stat_item(mtm);
    let history_view = MemoryHistoryView::new(mtm, metrics);
    unsafe {
        let _: () = msg_send![&history_item, setView: &*history_view];
    }
    let legend_items = (0..BREAKDOWN_ROW_POOL)
        .map(|_| RowItem::new(mtm, metrics))
        .collect();
    let swap_item = RowItem::new(mtm, metrics);
    let loading_item = RowItem::muted(mtm, metrics, "Loading…");
    let app_loading_item = RowItem::muted(mtm, metrics, "Loading…");
    let app_unavailable_item = RowItem::muted(mtm, metrics, "Unavailable");
    let app_items = (0..APP_ROW_POOL)
        .map(|_| RowItem::new(mtm, metrics))
        .collect();
    let cpu_item = RowItem::new(mtm, metrics);
    let gpu_item = RowItem::new(mtm, metrics);

    let refresh_item = make_command_item(
        mtm,
        "Refresh",
        Some(sel!(refreshNow:)),
        target,
        Some("arrow.clockwise"),
    );

    let auto_refresh_item = make_command_item(
        mtm,
        "Auto-Refresh",
        Some(sel!(toggleAutoRefresh:)),
        target,
        None,
    );
    auto_refresh_item.setState(NSControlStateValueOn);
    let pause_icon = make_action_icon("pause.fill");
    let play_icon = make_action_icon("play.fill");
    if let Some(img) = &pause_icon {
        auto_refresh_item.setImage(Some(img));
    }

    let show_app_usage_item = make_command_item(
        mtm,
        "Show Apps",
        Some(sel!(toggleShowAppUsage:)),
        target,
        None,
    );
    show_app_usage_item.setState(NSControlStateValueOn);

    let show_cpu_item =
        make_command_item(mtm, "Show CPU", Some(sel!(toggleShowCpu:)), target, None);
    show_cpu_item.setState(NSControlStateValueOn);

    let show_gpu_item =
        make_command_item(mtm, "Show GPU", Some(sel!(toggleShowGpu:)), target, None);
    show_gpu_item.setState(NSControlStateValueOff);

    let launch_at_login_item = make_command_item(
        mtm,
        LaunchAtLoginStatus::Disabled.menu_title(),
        Some(sel!(toggleLaunchAtLogin:)),
        target,
        None,
    );
    launch_at_login_item.setState(NSControlStateValueOff);
    launch_at_login_item.setEnabled(false);

    let diagnostics_item = make_command_item(
        mtm,
        "Copy Diagnostics",
        Some(sel!(copyDiagnostics:)),
        target,
        Some("doc.on.doc"),
    );

    let version = env!("CARGO_PKG_VERSION");
    let about_item = make_command_item(
        mtm,
        &format!("rami {version}"),
        None,
        None,
        Some("info.circle"),
    );
    about_item.setEnabled(false);

    let check_updates_item = make_command_item(
        mtm,
        "Check for Updates",
        Some(sel!(checkForUpdates:)),
        target,
        Some("arrow.up.circle"),
    );

    let settings_menu = NSMenu::new(mtm);
    settings_menu.setAutoenablesItems(false);
    settings_menu.addItem(&auto_refresh_item);
    settings_menu.addItem(&show_app_usage_item);
    settings_menu.addItem(&show_cpu_item);
    settings_menu.addItem(&show_gpu_item);
    settings_menu.addItem(&launch_at_login_item);
    settings_menu.addItem(&diagnostics_item);
    settings_menu.addItem(&NSMenuItem::separatorItem(mtm));
    settings_menu.addItem(&check_updates_item);
    settings_menu.addItem(&about_item);

    let settings_item = make_command_item(mtm, "Settings", None, None, Some("gearshape"));
    settings_item.setSubmenu(Some(&settings_menu));

    let quit_item = make_command_item(mtm, "Quit", Some(sel!(terminate:)), None, None);

    status_item.setMenu(Some(&menu));
    if let Some(button) = status_item.button(mtm) {
        button.setTitle(&empty);
        button.setImagePosition(NSCellImagePosition::ImageOnly);
    }

    let controller = TrayController {
        status_item,
        menu,
        map_item,
        map_view,
        pressure_item,
        pressure_view,
        history_item,
        history_view,
        legend_items,
        swap_item,
        loading_item,
        app_loading_item,
        app_unavailable_item,
        app_items,
        cpu_item,
        gpu_item,
        refresh_item,
        auto_refresh_item,
        show_app_usage_item,
        show_cpu_item,
        show_gpu_item,
        launch_at_login_item,
        _diagnostics_item: diagnostics_item,
        _about_item: about_item,
        _check_updates_item: check_updates_item,
        settings_item,
        quit_item,
        pause_icon,
        play_icon,
        last_gauge_percent: Cell::new(None),
        last_trend: Cell::new(MemoryTrend::Stable),
        last_pressure: Cell::new(MemoryPressure::Normal),
        shape: Cell::new(MenuShape::Uninitialized),
        last_map: RefCell::new(None),
        last_pressure_percent: Cell::new(None),
        last_history: RefCell::new(None),
        last_breakdown: RefCell::new(None),
        last_accent: Cell::new(Accent::Neutral),
        last_swap_row: RefCell::new(None),
        last_app_section: RefCell::new(None),
        last_cpu_row: RefCell::new(None),
        last_gpu_row: RefCell::new(None),
        last_auto_refresh_enabled: Cell::new(true),
        last_tooltip: RefCell::new(String::new()),
        last_launch_title: RefCell::new(String::new()),
        last_launch_checked: Cell::new(false),
        last_launch_enabled: Cell::new(false),
        last_show_app_usage: Cell::new(true),
        last_show_cpu: Cell::new(true),
        last_show_gpu: Cell::new(false),
    };
    controller.set_gauge(0, MemoryTrend::Stable, MemoryPressure::Normal, mtm);
    controller.apply_model(
        &placeholder_dropdown_model(),
        LaunchAtLoginStatus::Disabled,
        true,
        mtm,
    );
    controller
}
