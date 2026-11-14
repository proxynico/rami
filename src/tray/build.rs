//! Constructs the `TrayController`: the status item, the item pools the
//! dropdown is rebuilt from, and the Settings submenu. Pure construction —
//! all state updates stay in `tray/mod.rs`.

use super::{RowItem, TrayController};
use crate::format::{Accent, LEGEND_CATEGORIES};
use crate::history_view::MemoryHistoryView;
use crate::memory_map_view::MemoryMapView;
use crate::model::MemoryPressure;
use crate::pressure_view::PressureView;
use crate::row_view::{RowContent, RowView};
use crate::shared_mac_view::SharedMacView;
use crate::trend::MemoryTrend;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSCellImagePosition, NSMenu, NSMenuItem, NSStatusBar, NSView};
use objc2_foundation::NSString;
use std::cell::{Cell, RefCell};

fn make_command_item(
    mtm: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    target: Option<&AnyObject>,
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
    item
}

/// An untitled, enabled item that shows `view` instead of a title.
fn make_view_item(mtm: MainThreadMarker, view: &NSView) -> Retained<NSMenuItem> {
    let item = make_command_item(mtm, "", None, None);
    item.setView(Some(view));
    item
}

fn make_row(mtm: MainThreadMarker) -> RowItem {
    let view = RowView::new(mtm);
    RowItem {
        item: make_view_item(mtm, &view),
        view,
    }
}

fn make_muted_row(mtm: MainThreadMarker, label: &str) -> RowItem {
    let row = make_row(mtm);
    row.view.update(&RowContent::muted(label), Accent::Neutral);
    row
}

pub(super) fn build_controller(
    mtm: MainThreadMarker,
    refresh_target: Retained<AnyObject>,
    shared_mac_configured: bool,
) -> TrayController {
    let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let target = Some(&*refresh_target);

    let map_view = MemoryMapView::new(mtm);
    let pressure_view = PressureView::new(mtm);
    let history_view = MemoryHistoryView::new(mtm);
    let shared_view = SharedMacView::new(mtm);

    let auto_refresh_item =
        make_command_item(mtm, "Auto-Refresh", Some(sel!(toggleAutoRefresh:)), target);
    let show_cpu_item = make_command_item(mtm, "Show CPU", Some(sel!(toggleShowCpu:)), target);
    let show_gpu_item = make_command_item(mtm, "Show GPU", Some(sel!(toggleShowGpu:)), target);
    let show_shared_mac_item = shared_mac_configured.then(|| {
        make_command_item(
            mtm,
            "Show Shared Mac",
            Some(sel!(toggleShowSharedMac:)),
            target,
        )
    });
    let share_item = make_command_item(
        mtm,
        "Share over Tailscale",
        Some(sel!(toggleShareOverTailscale:)),
        target,
    );
    let launch_at_login_item = make_command_item(
        mtm,
        "Launch at Login",
        Some(sel!(toggleLaunchAtLogin:)),
        target,
    );
    let diagnostics_item = make_command_item(
        mtm,
        "Copy Diagnostics",
        Some(sel!(copyDiagnostics:)),
        target,
    );
    let check_updates_item = make_command_item(
        mtm,
        "Check for Updates",
        Some(sel!(checkForUpdates:)),
        target,
    );
    let about_item = make_command_item(
        mtm,
        &format!("rami {}", env!("CARGO_PKG_VERSION")),
        None,
        None,
    );
    about_item.setEnabled(false);

    // The submenu retains its items, and the Settings item retains it.
    let settings_menu = NSMenu::new(mtm);
    settings_menu.setAutoenablesItems(false);
    for item in [&auto_refresh_item, &show_cpu_item, &show_gpu_item] {
        settings_menu.addItem(item);
    }
    if let Some(item) = &show_shared_mac_item {
        settings_menu.addItem(item);
    }
    for item in [&share_item, &launch_at_login_item, &diagnostics_item] {
        settings_menu.addItem(item);
    }
    settings_menu.addItem(&NSMenuItem::separatorItem(mtm));
    settings_menu.addItem(&check_updates_item);
    settings_menu.addItem(&about_item);

    let settings_item = make_command_item(mtm, "Settings", None, None);
    settings_item.setSubmenu(Some(&settings_menu));

    let quit_item = make_command_item(mtm, "Quit", Some(sel!(terminate:)), None);

    status_item.setMenu(Some(&menu));
    if let Some(button) = status_item.button(mtm) {
        button.setTitle(&NSString::from_str(""));
        button.setImagePosition(NSCellImagePosition::ImageOnly);
    }

    let controller = TrayController {
        status_item,
        menu,
        map_item: make_view_item(mtm, &map_view),
        map_view,
        pressure_item: make_view_item(mtm, &pressure_view),
        pressure_view,
        history_item: make_view_item(mtm, &history_view),
        history_view,
        legend_items: (0..LEGEND_CATEGORIES.len())
            .map(|_| make_row(mtm))
            .collect(),
        swap_item: make_row(mtm),
        loading_item: make_muted_row(mtm, "Loading…"),
        cpu_item: make_row(mtm),
        gpu_item: make_row(mtm),
        shared_status_item: make_row(mtm),
        shared_item: make_view_item(mtm, &shared_view),
        shared_view,
        settings_item,
        quit_item,
        auto_refresh_item,
        show_cpu_item,
        show_gpu_item,
        share_item,
        show_shared_mac_item,
        launch_at_login_item,
        shape: Cell::new(None),
        last_gauge: Cell::new(None),
        last_tooltip: RefCell::new(String::new()),
        last_settings: Cell::new(None),
    };
    controller.set_gauge(0, MemoryTrend::Stable, MemoryPressure::Normal, mtm);
    controller.apply_model(None, mtm);
    controller
}
