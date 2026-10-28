mod build;
mod layout;

use self::layout::{menu_shape_for, menu_slots, MenuShape, Slot};
use crate::format::{
    dropdown_model, gauge_accessibility_label, gauge_tooltip, Accent, AppSectionDisplay,
    DropdownModel,
};
use crate::history_view::MemoryHistoryView;
use crate::login_item::LaunchAtLoginStatus;
use crate::memory_map_view::MemoryMapView;
use crate::model::{
    classify_pressure, CpuModuleState, GpuModuleState, MemoryPressure, MemorySnapshot,
    SystemSnapshot,
};
use crate::pressure_view::PressureView;
use crate::process_memory::AppMemorySnapshot;
use crate::row_view::{RowContent, RowView};
use crate::settings::Settings;
use crate::status_icon::{make_status_image, StatusImage};
use crate::trend::MemoryTrend;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{msg_send, MainThreadMarker};
use objc2_app_kit::{
    NSControlStateValueOff, NSControlStateValueOn, NSImage, NSMenu, NSMenuDelegate, NSMenuItem,
    NSStatusItem,
};
use objc2_foundation::NSString;
use std::cell::{Cell, RefCell};

/// The status item and its dropdown. Data views skip redraws when their
/// content is unchanged, so this only remembers what AppKit would otherwise
/// redo: the menu's shape, the gauge, the tooltip, and the Settings states.
pub struct TrayController {
    status_item: Retained<NSStatusItem>,
    menu: Retained<NSMenu>,
    map_item: Retained<NSMenuItem>,
    map_view: Retained<MemoryMapView>,
    pressure_item: Retained<NSMenuItem>,
    pressure_view: Retained<PressureView>,
    history_item: Retained<NSMenuItem>,
    history_view: Retained<MemoryHistoryView>,
    legend_items: Vec<RowItem>,
    swap_item: RowItem,
    loading_item: RowItem,
    app_loading_item: RowItem,
    app_unavailable_item: RowItem,
    app_items: Vec<RowItem>,
    cpu_item: RowItem,
    gpu_item: RowItem,
    refresh_item: Retained<NSMenuItem>,
    settings_item: Retained<NSMenuItem>,
    quit_item: Retained<NSMenuItem>,
    auto_refresh_item: Retained<NSMenuItem>,
    show_app_usage_item: Retained<NSMenuItem>,
    show_cpu_item: Retained<NSMenuItem>,
    show_gpu_item: Retained<NSMenuItem>,
    launch_at_login_item: Retained<NSMenuItem>,
    pause_icon: Option<Retained<NSImage>>,
    play_icon: Option<Retained<NSImage>>,
    shape: Cell<Option<MenuShape>>,
    last_gauge: Cell<Option<(u8, MemoryTrend, MemoryPressure)>>,
    last_tooltip: RefCell<String>,
    last_settings: Cell<Option<(Settings, LaunchAtLoginStatus)>>,
}

/// A data row: a menu item carrying a `RowView`.
struct RowItem {
    item: Retained<NSMenuItem>,
    view: Retained<RowView>,
}

impl TrayController {
    pub fn new(mtm: MainThreadMarker, refresh_target: Retained<AnyObject>) -> Self {
        build::build_controller(mtm, refresh_target)
    }

    pub fn set_gauge_snapshot(
        &self,
        snapshot: MemorySnapshot,
        trend: MemoryTrend,
        mtm: MainThreadMarker,
    ) {
        let pressure = classify_pressure(snapshot.pressure_percent);
        self.set_gauge(snapshot.used_percent, trend, pressure, mtm);
        self.set_button_help(
            &gauge_tooltip(
                snapshot.used_percent,
                snapshot.used_bytes,
                snapshot.total_bytes,
            ),
            &gauge_accessibility_label(
                snapshot.used_percent,
                snapshot.used_bytes,
                snapshot.total_bytes,
            ),
            mtm,
        );
    }

    pub fn set_menu_snapshot(
        &self,
        snapshot: MemorySnapshot,
        cpu: CpuModuleState,
        gpu: GpuModuleState,
        apps: &AppMemorySnapshot,
        history: &[u64],
        mtm: MainThreadMarker,
    ) {
        let system = SystemSnapshot {
            memory: snapshot,
            cpu,
            gpu,
        };
        self.apply_model(Some(&dropdown_model(system, apps, history)), mtm);
    }

    /// Attach the open/close delegate to the main tray menu only: the root
    /// menu owns the monitor's open/close lifecycle, and opening the Settings
    /// submenu must not change it.
    pub fn set_menu_delegate(&self, delegate: &ProtocolObject<dyn NSMenuDelegate>) {
        self.menu.setDelegate(Some(delegate));
    }

    pub fn set_settings_state(&self, settings: Settings, launch_at_login: LaunchAtLoginStatus) {
        if self.last_settings.get() == Some((settings, launch_at_login)) {
            return;
        }
        set_checked(&self.auto_refresh_item, settings.auto_refresh_enabled);
        let icon = if settings.auto_refresh_enabled {
            &self.pause_icon
        } else {
            &self.play_icon
        };
        self.auto_refresh_item.setImage(icon.as_deref());
        // "Show Apps" follows the macOS convention: checked when the app list is visible.
        set_checked(&self.show_app_usage_item, settings.show_app_usage);
        set_checked(&self.show_cpu_item, settings.show_cpu);
        set_checked(&self.show_gpu_item, settings.show_gpu);
        self.launch_at_login_item
            .setTitle(&NSString::from_str(launch_at_login.menu_title()));
        set_checked(
            &self.launch_at_login_item,
            launch_at_login.should_show_checked_state(),
        );
        self.launch_at_login_item
            .setEnabled(launch_at_login.should_enable_menu_item());
        self.last_settings.set(Some((settings, launch_at_login)));
    }

    #[allow(deprecated)]
    pub fn pop_up_menu(&self) {
        self.status_item.popUpStatusItemMenu(&self.menu);
    }

    pub fn set_placeholder(&self, mtm: MainThreadMarker) {
        self.set_gauge(0, MemoryTrend::Stable, MemoryPressure::Normal, mtm);
        self.set_button_help("rami — memory unavailable", "rami, memory unavailable", mtm);
        self.apply_model(None, mtm);
    }

    /// Set the menu-bar button's hover tooltip and VoiceOver label so the current
    /// memory reading is available without opening the menu.
    fn set_button_help(&self, tooltip: &str, accessibility_label: &str, mtm: MainThreadMarker) {
        if self.last_tooltip.borrow().as_str() == tooltip {
            return;
        }
        let Some(button) = self.status_item.button(mtm) else {
            return;
        };
        let tooltip = NSString::from_str(tooltip);
        let accessibility_label = NSString::from_str(accessibility_label);
        unsafe {
            let _: () = msg_send![&*button, setToolTip: &*tooltip];
            let _: () = msg_send![&*button, setAccessibilityLabel: &*accessibility_label];
        }
        *self.last_tooltip.borrow_mut() = tooltip.to_string();
    }

    fn set_gauge(
        &self,
        percent: u8,
        trend: MemoryTrend,
        pressure: MemoryPressure,
        mtm: MainThreadMarker,
    ) {
        let gauge = (percent, trend, pressure);
        if self.last_gauge.get() == Some(gauge) {
            return;
        }
        if let Some(button) = self.status_item.button(mtm) {
            let StatusImage { image, template } =
                make_status_image(percent, trend, Accent::from(pressure));
            image.setTemplate(template);
            button.setImage(Some(&image));
            self.last_gauge.set(Some(gauge));
        }
    }

    /// Rebuild the menu if its shape changed, then hand each view its
    /// content. `None` is the Loading menu.
    fn apply_model(&self, model: Option<&DropdownModel>, mtm: MainThreadMarker) {
        let shape = menu_shape_for(model);
        if self.shape.get() != Some(shape) {
            self.rebuild_menu(shape, mtm);
            self.shape.set(Some(shape));
        }
        let Some(model) = model else {
            return;
        };
        let accent = model.accent;
        self.map_view.update(&model.map, accent);
        self.pressure_view.update(model.pressure_percent, accent);
        self.history_view
            .update(&model.history, model.total_bytes, accent);
        for (row_item, row) in self.legend_items.iter().zip(&model.breakdown) {
            row_item.view.update(&RowContent::legend(row), accent);
        }
        if let AppSectionDisplay::Rows { rows } = &model.apps {
            for (row_item, row) in self.app_items.iter().zip(rows) {
                row_item.view.update(&RowContent::stat(row), accent);
            }
        }
        for (row_item, row) in [
            (&self.swap_item, &model.swap),
            (&self.cpu_item, &model.cpu),
            (&self.gpu_item, &model.gpu),
        ] {
            if let Some(row) = row {
                row_item.view.update(&RowContent::stat(row), accent);
            }
        }
    }

    fn rebuild_menu(&self, shape: MenuShape, mtm: MainThreadMarker) {
        self.menu.removeAllItems();
        for slot in menu_slots(shape) {
            let item = match slot {
                Slot::Separator => {
                    self.menu.addItem(&NSMenuItem::separatorItem(mtm));
                    continue;
                }
                Slot::Loading => &self.loading_item.item,
                Slot::Map => &self.map_item,
                Slot::Legend(index) => &self.legend_items[index].item,
                Slot::Pressure => &self.pressure_item,
                Slot::Swap => &self.swap_item.item,
                Slot::History => &self.history_item,
                Slot::AppLoading => &self.app_loading_item.item,
                Slot::AppUnavailable => &self.app_unavailable_item.item,
                Slot::App(index) => &self.app_items[index].item,
                Slot::Cpu => &self.cpu_item.item,
                Slot::Gpu => &self.gpu_item.item,
                Slot::Refresh => &self.refresh_item,
                Slot::Settings => &self.settings_item,
                Slot::Quit => &self.quit_item,
            };
            self.menu.addItem(item);
        }
    }
}

fn set_checked(item: &NSMenuItem, checked: bool) {
    item.setState(if checked {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
}
