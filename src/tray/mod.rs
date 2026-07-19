mod layout;
mod render;
mod style;

use self::layout::{
    menu_shape_for, settings_menu_projection, AppShape, CpuShape, GpuShape, MenuShape,
};
use self::render::{
    app_row_attributed, legend_row_attributed, loading_attributed_title, make_action_icon,
    make_placeholder_icon, make_stat_item, set_row_icon, stat_row_attributed,
    unavailable_attributed_title, RowRenderCache,
};
use self::style::{
    color_for_accent, status_tint_for_pressure, APP_ROW_POOL, DEMOTED_LABEL_ALPHA, ROW_ICON_SIZE,
};
use crate::format::{
    dropdown_model_with_sections, gauge_accessibility_label, gauge_symbol_name, gauge_tooltip,
    placeholder_dropdown_model, Accent, AppSectionDisplay, CpuDisplayState, DropdownModel,
    GpuModuleDisplay, LegendRow, ModuleDisplay, RingDisplay, StatRow,
};
use crate::login_item::LaunchAtLoginStatus;
use crate::memory_view::MemoryRingsView;
use crate::model::{classify_pressure, MemoryPressure, MemorySnapshot, SystemSnapshot};
use crate::module_title_view::ModuleTitleView;
use crate::process_cpu::{ProcessCpuSnapshot, PROCESS_CPU_ROW_LIMIT};
use crate::process_memory::AppMemorySnapshot;
#[cfg(test)]
use crate::status_icon::{badge_for_state, BadgeKind};
use crate::status_icon::{make_status_image, StatusImage};
use crate::trend::MemoryTrend;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSCellImagePosition, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSEvent, NSImage,
    NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem, NSWorkspace,
};
use objc2_foundation::{NSSize, NSString};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

pub struct TrayController {
    status_item: Retained<NSStatusItem>,
    menu: Retained<NSMenu>,
    rings_item: Retained<NSMenuItem>,
    rings_view: Retained<MemoryRingsView>,
    legend_items: Vec<Retained<NSMenuItem>>,
    swap_item: Retained<NSMenuItem>,
    loading_item: Retained<NSMenuItem>,
    app_loading_item: Retained<NSMenuItem>,
    app_unavailable_item: Retained<NSMenuItem>,
    app_items: Vec<Retained<NSMenuItem>>,
    cpu_title_item: Retained<NSMenuItem>,
    cpu_loading_item: Retained<NSMenuItem>,
    cpu_unavailable_item: Retained<NSMenuItem>,
    cpu_legend_items: Vec<Retained<NSMenuItem>>,
    cpu_core_items: Vec<Retained<NSMenuItem>>,
    cpu_process_items: Vec<Retained<NSMenuItem>>,
    gpu_title_item: Retained<NSMenuItem>,
    gpu_utilization_item: Retained<NSMenuItem>,
    refresh_item: Retained<NSMenuItem>,
    auto_refresh_item: Retained<NSMenuItem>,
    show_app_usage_item: Retained<NSMenuItem>,
    show_cpu_item: Retained<NSMenuItem>,
    show_gpu_item: Retained<NSMenuItem>,
    launch_at_login_item: Retained<NSMenuItem>,
    _diagnostics_item: Retained<NSMenuItem>,
    _about_item: Retained<NSMenuItem>,
    _check_updates_item: Retained<NSMenuItem>,
    settings_item: Retained<NSMenuItem>,
    settings_menu: Retained<NSMenu>,
    quit_item: Retained<NSMenuItem>,
    pause_icon: Option<Retained<NSImage>>,
    play_icon: Option<Retained<NSImage>>,
    last_image_name: RefCell<Option<&'static str>>,
    last_trend: Cell<MemoryTrend>,
    last_pressure: Cell<MemoryPressure>,
    shape: Cell<MenuShape>,
    last_rings: RefCell<Option<[RingDisplay; 2]>>,
    last_breakdown: RefCell<Option<[LegendRow; 4]>>,
    last_accent: Cell<Accent>,
    last_swap_row: RefCell<Option<StatRow>>,
    last_app_section: RefCell<Option<AppSectionDisplay>>,
    last_cpu_state: RefCell<Option<CpuDisplayState>>,
    last_gpu: RefCell<Option<GpuModuleDisplay>>,
    last_auto_refresh_enabled: Cell<bool>,
    last_tooltip: RefCell<String>,
    last_launch_title: RefCell<String>,
    last_launch_checked: Cell<bool>,
    last_launch_enabled: Cell<bool>,
    last_show_app_usage: Cell<bool>,
    last_show_cpu: Cell<bool>,
    last_show_gpu: Cell<bool>,
    app_icon_cache: RefCell<HashMap<String, Retained<NSImage>>>,
    row_render_cache: RowRenderCache,
}

impl TrayController {
    pub fn new(mtm: MainThreadMarker, refresh_target: Retained<AnyObject>) -> Self {
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let empty = NSString::from_str("");

        let placeholder_icon = make_placeholder_icon();
        let row_render_cache = RowRenderCache::new();
        let rings_item = make_stat_item(mtm);
        let rings_view = MemoryRingsView::new(mtm);
        unsafe {
            let _: () = msg_send![&rings_item, setView: &*rings_view];
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
        app_unavailable_item
            .setAttributedTitle(Some(&unavailable_attributed_title(&row_render_cache)));
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
        cpu_unavailable_item
            .setAttributedTitle(Some(&unavailable_attributed_title(&row_render_cache)));
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

        let refresh_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Refresh"),
                Some(sel!(refreshNow:)),
                &empty,
            )
        };
        unsafe {
            refresh_item.setTarget(Some(&refresh_target));
        }
        refresh_item.setEnabled(true);
        let refresh_icon = make_action_icon("arrow.clockwise");
        if let Some(img) = &refresh_icon {
            refresh_item.setImage(Some(img));
        }

        let auto_refresh_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Auto-Refresh"),
                Some(sel!(toggleAutoRefresh:)),
                &empty,
            )
        };
        unsafe {
            auto_refresh_item.setTarget(Some(&refresh_target));
        }
        auto_refresh_item.setEnabled(true);
        auto_refresh_item.setState(NSControlStateValueOn);
        let pause_icon = make_action_icon("pause.fill");
        let play_icon = make_action_icon("play.fill");
        if let Some(img) = &pause_icon {
            auto_refresh_item.setImage(Some(img));
        }

        let show_app_usage_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Show Apps"),
                Some(sel!(toggleShowAppUsage:)),
                &empty,
            )
        };
        unsafe {
            show_app_usage_item.setTarget(Some(&refresh_target));
        }
        show_app_usage_item.setEnabled(true);
        show_app_usage_item.setState(NSControlStateValueOn);

        let show_cpu_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Show CPU"),
                Some(sel!(toggleShowCpu:)),
                &empty,
            )
        };
        unsafe {
            show_cpu_item.setTarget(Some(&refresh_target));
        }
        show_cpu_item.setEnabled(true);
        show_cpu_item.setState(NSControlStateValueOn);

        let show_gpu_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Show GPU"),
                Some(sel!(toggleShowGpu:)),
                &empty,
            )
        };
        unsafe {
            show_gpu_item.setTarget(Some(&refresh_target));
        }
        show_gpu_item.setEnabled(true);
        show_gpu_item.setState(NSControlStateValueOn);

        let launch_at_login_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(LaunchAtLoginStatus::Disabled.menu_title()),
                Some(sel!(toggleLaunchAtLogin:)),
                &empty,
            )
        };
        unsafe {
            launch_at_login_item.setTarget(Some(&refresh_target));
        }
        launch_at_login_item.setState(NSControlStateValueOff);
        launch_at_login_item.setEnabled(false);

        let diagnostics_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Copy Diagnostics"),
                Some(sel!(copyDiagnostics:)),
                &empty,
            )
        };
        unsafe {
            diagnostics_item.setTarget(Some(&refresh_target));
        }
        diagnostics_item.setEnabled(true);
        if let Some(img) = make_action_icon("doc.on.doc") {
            diagnostics_item.setImage(Some(&img));
        }

        let version = env!("CARGO_PKG_VERSION");
        let about_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&format!("rami {version}")),
                None,
                &empty,
            )
        };
        about_item.setEnabled(false);
        if let Some(img) = make_action_icon("info.circle") {
            about_item.setImage(Some(&img));
        }

        let check_updates_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Check for Updates"),
                Some(sel!(checkForUpdates:)),
                &empty,
            )
        };
        unsafe {
            check_updates_item.setTarget(Some(&refresh_target));
        }
        check_updates_item.setEnabled(true);
        if let Some(img) = make_action_icon("arrow.up.circle") {
            check_updates_item.setImage(Some(&img));
        }

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

        let settings_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Settings"),
                Some(sel!(openSettings:)),
                &empty,
            )
        };
        unsafe {
            settings_item.setTarget(Some(&refresh_target));
        }
        settings_item.setEnabled(true);
        if let Some(img) = make_action_icon("gearshape") {
            settings_item.setImage(Some(&img));
        }

        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Quit"),
                Some(sel!(terminate:)),
                &empty,
            )
        };
        quit_item.setEnabled(true);

        status_item.setMenu(Some(&menu));
        if let Some(button) = status_item.button(mtm) {
            button.setTitle(&empty);
            button.setImagePosition(NSCellImagePosition::ImageOnly);
        }

        let controller = Self {
            status_item,
            menu,
            rings_item,
            rings_view,
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
            last_show_gpu: Cell::new(true),
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

    #[allow(clippy::too_many_arguments)]
    pub fn set_menu_snapshot(
        &self,
        snapshot: MemorySnapshot,
        cpu: crate::model::CpuModuleState,
        gpu: crate::model::GpuModuleState,
        apps: &AppMemorySnapshot,
        cpu_processes: &ProcessCpuSnapshot,
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        mtm: MainThreadMarker,
    ) {
        self.apply_model(
            &dropdown_model_with_sections(
                SystemSnapshot {
                    memory: snapshot,
                    cpu,
                    gpu,
                },
                apps,
                cpu_processes,
            ),
            launch_at_login_status,
            auto_refresh_enabled,
            mtm,
        );
    }

    /// Attach the open/close delegate to the main tray menu only; the separate
    /// settings menu must not count as the live monitor menu being open.
    pub fn set_menu_delegate(&self, delegate: &ProtocolObject<dyn NSMenuDelegate>) {
        self.menu.setDelegate(Some(delegate));
    }

    pub fn set_show_app_usage(&self, enabled: bool) {
        // "Show Apps" follows the macOS convention: checked when the app list is visible.
        if self.last_show_app_usage.get() == enabled {
            return;
        }
        self.show_app_usage_item.setState(if enabled {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        self.last_show_app_usage.set(enabled);
    }

    pub fn set_show_cpu(&self, enabled: bool) {
        if self.last_show_cpu.get() == enabled {
            return;
        }
        self.show_cpu_item.setState(if enabled {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        self.last_show_cpu.set(enabled);
    }

    pub fn set_show_gpu(&self, enabled: bool) {
        if self.last_show_gpu.get() == enabled {
            return;
        }
        self.show_gpu_item.setState(if enabled {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        self.last_show_gpu.set(enabled);
    }

    pub fn set_settings_state(
        &self,
        launch_at_login: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        show_app_usage: bool,
        show_cpu: bool,
        show_gpu: bool,
    ) {
        let projection = settings_menu_projection(
            launch_at_login,
            auto_refresh_enabled,
            show_app_usage,
            show_cpu,
            show_gpu,
        );
        self.update_auto_refresh(projection.auto_refresh_enabled);
        self.set_show_app_usage(projection.show_app_usage);
        self.set_show_cpu(projection.show_cpu);
        self.set_show_gpu(projection.show_gpu);
        self.update_launch_at_login(projection.launch_at_login);
    }

    #[allow(deprecated)]
    pub fn pop_up_menu(&self) {
        self.status_item.popUpStatusItemMenu(&self.menu);
    }

    pub fn pop_up_settings_menu(&self) {
        let location = NSEvent::mouseLocation();
        self.settings_menu
            .popUpMenuPositioningItem_atLocation_inView(None, location, None);
    }

    pub fn set_placeholder(
        &self,
        launch_at_login_status: LaunchAtLoginStatus,
        mtm: MainThreadMarker,
    ) {
        self.set_gauge(0, MemoryTrend::Stable, MemoryPressure::Normal, mtm);
        self.set_button_help("rami — memory unavailable", "rami, memory unavailable", mtm);
        self.apply_model(
            &placeholder_dropdown_model(),
            launch_at_login_status,
            true,
            mtm,
        );
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
        let name = gauge_symbol_name(percent);
        let name_unchanged = *self.last_image_name.borrow() == Some(name);
        let trend_unchanged = self.last_trend.get() == trend;
        let pressure_unchanged = self.last_pressure.get() == pressure;
        if name_unchanged && trend_unchanged && pressure_unchanged {
            return;
        }

        if let Some(button) = self.status_item.button(mtm) {
            let accent = color_for_accent(Accent::from(pressure));
            match make_status_image(name, trend, &accent) {
                Some(StatusImage { image, template }) => {
                    image.setTemplate(template);
                    button.setImage(Some(&image));
                    *self.last_image_name.borrow_mut() = Some(name);
                }
                None => {
                    button.setImage(None);
                    *self.last_image_name.borrow_mut() = None;
                }
            }
            // Let the normal template gauge follow the menu bar's light/dark
            // appearance. Only warning and critical pressure force a semantic tint.
            let status_tint = status_tint_for_pressure(pressure).map(color_for_accent);
            button.setContentTintColor(status_tint.as_deref());
            self.last_trend.set(trend);
            self.last_pressure.set(pressure);
        }
    }

    fn apply_model(
        &self,
        model: &DropdownModel,
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        mtm: MainThreadMarker,
    ) {
        let new_shape = menu_shape_for(model);
        let shape_changed = self.shape.get() != new_shape;
        if shape_changed {
            self.rebuild_menu(new_shape, mtm);
            self.shape.set(new_shape);
            self.last_rings.borrow_mut().take();
            self.last_breakdown.borrow_mut().take();
            self.last_swap_row.borrow_mut().take();
            self.last_app_section.borrow_mut().take();
            self.last_cpu_state.borrow_mut().take();
            self.last_gpu.borrow_mut().take();
        }

        if let DropdownModel::Loaded { accent, modules } = model {
            let Some(ModuleDisplay::Memory(memory)) = modules.first() else {
                return;
            };
            let accent_changed = self.last_accent.get() != *accent;
            let accent_color = color_for_accent(*accent);
            if accent_changed || self.last_rings.borrow().as_ref() != Some(&memory.rings) {
                self.rings_view.update(&memory.rings, accent_color.clone());
                *self.last_rings.borrow_mut() = Some(memory.rings.clone());
            }
            if accent_changed || self.last_breakdown.borrow().as_ref() != Some(&memory.breakdown) {
                update_legend_items(
                    &self.legend_items,
                    &memory.breakdown,
                    *accent,
                    &self.row_render_cache,
                );
                *self.last_breakdown.borrow_mut() = Some(memory.breakdown.clone());
            }
            self.update_app_section(&memory.apps, &accent_color, accent_changed);
            if accent_changed || self.last_swap_row.borrow().as_ref() != memory.swap.as_ref() {
                if let Some(swap_row) = &memory.swap {
                    self.swap_item.setAttributedTitle(Some(&stat_row_attributed(
                        swap_row,
                        accent_color.clone(),
                        &self.row_render_cache,
                    )));
                }
                *self.last_swap_row.borrow_mut() = memory.swap.clone();
            }
            if let Some(cpu) = modules.iter().find_map(|module| match module {
                ModuleDisplay::Cpu(cpu) => Some(cpu),
                ModuleDisplay::Memory(_) | ModuleDisplay::Gpu(_) => None,
            }) {
                self.update_cpu_module(&cpu.state, &accent_color, *accent, accent_changed);
            }
            if let Some(gpu) = modules.iter().find_map(|module| match module {
                ModuleDisplay::Gpu(gpu) => Some(gpu),
                ModuleDisplay::Memory(_) | ModuleDisplay::Cpu(_) => None,
            }) {
                self.update_gpu_module(gpu, &accent_color, *accent, accent_changed);
            }
            self.last_accent.set(*accent);
        }

        self.update_auto_refresh(auto_refresh_enabled);
        self.update_launch_at_login(launch_at_login_status);
    }

    fn update_app_section(&self, apps: &AppSectionDisplay, accent: &NSColor, accent_changed: bool) {
        if !accent_changed && self.last_app_section.borrow().as_ref() == Some(apps) {
            return;
        }
        if let AppSectionDisplay::Rows { rows } = apps {
            for (item, row) in self.app_items.iter().zip(rows.iter()) {
                item.setAttributedTitle(Some(&app_row_attributed(
                    row,
                    accent,
                    &self.row_render_cache,
                )));
                item.setImage(self.app_row_icon(row).as_deref());
                item.setSubmenu(None);
            }
        }
        *self.last_app_section.borrow_mut() = Some(apps.clone());
    }

    fn update_cpu_module(
        &self,
        cpu: &CpuDisplayState,
        accent: &NSColor,
        accent_kind: Accent,
        accent_changed: bool,
    ) {
        if !accent_changed && self.last_cpu_state.borrow().as_ref() == Some(cpu) {
            return;
        }

        if let CpuDisplayState::Available {
            utilization,
            cores,
            processes,
        } = cpu
        {
            update_legend_items(
                &self.cpu_legend_items,
                utilization,
                accent_kind,
                &self.row_render_cache,
            );
            for (item, row) in self.cpu_core_items.iter().zip(cores) {
                // Row hierarchy (#23): per-cluster splits are derived detail,
                // demoted below the User total and the per-process rows.
                item.setAttributedTitle(Some(&stat_row_attributed(
                    row,
                    accent.colorWithAlphaComponent(DEMOTED_LABEL_ALPHA),
                    &self.row_render_cache,
                )));
                item.setImage(
                    self.row_render_cache
                        .legend_icon(accent_kind, 65)
                        .as_deref(),
                );
            }
            for (item, row) in self.cpu_process_items.iter().zip(processes) {
                item.setAttributedTitle(Some(&stat_row_attributed(
                    row,
                    accent.colorWithAlphaComponent(1.0),
                    &self.row_render_cache,
                )));
                item.setImage(
                    self.row_render_cache
                        .legend_icon(accent_kind, 35)
                        .as_deref(),
                );
            }
        }
        *self.last_cpu_state.borrow_mut() = Some(cpu.clone());
    }

    fn update_gpu_module(
        &self,
        gpu: &GpuModuleDisplay,
        accent: &NSColor,
        accent_kind: Accent,
        accent_changed: bool,
    ) {
        if !accent_changed && self.last_gpu.borrow().as_ref() == Some(gpu) {
            return;
        }

        self.gpu_utilization_item
            .setAttributedTitle(Some(&stat_row_attributed(
                &gpu.utilization,
                accent.colorWithAlphaComponent(1.0),
                &self.row_render_cache,
            )));
        self.gpu_utilization_item.setImage(
            self.row_render_cache
                .legend_icon(accent_kind, 100)
                .as_deref(),
        );
        *self.last_gpu.borrow_mut() = Some(gpu.clone());
    }

    fn rebuild_menu(&self, shape: MenuShape, mtm: MainThreadMarker) {
        self.menu.removeAllItems();
        match shape {
            MenuShape::Uninitialized => {}
            MenuShape::Loading => {
                self.menu.addItem(&self.loading_item);
            }
            MenuShape::Loaded {
                apps,
                show_swap,
                cpu,
                gpu,
            } => {
                self.menu.addItem(&self.rings_item);
                for item in &self.legend_items {
                    self.menu.addItem(item);
                }
                if show_swap {
                    self.menu.addItem(&self.swap_item);
                }
                match apps {
                    AppShape::Hidden => {}
                    AppShape::Loading => {
                        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
                        self.menu.addItem(&self.app_loading_item);
                    }
                    AppShape::Unavailable => {
                        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
                        self.menu.addItem(&self.app_unavailable_item);
                    }
                    AppShape::Rows { rows } => {
                        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
                        for item in self.app_items.iter().take(rows) {
                            self.menu.addItem(item);
                        }
                    }
                }
                match cpu {
                    CpuShape::Hidden => {}
                    CpuShape::Loading => {
                        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
                        self.menu.addItem(&self.cpu_title_item);
                        self.menu.addItem(&self.cpu_loading_item);
                    }
                    CpuShape::Unavailable => {
                        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
                        self.menu.addItem(&self.cpu_title_item);
                        self.menu.addItem(&self.cpu_unavailable_item);
                    }
                    CpuShape::Available { cores, processes } => {
                        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
                        self.menu.addItem(&self.cpu_title_item);
                        for item in &self.cpu_legend_items {
                            self.menu.addItem(item);
                        }
                        for item in self.cpu_core_items.iter().take(cores) {
                            self.menu.addItem(item);
                        }
                        for item in self.cpu_process_items.iter().take(processes) {
                            self.menu.addItem(item);
                        }
                    }
                }
                if gpu == GpuShape::Available {
                    self.menu.addItem(&NSMenuItem::separatorItem(mtm));
                    self.menu.addItem(&self.gpu_title_item);
                    self.menu.addItem(&self.gpu_utilization_item);
                }
            }
        }
        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
        self.menu.addItem(&self.refresh_item);
        self.menu.addItem(&self.settings_item);
        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
        self.menu.addItem(&self.quit_item);
    }

    fn update_auto_refresh(&self, enabled: bool) {
        if self.last_auto_refresh_enabled.get() == enabled
            && !matches!(self.shape.get(), MenuShape::Uninitialized)
        {
            return;
        }
        self.auto_refresh_item.setState(if enabled {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        let icon = if enabled {
            self.pause_icon.as_ref()
        } else {
            self.play_icon.as_ref()
        };
        self.auto_refresh_item.setImage(icon.map(|r| r.as_ref()));
        self.last_auto_refresh_enabled.set(enabled);
    }

    fn update_launch_at_login(&self, status: LaunchAtLoginStatus) {
        let title = status.menu_title();
        if self.last_launch_title.borrow().as_str() != title {
            self.launch_at_login_item
                .setTitle(&NSString::from_str(title));
            *self.last_launch_title.borrow_mut() = title.to_string();
        }
        let checked = status.should_show_checked_state();
        if self.last_launch_checked.get() != checked {
            self.launch_at_login_item.setState(if checked {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            self.last_launch_checked.set(checked);
        }
        let enabled = status.should_enable_menu_item();
        if self.last_launch_enabled.get() != enabled {
            self.launch_at_login_item.setEnabled(enabled);
            self.last_launch_enabled.set(enabled);
        }
    }

    fn app_row_icon(&self, row: &StatRow) -> Option<Retained<NSImage>> {
        let bundle_path = row.bundle_path.as_ref()?;
        if let Some(cached) = self.app_icon_cache.borrow().get(bundle_path) {
            return Some(cached.clone());
        }
        let path = NSString::from_str(bundle_path);
        let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
        image.setSize(NSSize::new(ROW_ICON_SIZE, ROW_ICON_SIZE));
        self.app_icon_cache
            .borrow_mut()
            .insert(bundle_path.clone(), image.clone());
        Some(image)
    }
}

fn update_legend_items(
    items: &[Retained<NSMenuItem>],
    rows: &[LegendRow],
    accent: Accent,
    render_cache: &RowRenderCache,
) {
    let accent_color = color_for_accent(accent);
    for (item, row) in items.iter().zip(rows) {
        item.setAttributedTitle(Some(&legend_row_attributed(
            row,
            &accent_color,
            render_cache,
        )));
        item.setImage(
            render_cache
                .legend_icon(accent, row.opacity_percent)
                .as_deref(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{badge_for_state, BadgeKind};
    use crate::trend::MemoryTrend;

    #[test]
    fn badge_uses_trend_only() {
        assert_eq!(badge_for_state(MemoryTrend::Stable), BadgeKind::None);
        assert_eq!(badge_for_state(MemoryTrend::Rising), BadgeKind::None);
        assert_eq!(
            badge_for_state(MemoryTrend::RisingFast),
            BadgeKind::RisingFast
        );
    }
}
