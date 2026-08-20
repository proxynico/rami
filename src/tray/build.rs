//! Constructs the `TrayController`: the status item, the item pools the
//! dropdown is rebuilt from, and the Settings submenu. Pure construction —
//! all state updates stay in `tray/mod.rs`.

use super::layout::MenuShape;
use super::render::{
    loading_attributed_title, make_action_icon, make_placeholder_icon, make_stat_item,
    set_row_icon, unavailable_attributed_title, RowRenderCache,
};
use super::style::APP_ROW_POOL;
use super::TrayController;
use crate::format::{placeholder_dropdown_model, Accent};
use crate::history_view::MemoryHistoryView;
use crate::login_item::LaunchAtLoginStatus;
use crate::memory_view::MemoryRingsView;
use crate::model::MemoryPressure;
use crate::module_title_view::ModuleTitleView;
use crate::process_cpu::PROCESS_CPU_ROW_LIMIT;
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
use std::collections::HashMap;

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

    let placeholder_icon = make_placeholder_icon();
    let row_render_cache = RowRenderCache::new();
    let rings_item = make_stat_item(mtm);
    let rings_view = MemoryRingsView::new(mtm);
    unsafe {
        let _: () = msg_send![&rings_item, setView: &*rings_view];
    }
    let history_item = make_stat_item(mtm);
    let history_view = MemoryHistoryView::new(mtm);
    unsafe {
        let _: () = msg_send![&history_item, setView: &*history_view];
    }
    let legend_items = (0..4).map(|_| make_stat_item(mtm)).collect();
    let swap_item = make_stat_item(mtm);
    set_row_icon(&swap_item, "arrow.up.arrow.down", &placeholder_icon);
    let loading_item = make_stat_item(mtm);
    loading_item.setImage(Some(&placeholder_icon));
    loading_item.setAttributedTitle(Some(&loading_attributed_title(&row_render_cache)));
    let app_loading_item = make_stat_item(mtm);
    app_loading_item.setImage(Some(&placeholder_icon));
    app_loading_item.setAttributedTitle(Some(&loading_attributed_title(&row_render_cache)));
    let app_unavailable_item = make_stat_item(mtm);
    app_unavailable_item.setImage(Some(&placeholder_icon));
    app_unavailable_item.setAttributedTitle(Some(&unavailable_attributed_title(&row_render_cache)));
    let app_items: Vec<Retained<NSMenuItem>> =
        (0..APP_ROW_POOL).map(|_| make_stat_item(mtm)).collect();
    let cpu_title_item = make_stat_item(mtm);
    let cpu_title_view = ModuleTitleView::new(mtm, "CPU");
    unsafe {
        let _: () = msg_send![&cpu_title_item, setView: &*cpu_title_view];
    }
    let cpu_loading_item = make_stat_item(mtm);
    cpu_loading_item.setImage(Some(&placeholder_icon));
    cpu_loading_item.setAttributedTitle(Some(&loading_attributed_title(&row_render_cache)));
    let cpu_unavailable_item = make_stat_item(mtm);
    cpu_unavailable_item.setImage(Some(&placeholder_icon));
    cpu_unavailable_item.setAttributedTitle(Some(&unavailable_attributed_title(&row_render_cache)));
    let cpu_legend_items = (0..2).map(|_| make_stat_item(mtm)).collect();
    let cpu_core_items = (0..2).map(|_| make_stat_item(mtm)).collect();
    let cpu_process_items = (0..PROCESS_CPU_ROW_LIMIT)
        .map(|_| make_stat_item(mtm))
        .collect();
    let gpu_title_item = make_stat_item(mtm);
    let gpu_title_view = ModuleTitleView::new(mtm, "GPU");
    unsafe {
        let _: () = msg_send![&gpu_title_item, setView: &*gpu_title_view];
    }
    let gpu_utilization_item = make_stat_item(mtm);

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

    let settings_item = make_command_item(
        mtm,
        "Settings",
        Some(sel!(openSettings:)),
        target,
        Some("gearshape"),
    );

    let quit_item = make_command_item(mtm, "Quit", Some(sel!(terminate:)), None, None);

    status_item.setMenu(Some(&menu));
    if let Some(button) = status_item.button(mtm) {
        button.setTitle(&empty);
        button.setImagePosition(NSCellImagePosition::ImageOnly);
    }

    let controller = TrayController {
        status_item,
        menu,
        rings_item,
        rings_view,
        history_item,
        history_view,
        legend_items,
        swap_item,
        loading_item,
        app_loading_item,
        app_unavailable_item,
        app_items,
        cpu_title_item,
        cpu_loading_item,
        cpu_unavailable_item,
        cpu_legend_items,
        cpu_core_items,
        cpu_process_items,
        gpu_title_item,
        gpu_utilization_item,
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
        settings_menu,
        quit_item,
        pause_icon,
        play_icon,
        last_image_name: RefCell::new(None),
        last_trend: Cell::new(MemoryTrend::Stable),
        last_pressure: Cell::new(MemoryPressure::Normal),
        shape: Cell::new(MenuShape::Uninitialized),
        last_rings: RefCell::new(None),
        last_history: RefCell::new(None),
        last_breakdown: RefCell::new(None),
        last_accent: Cell::new(Accent::Neutral),
        last_swap_row: RefCell::new(None),
        last_app_section: RefCell::new(None),
        last_cpu_state: RefCell::new(None),
        last_gpu: RefCell::new(None),
        last_auto_refresh_enabled: Cell::new(true),
        last_tooltip: RefCell::new(String::new()),
        last_launch_title: RefCell::new(String::new()),
        last_launch_checked: Cell::new(false),
        last_launch_enabled: Cell::new(false),
        last_show_app_usage: Cell::new(true),
        last_show_cpu: Cell::new(true),
        last_show_gpu: Cell::new(false),
        app_icon_cache: RefCell::new(HashMap::new()),
        row_render_cache,
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
