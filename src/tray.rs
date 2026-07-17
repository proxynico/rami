use crate::format::{
    dropdown_model_with_apps, gauge_accessibility_label, gauge_symbol_name, gauge_tooltip,
    placeholder_dropdown_model, AppSectionDisplay, DropdownModel, StatRow,
};
use crate::login_item::LaunchAtLoginStatus;
use crate::model::{classify_pressure, MemoryPressure, MemorySnapshot};
use crate::process_memory::AppMemorySnapshot;
use crate::sparkline;
#[cfg(test)]
use crate::status_icon::{badge_for_state, BadgeKind};
use crate::status_icon::{make_status_image, StatusImage};
use crate::trend::MemoryTrend;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{msg_send, sel, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSCellImagePosition, NSColor, NSControlStateValueOff, NSControlStateValueOn,
    NSEventModifierFlags, NSFont, NSFontAttributeName, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSImage, NSImageSymbolConfiguration, NSImageSymbolScale,
    NSMenu, NSMenuDelegate, NSMenuItem, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSStatusBar, NSStatusItem, NSTextAlignment, NSTextTab, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSDictionary, NSMutableAttributedString, NSSize, NSString,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppShape {
    Hidden,
    Loading,
    Unavailable,
    Rows { rows: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuShape {
    Uninitialized,
    Loading,
    Loaded { apps: AppShape, show_swap: bool },
}

pub struct TrayController {
    status_item: Retained<NSStatusItem>,
    menu: Retained<NSMenu>,
    memory_item: Retained<NSMenuItem>,
    available_item: Retained<NSMenuItem>,
    swap_item: Retained<NSMenuItem>,
    sparkline_item: Retained<NSMenuItem>,
    sparkline_view: Retained<sparkline::SparklineView>,
    loading_item: Retained<NSMenuItem>,
    app_loading_item: Retained<NSMenuItem>,
    app_unavailable_item: Retained<NSMenuItem>,
    app_items: Vec<Retained<NSMenuItem>>,
    app_quit_items: Vec<Retained<NSMenuItem>>,
    app_submenus: Vec<Retained<NSMenu>>,
    refresh_item: Retained<NSMenuItem>,
    auto_refresh_item: Retained<NSMenuItem>,
    show_app_usage_item: Retained<NSMenuItem>,
    launch_at_login_item: Retained<NSMenuItem>,
    _diagnostics_item: Retained<NSMenuItem>,
    _about_item: Retained<NSMenuItem>,
    _check_updates_item: Retained<NSMenuItem>,
    settings_item: Retained<NSMenuItem>,
    _settings_submenu: Retained<NSMenu>,
    quit_item: Retained<NSMenuItem>,
    pause_icon: Option<Retained<NSImage>>,
    play_icon: Option<Retained<NSImage>>,
    last_image_name: RefCell<Option<&'static str>>,
    last_trend: Cell<MemoryTrend>,
    last_pressure: Cell<MemoryPressure>,
    shape: Cell<MenuShape>,
    last_memory_row: RefCell<Option<StatRow>>,
    last_available_row: RefCell<Option<StatRow>>,
    last_swap_row: RefCell<Option<StatRow>>,
    last_history: RefCell<Vec<u64>>,
    last_app_section: RefCell<Option<AppSectionDisplay>>,
    last_auto_refresh_enabled: Cell<bool>,
    last_tooltip: RefCell<String>,
    last_launch_title: RefCell<String>,
    last_launch_checked: Cell<bool>,
    last_launch_enabled: Cell<bool>,
    app_icon_cache: RefCell<HashMap<String, Retained<NSImage>>>,
}

const APP_ROW_POOL: usize = 5;
const ROW_ICON_SIZE: f64 = 16.0;

impl TrayController {
    pub fn new(mtm: MainThreadMarker, refresh_target: Retained<AnyObject>) -> Self {
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let empty = NSString::from_str("");

        let placeholder_icon = make_placeholder_icon();
        let memory_item = make_stat_item(mtm);
        set_row_icon(&memory_item, "memorychip", &placeholder_icon);
        let available_item = make_stat_item(mtm);
        set_row_icon(&available_item, "tray.and.arrow.down", &placeholder_icon);
        let swap_item = make_stat_item(mtm);
        set_row_icon(&swap_item, "arrow.up.arrow.down", &placeholder_icon);
        let sparkline_item = make_stat_item(mtm);
        let sparkline_view = sparkline::SparklineView::new(mtm, Vec::new());
        unsafe {
            let _: () = msg_send![&sparkline_item, setView: &*sparkline_view];
        }
        let loading_item = make_stat_item(mtm);
        loading_item.setImage(Some(&placeholder_icon));
        loading_item.setAttributedTitle(Some(&loading_attributed_title()));
        let app_loading_item = make_stat_item(mtm);
        app_loading_item.setImage(Some(&placeholder_icon));
        app_loading_item.setAttributedTitle(Some(&loading_attributed_title()));
        let app_unavailable_item = make_stat_item(mtm);
        app_unavailable_item.setImage(Some(&placeholder_icon));
        app_unavailable_item.setAttributedTitle(Some(&unavailable_attributed_title()));
        let app_items: Vec<Retained<NSMenuItem>> =
            (0..APP_ROW_POOL).map(|_| make_stat_item(mtm)).collect();
        let app_quit_items: Vec<Retained<NSMenuItem>> = (0..APP_ROW_POOL)
            .map(|_| make_quit_app_item(mtm, &refresh_target))
            .collect();
        let app_submenus: Vec<Retained<NSMenu>> = (0..APP_ROW_POOL)
            .map(|idx| {
                let submenu = NSMenu::new(mtm);
                submenu.setAutoenablesItems(false);
                submenu.addItem(&app_quit_items[idx]);
                submenu
            })
            .collect();

        let refresh_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Refresh"),
                Some(sel!(refreshNow:)),
                &NSString::from_str("r"),
            )
        };
        unsafe {
            refresh_item.setTarget(Some(&refresh_target));
        }
        refresh_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
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

        let settings_submenu = NSMenu::new(mtm);
        settings_submenu.setAutoenablesItems(false);
        settings_submenu.addItem(&auto_refresh_item);
        settings_submenu.addItem(&show_app_usage_item);
        settings_submenu.addItem(&launch_at_login_item);
        settings_submenu.addItem(&diagnostics_item);
        settings_submenu.addItem(&NSMenuItem::separatorItem(mtm));
        settings_submenu.addItem(&check_updates_item);
        settings_submenu.addItem(&about_item);

        let settings_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Settings"),
                None,
                &empty,
            )
        };
        settings_item.setSubmenu(Some(&settings_submenu));
        settings_item.setEnabled(true);
        if let Some(img) = make_action_icon("gearshape") {
            settings_item.setImage(Some(&img));
        }

        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Quit"),
                Some(sel!(terminate:)),
                &NSString::from_str("q"),
            )
        };
        quit_item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        quit_item.setEnabled(true);

        status_item.setMenu(Some(&menu));
        if let Some(button) = status_item.button(mtm) {
            button.setTitle(&empty);
            button.setImagePosition(NSCellImagePosition::ImageOnly);
        }

        let controller = Self {
            status_item,
            menu,
            memory_item,
            available_item,
            swap_item,
            sparkline_item,
            sparkline_view,
            loading_item,
            app_loading_item,
            app_unavailable_item,
            app_items,
            app_quit_items,
            app_submenus,
            refresh_item,
            auto_refresh_item,
            show_app_usage_item,
            launch_at_login_item,
            _diagnostics_item: diagnostics_item,
            _about_item: about_item,
            _check_updates_item: check_updates_item,
            settings_item,
            _settings_submenu: settings_submenu,
            quit_item,
            pause_icon,
            play_icon,
            last_image_name: RefCell::new(None),
            last_trend: Cell::new(MemoryTrend::Stable),
            last_pressure: Cell::new(MemoryPressure::Normal),
            shape: Cell::new(MenuShape::Uninitialized),
            last_memory_row: RefCell::new(None),
            last_available_row: RefCell::new(None),
            last_swap_row: RefCell::new(None),
            last_history: RefCell::new(Vec::new()),
            last_app_section: RefCell::new(None),
            last_auto_refresh_enabled: Cell::new(true),
            last_tooltip: RefCell::new(String::new()),
            last_launch_title: RefCell::new(String::new()),
            last_launch_checked: Cell::new(false),
            last_launch_enabled: Cell::new(false),
            app_icon_cache: RefCell::new(HashMap::new()),
        };
        controller.set_gauge(0, MemoryTrend::Stable, MemoryPressure::Normal, mtm);
        controller.apply_model(
            &placeholder_dropdown_model(),
            &[],
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
        let pressure = classify_pressure(snapshot.available_bytes, snapshot.total_bytes);
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
        apps: &AppMemorySnapshot,
        history: &[u64],
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        mtm: MainThreadMarker,
    ) {
        self.apply_model(
            &dropdown_model_with_apps(snapshot, apps),
            history,
            launch_at_login_status,
            auto_refresh_enabled,
            mtm,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_snapshot(
        &self,
        snapshot: MemorySnapshot,
        trend: MemoryTrend,
        apps: &AppMemorySnapshot,
        history: &[u64],
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        mtm: MainThreadMarker,
    ) {
        self.set_gauge_snapshot(snapshot, trend, mtm);
        self.set_menu_snapshot(
            snapshot,
            apps,
            history,
            launch_at_login_status,
            auto_refresh_enabled,
            mtm,
        );
    }

    /// Attach the open/close delegate to the main tray menu only; the settings
    /// submenu opening must not count as a menu open.
    pub fn set_menu_delegate(&self, delegate: &ProtocolObject<dyn NSMenuDelegate>) {
        self.menu.setDelegate(Some(delegate));
    }

    pub fn set_show_app_usage(&self, enabled: bool) {
        // "Show Apps" follows the macOS convention: checked when the app list is visible.
        self.show_app_usage_item.setState(if enabled {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }

    #[allow(deprecated)]
    pub fn pop_up_menu(&self) {
        self.status_item.popUpStatusItemMenu(&self.menu);
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
            &[],
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
            match make_status_image(name, trend) {
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
            // Tint the template gauge to flag memory pressure: red when
            // nearly exhausted, orange when tight, otherwise the default
            // appearance. The non-template rising-fast composite uses a yellow
            // climb badge instead, so pressure tint and climb signal stay distinct.
            let tint = match pressure {
                MemoryPressure::Critical => Some(NSColor::systemRedColor()),
                MemoryPressure::Warning => Some(NSColor::systemOrangeColor()),
                MemoryPressure::Normal => None,
            };
            button.setContentTintColor(tint.as_deref());
            self.last_trend.set(trend);
            self.last_pressure.set(pressure);
        }
    }

    fn apply_model(
        &self,
        model: &DropdownModel,
        history: &[u64],
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        mtm: MainThreadMarker,
    ) {
        let new_shape = menu_shape_for(model);
        if self.shape.get() != new_shape {
            self.rebuild_menu(new_shape, mtm);
            self.shape.set(new_shape);
            self.last_memory_row.borrow_mut().take();
            self.last_available_row.borrow_mut().take();
            self.last_swap_row.borrow_mut().take();
            self.last_history.borrow_mut().clear();
            self.last_app_section.borrow_mut().take();
        }

        if let DropdownModel::Loaded {
            memory,
            available,
            apps,
            swap,
        } = model
        {
            if self.last_memory_row.borrow().as_ref() != Some(memory) {
                self.memory_item
                    .setAttributedTitle(Some(&stat_row_attributed(memory, NSColor::labelColor())));
                *self.last_memory_row.borrow_mut() = Some(memory.clone());
            }
            if self.last_available_row.borrow().as_ref() != Some(available) {
                self.available_item
                    .setAttributedTitle(Some(&stat_row_attributed(
                        available,
                        NSColor::labelColor(),
                    )));
                *self.last_available_row.borrow_mut() = Some(available.clone());
            }
            self.update_app_section(apps);
            if self.last_swap_row.borrow().as_ref() != swap.as_ref() {
                if let Some(swap_row) = swap {
                    self.swap_item.setAttributedTitle(Some(&stat_row_attributed(
                        swap_row,
                        NSColor::labelColor(),
                    )));
                }
                *self.last_swap_row.borrow_mut() = swap.clone();
            }
            if self.last_history.borrow().as_slice() != history {
                self.sparkline_view.update(history.to_vec());
                *self.last_history.borrow_mut() = history.to_vec();
            }
        }

        self.update_auto_refresh(auto_refresh_enabled);
        self.update_launch_at_login(launch_at_login_status);
    }

    fn update_app_section(&self, apps: &AppSectionDisplay) {
        if self.last_app_section.borrow().as_ref() == Some(apps) {
            return;
        }
        if let AppSectionDisplay::Rows { rows } = apps {
            for (item, row) in self.app_items.iter().zip(rows.iter()) {
                item.setAttributedTitle(Some(&app_row_attributed(row)));
                item.setImage(self.app_row_icon(row).as_deref());
            }
            for (idx, row) in rows.iter().enumerate() {
                let item = &self.app_items[idx];
                if let Some(key) = &row.quit_key {
                    let quit_item = &self.app_quit_items[idx];
                    quit_item.setTitle(&NSString::from_str(&format!("Quit {}", row.primary)));
                    // Carry the app's stable identity on the menu item so the quit handler
                    // targets the app the user saw, never a positional slot that may have shifted.
                    let key_obj = NSString::from_str(key);
                    unsafe {
                        let _: () = msg_send![&**quit_item, setRepresentedObject: &*key_obj];
                    }
                    quit_item.setEnabled(true);
                    item.setSubmenu(Some(&self.app_submenus[idx]));
                } else {
                    item.setSubmenu(None);
                }
            }
        }
        *self.last_app_section.borrow_mut() = Some(apps.clone());
    }

    fn rebuild_menu(&self, shape: MenuShape, mtm: MainThreadMarker) {
        self.menu.removeAllItems();
        match shape {
            MenuShape::Uninitialized => {}
            MenuShape::Loading => {
                self.menu.addItem(&self.loading_item);
            }
            MenuShape::Loaded { apps, show_swap } => {
                self.menu.addItem(&self.memory_item);
                self.menu.addItem(&self.available_item);
                if show_swap {
                    self.menu.addItem(&self.swap_item);
                }
                self.menu.addItem(&self.sparkline_item);
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

fn menu_shape_for(model: &DropdownModel) -> MenuShape {
    match model {
        DropdownModel::Loading => MenuShape::Loading,
        DropdownModel::Loaded { apps, swap, .. } => {
            let app_shape = match apps {
                AppSectionDisplay::Hidden => AppShape::Hidden,
                AppSectionDisplay::Loading => AppShape::Loading,
                AppSectionDisplay::Unavailable => AppShape::Unavailable,
                AppSectionDisplay::Rows { rows } => AppShape::Rows {
                    rows: rows.len().min(APP_ROW_POOL),
                },
            };
            MenuShape::Loaded {
                apps: app_shape,
                show_swap: swap.is_some(),
            }
        }
    }
}

fn make_stat_item(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(""),
            None,
            &NSString::from_str(""),
        )
    };
    item.setEnabled(true);
    item
}

fn make_quit_app_item(
    mtm: MainThreadMarker,
    refresh_target: &Retained<AnyObject>,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Quit App"),
            Some(sel!(quitApp:)),
            &NSString::from_str(""),
        )
    };
    unsafe {
        item.setTarget(Some(refresh_target));
    }
    item.setEnabled(true);
    item
}

fn app_row_attributed(row: &StatRow) -> Retained<NSAttributedString> {
    stat_row_attributed_colored(row, NSColor::labelColor(), NSColor::secondaryLabelColor())
}

const ROW_TAIL_TAB: f64 = 180.0;

fn row_paragraph_style() -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    let tail_tab = unsafe {
        NSTextTab::initWithTextAlignment_location_options(
            NSTextTab::alloc(),
            NSTextAlignment::Right,
            ROW_TAIL_TAB,
            &NSDictionary::new(),
        )
    };
    let tabs = NSArray::from_retained_slice(&[tail_tab]);
    style.setTabStops(Some(&tabs));
    style
}

fn make_placeholder_icon() -> Retained<NSImage> {
    // Transparent 16pt square so non-icon rows align with app rows that carry an icon.
    NSImage::initWithSize(NSImage::alloc(), NSSize::new(ROW_ICON_SIZE, ROW_ICON_SIZE))
}

fn make_row_icon(name: &str) -> Option<Retained<NSImage>> {
    let symbol_name = NSString::from_str(name);
    let desc = NSString::from_str("");
    let base =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
    let config = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Medium);
    let image = base.imageWithSymbolConfiguration(&config)?;
    image.setSize(NSSize::new(ROW_ICON_SIZE, ROW_ICON_SIZE));
    image.setTemplate(true);
    Some(image)
}

fn set_row_icon(item: &NSMenuItem, name: &str, fallback: &NSImage) {
    match make_row_icon(name) {
        Some(icon) => item.setImage(Some(&icon)),
        None => item.setImage(Some(fallback)),
    }
}

fn make_action_icon(name: &str) -> Option<Retained<NSImage>> {
    let desc = NSString::from_str("");
    let symbol_name = NSString::from_str(name);
    let base =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
    let config = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Small);
    let image = base.imageWithSymbolConfiguration(&config)?;
    image.setTemplate(true);
    Some(image)
}

fn stat_font() -> Retained<NSFont> {
    let weight = unsafe { NSFontWeightRegular };
    NSFont::monospacedDigitSystemFontOfSize_weight(13.0, weight)
}

fn attrs_for(
    color: Retained<NSColor>,
    font: Retained<NSFont>,
) -> Retained<NSDictionary<NSString, AnyObject>> {
    unsafe {
        let color_obj = Retained::cast_unchecked::<AnyObject>(color);
        let font_obj = Retained::cast_unchecked::<AnyObject>(font);
        NSDictionary::from_retained_objects(
            &[NSForegroundColorAttributeName, NSFontAttributeName],
            &[color_obj, font_obj],
        )
    }
}

fn stat_row_attributed(
    row: &StatRow,
    primary_color: Retained<NSColor>,
) -> Retained<NSAttributedString> {
    stat_row_attributed_colored(row, primary_color, NSColor::secondaryLabelColor())
}

fn stat_row_attributed_colored(
    row: &StatRow,
    primary_color: Retained<NSColor>,
    tail_color: Retained<NSColor>,
) -> Retained<NSAttributedString> {
    let font = stat_font();
    let primary_attrs = attrs_for(primary_color, font.clone());
    let primary_str = NSString::from_str(&row.primary);
    let primary = unsafe { NSAttributedString::new_with_attributes(&primary_str, &primary_attrs) };

    let Some(tail) = &row.tail else {
        return primary;
    };

    let result = NSMutableAttributedString::new();
    result.appendAttributedString(&primary);

    let separator_attrs = attrs_for(NSColor::secondaryLabelColor(), font.clone());
    let separator_str = NSString::from_str("\t");
    let separator =
        unsafe { NSAttributedString::new_with_attributes(&separator_str, &separator_attrs) };
    result.appendAttributedString(&separator);

    // Split tail on \t so the delta keeps its own color (orange) while sharing
    // a single right-aligned tail column with the footprint. No reserved delta
    // gutter: the row's right edge collapses when nothing is rising.
    let mut tail_parts = tail.splitn(2, '\t');
    let footprint_part = tail_parts.next().unwrap_or("");
    let delta_part = tail_parts.next();

    let tail_attrs = attrs_for(tail_color.clone(), font.clone());
    let footprint_str = NSString::from_str(footprint_part);
    let footprint = unsafe { NSAttributedString::new_with_attributes(&footprint_str, &tail_attrs) };
    result.appendAttributedString(&footprint);

    if let Some(delta) = delta_part {
        let gap_str = NSString::from_str("  ");
        let gap = unsafe { NSAttributedString::new_with_attributes(&gap_str, &tail_attrs) };
        result.appendAttributedString(&gap);

        let delta_attrs = attrs_for(NSColor::systemOrangeColor(), font.clone());
        let delta_str = NSString::from_str(delta);
        let delta_attr =
            unsafe { NSAttributedString::new_with_attributes(&delta_str, &delta_attrs) };
        result.appendAttributedString(&delta_attr);
    }

    apply_paragraph_style(&result);
    Retained::into_super(result)
}

fn apply_paragraph_style(s: &NSMutableAttributedString) {
    let style = row_paragraph_style();
    let style_obj = unsafe { Retained::cast_unchecked::<AnyObject>(style) };
    let range = objc2_foundation::NSRange {
        location: 0,
        length: s.length(),
    };
    unsafe {
        s.addAttribute_value_range(NSParagraphStyleAttributeName, &style_obj, range);
    }
}

fn loading_attributed_title() -> Retained<NSAttributedString> {
    stat_row_attributed(
        &StatRow {
            primary: "Loading…".to_string(),
            tail: None,
            quit_key: None,
            bundle_path: None,
        },
        NSColor::secondaryLabelColor(),
    )
}

fn unavailable_attributed_title() -> Retained<NSAttributedString> {
    stat_row_attributed(
        &StatRow {
            primary: "Unavailable".to_string(),
            tail: None,
            quit_key: None,
            bundle_path: None,
        },
        NSColor::secondaryLabelColor(),
    )
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MenuEntry<'a> {
    Stat {
        primary: &'a str,
        tail: Option<&'a str>,
        is_high: bool,
    },
    Sparkline,
    Loading,
    AppLoading,
    AppUnavailable,
    AppRow {
        primary: &'a str,
        tail: Option<&'a str>,
        quit_key: Option<&'a str>,
    },
    Separator,
    Refresh,
    Settings {
        auto_refresh_enabled: bool,
        show_app_usage: bool,
        launch_at_login: LaunchAtLoginStatus,
    },
    Quit,
}

#[cfg(test)]
pub(crate) fn loaded_menu_entries<'a>(
    model: &'a DropdownModel,
    launch_at_login_status: LaunchAtLoginStatus,
    auto_refresh_enabled: bool,
) -> Vec<MenuEntry<'a>> {
    loaded_menu_entries_with_app_usage(model, launch_at_login_status, auto_refresh_enabled, false)
}

#[cfg(test)]
pub(crate) fn loaded_menu_entries_with_app_usage<'a>(
    model: &'a DropdownModel,
    launch_at_login_status: LaunchAtLoginStatus,
    auto_refresh_enabled: bool,
    show_app_usage: bool,
) -> Vec<MenuEntry<'a>> {
    let mut entries = Vec::new();
    match model {
        DropdownModel::Loading => {
            entries.push(MenuEntry::Loading);
        }
        DropdownModel::Loaded {
            memory,
            available,
            apps,
            swap,
        } => {
            entries.push(MenuEntry::Stat {
                primary: &memory.primary,
                tail: memory.tail.as_deref(),
                is_high: false,
            });
            entries.push(MenuEntry::Stat {
                primary: &available.primary,
                tail: available.tail.as_deref(),
                is_high: false,
            });
            if let Some(swap) = swap {
                entries.push(MenuEntry::Stat {
                    primary: &swap.primary,
                    tail: swap.tail.as_deref(),
                    is_high: false,
                });
            }
            entries.push(MenuEntry::Sparkline);
            match apps {
                AppSectionDisplay::Hidden => {}
                AppSectionDisplay::Loading => {
                    entries.push(MenuEntry::Separator);
                    entries.push(MenuEntry::AppLoading);
                }
                AppSectionDisplay::Unavailable => {
                    entries.push(MenuEntry::Separator);
                    entries.push(MenuEntry::AppUnavailable);
                }
                AppSectionDisplay::Rows { rows } => {
                    entries.push(MenuEntry::Separator);
                    for row in rows.iter().take(APP_ROW_POOL) {
                        entries.push(MenuEntry::AppRow {
                            primary: &row.primary,
                            tail: row.tail.as_deref(),
                            quit_key: row.quit_key.as_deref(),
                        });
                    }
                }
            }
        }
    }
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Refresh);
    entries.push(MenuEntry::Settings {
        auto_refresh_enabled,
        show_app_usage,
        launch_at_login: launch_at_login_status,
    });
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Quit);
    entries
}

#[cfg(test)]
mod tests {
    use super::{badge_for_state, loaded_menu_entries, BadgeKind, MenuEntry};
    use crate::format::{dropdown_model, dropdown_model_with_apps, placeholder_dropdown_model};
    use crate::login_item::LaunchAtLoginStatus;
    use crate::model::MemorySnapshot;
    use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};
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

    fn snapshot() -> MemorySnapshot {
        MemorySnapshot {
            used_bytes: 6_120_328_397,
            total_bytes: 17_179_869_184,
            used_percent: 47,
            swap_used_bytes: 1_288_490_189,
            available_bytes: 11_055_540_777,
        }
    }

    #[test]
    fn loading_layout_omits_memory_detail_sections() {
        let model = placeholder_dropdown_model();
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert_eq!(
            entries,
            vec![
                MenuEntry::Loading,
                MenuEntry::Separator,
                MenuEntry::Refresh,
                MenuEntry::Settings {
                    auto_refresh_enabled: true,
                    show_app_usage: false,
                    launch_at_login: LaunchAtLoginStatus::Disabled,
                },
                MenuEntry::Separator,
                MenuEntry::Quit,
            ]
        );
    }

    #[test]
    fn loaded_layout_renders_memory_and_swap_rows() {
        let snapshot = MemorySnapshot {
            used_bytes: 6_120_328_397,
            total_bytes: 17_179_869_184,
            used_percent: 47,
            swap_used_bytes: 1_288_490_189,
            available_bytes: 11_055_540_777,
        };
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Enabled, true);
        assert_eq!(
            entries,
            vec![
                MenuEntry::Stat {
                    primary: "5.7 / 16.0 GB",
                    tail: Some("47%"),
                    is_high: false,
                },
                MenuEntry::Stat {
                    primary: "Available",
                    tail: Some("10.3 GB"),
                    is_high: false,
                },
                MenuEntry::Stat {
                    primary: "Swap",
                    tail: Some("1.2 GB"),
                    is_high: false,
                },
                MenuEntry::Sparkline,
                MenuEntry::Separator,
                MenuEntry::Refresh,
                MenuEntry::Settings {
                    auto_refresh_enabled: true,
                    show_app_usage: false,
                    launch_at_login: LaunchAtLoginStatus::Enabled,
                },
                MenuEntry::Separator,
                MenuEntry::Quit,
            ]
        );
    }

    #[test]
    fn loaded_layout_hides_swap_row_when_zero() {
        let snapshot = MemorySnapshot {
            used_bytes: 6_120_328_397,
            total_bytes: 17_179_869_184,
            used_percent: 47,
            swap_used_bytes: 0,
            available_bytes: 11_055_540_777,
        };
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert!(!entries.iter().any(|e| matches!(
            e,
            MenuEntry::Stat {
                primary: "Swap",
                ..
            }
        )));
    }

    #[test]
    fn loaded_with_apps_hidden_omits_apps_section() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Hidden);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert!(!entries.iter().any(|e| matches!(
            e,
            MenuEntry::AppRow { .. } | MenuEntry::AppLoading | MenuEntry::AppUnavailable
        )));
    }

    #[test]
    fn loaded_with_apps_loading_renders_loading_row() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Loading);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert_eq!(entries[4], MenuEntry::Separator);
        assert_eq!(entries[5], MenuEntry::AppLoading);
    }

    #[test]
    fn loaded_with_apps_unavailable_renders_one_row() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Unavailable);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert_eq!(entries[4], MenuEntry::Separator);
        assert_eq!(entries[5], MenuEntry::AppUnavailable);
    }

    #[test]
    fn show_app_usage_state_reflects_toggle() {
        use super::loaded_menu_entries_with_app_usage;
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Hidden);
        let on =
            loaded_menu_entries_with_app_usage(&model, LaunchAtLoginStatus::Disabled, true, true);
        assert!(on.iter().any(|e| matches!(
            e,
            MenuEntry::Settings {
                show_app_usage: true,
                ..
            }
        )));

        let off =
            loaded_menu_entries_with_app_usage(&model, LaunchAtLoginStatus::Disabled, true, false);
        assert!(off.iter().any(|e| matches!(
            e,
            MenuEntry::Settings {
                show_app_usage: false,
                ..
            }
        )));
    }

    #[test]
    fn loaded_with_apps_rows_follow_memory_section() {
        let usage = vec![
            AppMemoryUsage {
                name: "Cursor".to_string(),
                group_key: "/Applications/Cursor.app".to_string(),
                footprint_bytes: 2_147_483_648,
                pids: vec![1],
                can_quit: true,
                delta_bytes: None,
            },
            AppMemoryUsage {
                name: "Chrome".to_string(),
                group_key: "/Applications/Chrome.app".to_string(),
                footprint_bytes: 1_288_490_189,
                pids: vec![2],
                can_quit: true,
                delta_bytes: None,
            },
        ];
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Loaded(usage));
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);

        // Memory, Available, Swap, Sparkline, separator, two app rows, separator.
        assert!(matches!(
            entries[0],
            MenuEntry::Stat {
                primary: "5.7 / 16.0 GB",
                ..
            }
        ));
        assert!(matches!(
            entries[1],
            MenuEntry::Stat {
                primary: "Available",
                ..
            }
        ));
        assert!(matches!(
            entries[2],
            MenuEntry::Stat {
                primary: "Swap",
                ..
            }
        ));
        assert_eq!(entries[3], MenuEntry::Sparkline);
        assert_eq!(entries[4], MenuEntry::Separator);
        assert_eq!(
            entries[5],
            MenuEntry::AppRow {
                primary: "Cursor",
                tail: Some("2.0 GB"),
                quit_key: Some("/Applications/Cursor.app"),
            }
        );
        assert_eq!(
            entries[6],
            MenuEntry::AppRow {
                primary: "Chrome",
                tail: Some("1.2 GB"),
                quit_key: Some("/Applications/Chrome.app"),
            }
        );
        assert_eq!(entries[7], MenuEntry::Separator);
    }
}
