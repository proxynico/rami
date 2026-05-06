use crate::format::{
    dropdown_model_with_apps_and_trend, gauge_symbol_name, placeholder_dropdown_model,
    AppSectionDisplay, DropdownModel, PressureDisplay, StatRow,
};
use crate::login_item::LaunchAtLoginStatus;
use crate::model::{MemoryPressure, MemorySnapshot};
use crate::process_memory::AppMemorySnapshot;
use crate::trend::MemoryTrend;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSCellImagePosition, NSColor, NSCompositingOperation, NSControlStateValueOff,
    NSControlStateValueOn, NSEventModifierFlags, NSFont, NSFontAttributeName, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSImage, NSImageSymbolConfiguration, NSImageSymbolScale,
    NSMenu, NSMenuItem, NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSStatusBar,
    NSStatusItem, NSTextAlignment, NSTextTab, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSDictionary, NSMutableAttributedString, NSPoint, NSRect, NSSize,
    NSString,
};
use std::cell::{Cell, RefCell};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppShape {
    Hidden,
    Loading,
    Unavailable,
    Rows { rows: usize, culprit: bool },
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
    memory_section: Retained<NSMenuItem>,
    apps_section: Retained<NSMenuItem>,
    memory_item: Retained<NSMenuItem>,
    pressure_item: Retained<NSMenuItem>,
    swap_item: Retained<NSMenuItem>,
    loading_item: Retained<NSMenuItem>,
    app_loading_item: Retained<NSMenuItem>,
    app_unavailable_item: Retained<NSMenuItem>,
    app_culprit_item: Retained<NSMenuItem>,
    app_items: Vec<Retained<NSMenuItem>>,
    app_quit_items: Vec<Retained<NSMenuItem>>,
    app_submenus: Vec<Retained<NSMenu>>,
    refresh_item: Retained<NSMenuItem>,
    auto_refresh_item: Retained<NSMenuItem>,
    show_app_usage_item: Retained<NSMenuItem>,
    launch_at_login_item: Retained<NSMenuItem>,
    quit_item: Retained<NSMenuItem>,
    pause_icon: Option<Retained<NSImage>>,
    play_icon: Option<Retained<NSImage>>,
    last_image_name: RefCell<Option<&'static str>>,
    last_pressure: Cell<MemoryPressure>,
    last_trend: Cell<MemoryTrend>,
    shape: Cell<MenuShape>,
    last_memory_row: RefCell<Option<StatRow>>,
    last_pressure_display: RefCell<Option<PressureDisplay>>,
    last_swap_row: RefCell<Option<StatRow>>,
    last_app_section: RefCell<Option<AppSectionDisplay>>,
    last_auto_refresh_enabled: Cell<bool>,
    last_launch_title: RefCell<String>,
    last_launch_checked: Cell<bool>,
    last_launch_enabled: Cell<bool>,
}

const APP_ROW_POOL: usize = 5;

impl TrayController {
    pub fn new(mtm: MainThreadMarker, refresh_target: Retained<AnyObject>) -> Self {
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let empty = NSString::from_str("");

        let memory_section = NSMenuItem::sectionHeaderWithTitle(&NSString::from_str("Memory"), mtm);
        let apps_section = NSMenuItem::sectionHeaderWithTitle(&NSString::from_str("Apps"), mtm);

        let memory_item = make_stat_item(mtm);
        let pressure_item = make_stat_item(mtm);
        let swap_item = make_stat_item(mtm);
        let loading_item = make_stat_item(mtm);
        loading_item.setAttributedTitle(Some(&loading_attributed_title()));
        let app_loading_item = make_stat_item(mtm);
        app_loading_item.setAttributedTitle(Some(&loading_attributed_title()));
        let app_unavailable_item = make_stat_item(mtm);
        app_unavailable_item.setAttributedTitle(Some(&unavailable_attributed_title()));
        let app_culprit_item = make_stat_item(mtm);
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
                &NSString::from_str("Show App Usage"),
                Some(sel!(toggleShowAppUsage:)),
                &empty,
            )
        };
        unsafe {
            show_app_usage_item.setTarget(Some(&refresh_target));
        }
        show_app_usage_item.setEnabled(true);
        show_app_usage_item.setState(NSControlStateValueOff);

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
            memory_section,
            apps_section,
            memory_item,
            pressure_item,
            swap_item,
            loading_item,
            app_loading_item,
            app_unavailable_item,
            app_culprit_item,
            app_items,
            app_quit_items,
            app_submenus,
            refresh_item,
            auto_refresh_item,
            show_app_usage_item,
            launch_at_login_item,
            quit_item,
            pause_icon,
            play_icon,
            last_image_name: RefCell::new(None),
            last_pressure: Cell::new(MemoryPressure::Normal),
            last_trend: Cell::new(MemoryTrend::Stable),
            shape: Cell::new(MenuShape::Uninitialized),
            last_memory_row: RefCell::new(None),
            last_pressure_display: RefCell::new(None),
            last_swap_row: RefCell::new(None),
            last_app_section: RefCell::new(None),
            last_auto_refresh_enabled: Cell::new(true),
            last_launch_title: RefCell::new(String::new()),
            last_launch_checked: Cell::new(false),
            last_launch_enabled: Cell::new(false),
        };
        controller.set_gauge(0, MemoryPressure::Normal, MemoryTrend::Stable, mtm);
        controller.apply_model(
            &placeholder_dropdown_model(),
            LaunchAtLoginStatus::Disabled,
            true,
            mtm,
        );
        controller
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_snapshot(
        &self,
        snapshot: MemorySnapshot,
        trend: MemoryTrend,
        apps: &AppMemorySnapshot,
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        mtm: MainThreadMarker,
    ) {
        self.set_gauge(snapshot.used_percent, snapshot.pressure, trend, mtm);
        self.apply_model(
            &dropdown_model_with_apps_and_trend(snapshot, trend, apps),
            launch_at_login_status,
            auto_refresh_enabled,
            mtm,
        );
    }

    pub fn set_show_app_usage(&self, enabled: bool) {
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
        self.set_gauge(0, MemoryPressure::Normal, MemoryTrend::Stable, mtm);
        self.apply_model(
            &placeholder_dropdown_model(),
            launch_at_login_status,
            true,
            mtm,
        );
    }

    fn set_gauge(
        &self,
        percent: u8,
        pressure: MemoryPressure,
        trend: MemoryTrend,
        mtm: MainThreadMarker,
    ) {
        let name = gauge_symbol_name(percent);
        let name_unchanged = *self.last_image_name.borrow() == Some(name);
        let pressure_unchanged = self.last_pressure.get() == pressure;
        let trend_unchanged = self.last_trend.get() == trend;
        if name_unchanged && pressure_unchanged && trend_unchanged {
            return;
        }

        if let Some(button) = self.status_item.button(mtm) {
            let warning = matches!(pressure, MemoryPressure::High);
            match make_status_image(name, pressure, trend) {
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
            if warning {
                button.setContentTintColor(Some(&NSColor::systemRedColor()));
            } else {
                button.setContentTintColor(None);
            }
            self.last_pressure.set(pressure);
            self.last_trend.set(trend);
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
        if self.shape.get() != new_shape {
            self.rebuild_menu(new_shape, mtm);
            self.shape.set(new_shape);
            self.last_memory_row.borrow_mut().take();
            self.last_pressure_display.borrow_mut().take();
            self.last_swap_row.borrow_mut().take();
            self.last_app_section.borrow_mut().take();
        }

        if let DropdownModel::Loaded {
            memory,
            apps,
            pressure,
            swap,
        } = model
        {
            if self.last_memory_row.borrow().as_ref() != Some(memory) {
                self.memory_item
                    .setAttributedTitle(Some(&stat_row_attributed(memory, NSColor::labelColor())));
                *self.last_memory_row.borrow_mut() = Some(memory.clone());
            }
            self.update_app_section(apps);
            if self.last_pressure_display.borrow().as_ref() != Some(pressure) {
                self.pressure_item
                    .setAttributedTitle(Some(&pressure_attributed(pressure)));
                *self.last_pressure_display.borrow_mut() = Some(pressure.clone());
            }
            if self.last_swap_row.borrow().as_ref() != swap.as_ref() {
                if let Some(swap_row) = swap {
                    self.swap_item.setAttributedTitle(Some(&stat_row_attributed(
                        swap_row,
                        NSColor::labelColor(),
                    )));
                }
                *self.last_swap_row.borrow_mut() = swap.clone();
            }
        }

        self.update_auto_refresh(auto_refresh_enabled);
        self.update_launch_at_login(launch_at_login_status);
        self.status_item.setMenu(Some(&self.menu));
    }

    fn update_app_section(&self, apps: &AppSectionDisplay) {
        if self.last_app_section.borrow().as_ref() == Some(apps) {
            return;
        }
        if let AppSectionDisplay::Rows { culprit, rows } = apps {
            if let Some(culprit) = culprit {
                self.app_culprit_item
                    .setAttributedTitle(Some(&culprit_attributed(culprit)));
            }
            for (item, row) in self.app_items.iter().zip(rows.iter()) {
                item.setAttributedTitle(Some(&app_row_attributed(row)));
                item.setImage(app_row_icon(row).as_deref());
            }
            for (idx, row) in rows.iter().enumerate() {
                let item = &self.app_items[idx];
                if let Some(tag) = row.action_tag {
                    let quit_item = &self.app_quit_items[idx];
                    quit_item.setTitle(&NSString::from_str(&format!("Quit {}", row.primary)));
                    quit_item.setTag(tag as isize);
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
                self.menu.addItem(&self.memory_section);
                self.menu.addItem(&self.loading_item);
            }
            MenuShape::Loaded { apps, show_swap } => {
                self.menu.addItem(&self.memory_section);
                self.menu.addItem(&self.memory_item);
                self.menu.addItem(&self.pressure_item);
                if show_swap {
                    self.menu.addItem(&self.swap_item);
                }
                match apps {
                    AppShape::Hidden => {}
                    AppShape::Loading => {
                        self.menu.addItem(&self.apps_section);
                        self.menu.addItem(&self.app_loading_item);
                    }
                    AppShape::Unavailable => {
                        self.menu.addItem(&self.apps_section);
                        self.menu.addItem(&self.app_unavailable_item);
                    }
                    AppShape::Rows { rows, culprit } => {
                        self.menu.addItem(&self.apps_section);
                        if culprit {
                            self.menu.addItem(&self.app_culprit_item);
                        }
                        for item in self.app_items.iter().take(rows) {
                            self.menu.addItem(item);
                        }
                    }
                }
            }
        }
        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
        self.menu.addItem(&self.refresh_item);
        self.menu.addItem(&self.auto_refresh_item);
        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
        self.menu.addItem(&self.show_app_usage_item);
        self.menu.addItem(&self.launch_at_login_item);
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
}

fn menu_shape_for(model: &DropdownModel) -> MenuShape {
    match model {
        DropdownModel::Loading => MenuShape::Loading,
        DropdownModel::Loaded { apps, swap, .. } => {
            let app_shape = match apps {
                AppSectionDisplay::Hidden => AppShape::Hidden,
                AppSectionDisplay::Loading => AppShape::Loading,
                AppSectionDisplay::Unavailable => AppShape::Unavailable,
                AppSectionDisplay::Rows { culprit, rows } => AppShape::Rows {
                    rows: rows.len().min(APP_ROW_POOL),
                    culprit: culprit.is_some(),
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
            Some(sel!(quitAppAtIndex:)),
            &NSString::from_str(""),
        )
    };
    unsafe {
        item.setTarget(Some(refresh_target));
    }
    item.setEnabled(true);
    item
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BadgeKind {
    None,
    Rising,
    RisingFast,
    Elevated,
    High,
}

fn badge_for_state(pressure: MemoryPressure, trend: MemoryTrend) -> BadgeKind {
    match pressure {
        MemoryPressure::High => BadgeKind::High,
        MemoryPressure::Elevated => BadgeKind::Elevated,
        MemoryPressure::Normal => match trend {
            MemoryTrend::RisingFast => BadgeKind::RisingFast,
            MemoryTrend::Rising => BadgeKind::Rising,
            MemoryTrend::Stable => BadgeKind::None,
        },
    }
}

struct StatusImage {
    image: Retained<NSImage>,
    template: bool,
}

fn make_status_image(
    gauge_name: &'static str,
    pressure: MemoryPressure,
    trend: MemoryTrend,
) -> Option<StatusImage> {
    let badge = badge_for_state(pressure, trend);
    let base_template = render_template_symbol(gauge_name, NSImageSymbolScale::Large)?;
    match badge {
        BadgeKind::None => Some(StatusImage {
            image: base_template,
            template: true,
        }),
        BadgeKind::Rising => {
            let badge_image =
                render_template_symbol("arrow.up.right.circle.fill", NSImageSymbolScale::Small)?;
            let composite = compose_with_badge(&base_template, &badge_image)?;
            Some(StatusImage {
                image: composite,
                template: true,
            })
        }
        BadgeKind::High => {
            let badge_image =
                render_template_symbol("exclamationmark.triangle.fill", NSImageSymbolScale::Small)?;
            let composite = compose_with_badge(&base_template, &badge_image)?;
            Some(StatusImage {
                image: composite,
                template: true,
            })
        }
        BadgeKind::RisingFast => {
            let label = NSColor::labelColor();
            let orange = NSColor::systemOrangeColor();
            let base_colored =
                render_colored_symbol(gauge_name, NSImageSymbolScale::Large, &label)?;
            let badge_image = render_colored_symbol(
                "arrow.up.right.circle.fill",
                NSImageSymbolScale::Small,
                &orange,
            )?;
            let composite = compose_with_badge(&base_colored, &badge_image)?;
            Some(StatusImage {
                image: composite,
                template: false,
            })
        }
        BadgeKind::Elevated => {
            let label = NSColor::labelColor();
            let orange = NSColor::systemOrangeColor();
            let base_colored =
                render_colored_symbol(gauge_name, NSImageSymbolScale::Large, &label)?;
            let badge_image = render_colored_symbol(
                "exclamationmark.circle.fill",
                NSImageSymbolScale::Small,
                &orange,
            )?;
            let composite = compose_with_badge(&base_colored, &badge_image)?;
            Some(StatusImage {
                image: composite,
                template: false,
            })
        }
    }
}

fn render_template_symbol(name: &str, scale: NSImageSymbolScale) -> Option<Retained<NSImage>> {
    let symbol_name = NSString::from_str(name);
    let desc = NSString::from_str("");
    let base =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
    let config = NSImageSymbolConfiguration::configurationWithScale(scale);
    base.imageWithSymbolConfiguration(&config)
}

fn render_colored_symbol(
    name: &str,
    scale: NSImageSymbolScale,
    color: &NSColor,
) -> Option<Retained<NSImage>> {
    let symbol_name = NSString::from_str(name);
    let desc = NSString::from_str("");
    let base =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
    let scale_config = NSImageSymbolConfiguration::configurationWithScale(scale);
    let color_config = NSImageSymbolConfiguration::configurationWithHierarchicalColor(color);
    let combined = scale_config.configurationByApplyingConfiguration(&color_config);
    base.imageWithSymbolConfiguration(&combined)
}

fn compose_with_badge(base: &NSImage, badge: &NSImage) -> Option<Retained<NSImage>> {
    let size = base.size();
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    let composite = NSImage::initWithSize(NSImage::alloc(), size);
    let full_rect = NSRect::new(NSPoint::ZERO, size);
    let zero_rect = NSRect::ZERO;
    #[allow(deprecated)]
    composite.lockFocus();
    base.drawInRect_fromRect_operation_fraction(
        full_rect,
        zero_rect,
        NSCompositingOperation::SourceOver,
        1.0,
    );
    let badge_extent = (size.height * 0.65).min(size.width);
    let badge_rect = NSRect::new(
        NSPoint::new(size.width - badge_extent, 0.0),
        NSSize::new(badge_extent, badge_extent),
    );
    badge.drawInRect_fromRect_operation_fraction(
        badge_rect,
        zero_rect,
        NSCompositingOperation::SourceOver,
        1.0,
    );
    #[allow(deprecated)]
    composite.unlockFocus();
    Some(composite)
}

fn app_row_icon(row: &StatRow) -> Option<Retained<NSImage>> {
    let bundle_path = row.bundle_path.as_ref()?;
    let path = NSString::from_str(bundle_path);
    let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
    image.setSize(NSSize::new(16.0, 16.0));
    Some(image)
}

fn app_row_attributed(row: &StatRow) -> Retained<NSAttributedString> {
    stat_row_attributed_colored(row, NSColor::labelColor(), NSColor::secondaryLabelColor())
}

const ROW_TAIL_TAB: f64 = 260.0;

fn tail_paragraph_style() -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    let tab_stop = unsafe {
        NSTextTab::initWithTextAlignment_location_options(
            NSTextTab::alloc(),
            NSTextAlignment::Right,
            ROW_TAIL_TAB,
            &NSDictionary::new(),
        )
    };
    let tabs = NSArray::from_retained_slice(&[tab_stop]);
    style.setTabStops(Some(&tabs));
    style
}

fn attrs_for_with_paragraph(
    color: Retained<NSColor>,
    font: Retained<NSFont>,
    paragraph: Retained<NSMutableParagraphStyle>,
) -> Retained<NSDictionary<NSString, AnyObject>> {
    unsafe {
        let color_obj = Retained::cast_unchecked::<AnyObject>(color);
        let font_obj = Retained::cast_unchecked::<AnyObject>(font);
        let paragraph_obj = Retained::cast_unchecked::<AnyObject>(paragraph);
        NSDictionary::from_retained_objects(
            &[
                NSForegroundColorAttributeName,
                NSFontAttributeName,
                NSParagraphStyleAttributeName,
            ],
            &[color_obj, font_obj, paragraph_obj],
        )
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

    let tail_attrs = attrs_for_with_paragraph(tail_color, font, tail_paragraph_style());
    let tail_str = NSString::from_str(tail);
    let tail_attr = unsafe { NSAttributedString::new_with_attributes(&tail_str, &tail_attrs) };
    result.appendAttributedString(&tail_attr);

    Retained::into_super(result)
}

fn pressure_attributed(display: &PressureDisplay) -> Retained<NSAttributedString> {
    let tail_color = if display.is_high {
        NSColor::systemRedColor()
    } else if display.is_elevated {
        NSColor::systemOrangeColor()
    } else {
        NSColor::secondaryLabelColor()
    };
    stat_row_attributed_colored(
        &StatRow {
            primary: "Pressure".to_string(),
            tail: Some(display.text.clone()),
            action_tag: None,
            bundle_path: None,
        },
        NSColor::labelColor(),
        tail_color,
    )
}

fn culprit_attributed(row: &StatRow) -> Retained<NSAttributedString> {
    stat_row_attributed_colored(
        row,
        NSColor::secondaryLabelColor(),
        NSColor::secondaryLabelColor(),
    )
}

fn loading_attributed_title() -> Retained<NSAttributedString> {
    stat_row_attributed(
        &StatRow {
            primary: "Loading…".to_string(),
            tail: None,
            action_tag: None,
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
            action_tag: None,
            bundle_path: None,
        },
        NSColor::secondaryLabelColor(),
    )
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MenuEntry<'a> {
    SectionHeader(&'a str),
    Stat {
        primary: &'a str,
        tail: Option<&'a str>,
        is_high: bool,
    },
    Loading,
    AppLoading,
    AppUnavailable,
    AppRow {
        primary: &'a str,
        tail: Option<&'a str>,
        action_tag: Option<usize>,
    },
    Separator,
    Refresh,
    AutoRefresh {
        enabled: bool,
    },
    ShowAppUsage {
        enabled: bool,
    },
    LaunchAtLogin(LaunchAtLoginStatus),
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
            entries.push(MenuEntry::SectionHeader("Memory"));
            entries.push(MenuEntry::Loading);
        }
        DropdownModel::Loaded {
            memory,
            apps,
            pressure,
            swap,
        } => {
            entries.push(MenuEntry::SectionHeader("Memory"));
            entries.push(MenuEntry::Stat {
                primary: &memory.primary,
                tail: memory.tail.as_deref(),
                is_high: false,
            });
            entries.push(MenuEntry::Stat {
                primary: "Pressure",
                tail: Some(pressure.text.as_str()),
                is_high: pressure.is_high,
            });
            if let Some(swap) = swap {
                entries.push(MenuEntry::Stat {
                    primary: &swap.primary,
                    tail: swap.tail.as_deref(),
                    is_high: false,
                });
            }
            match apps {
                AppSectionDisplay::Hidden => {}
                AppSectionDisplay::Loading => {
                    entries.push(MenuEntry::SectionHeader("Apps"));
                    entries.push(MenuEntry::AppLoading);
                }
                AppSectionDisplay::Unavailable => {
                    entries.push(MenuEntry::SectionHeader("Apps"));
                    entries.push(MenuEntry::AppUnavailable);
                }
                AppSectionDisplay::Rows { culprit, rows } => {
                    entries.push(MenuEntry::SectionHeader("Apps"));
                    if let Some(culprit) = culprit {
                        entries.push(MenuEntry::AppRow {
                            primary: &culprit.primary,
                            tail: culprit.tail.as_deref(),
                            action_tag: None,
                        });
                    }
                    for row in rows.iter().take(APP_ROW_POOL) {
                        entries.push(MenuEntry::AppRow {
                            primary: &row.primary,
                            tail: row.tail.as_deref(),
                            action_tag: row.action_tag,
                        });
                    }
                }
            }
        }
    }
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Refresh);
    entries.push(MenuEntry::AutoRefresh {
        enabled: auto_refresh_enabled,
    });
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::ShowAppUsage {
        enabled: show_app_usage,
    });
    entries.push(MenuEntry::LaunchAtLogin(launch_at_login_status));
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Quit);
    entries
}

#[cfg(test)]
mod tests {
    use super::{badge_for_state, loaded_menu_entries, BadgeKind, MenuEntry};
    use crate::format::{dropdown_model, dropdown_model_with_apps, placeholder_dropdown_model};
    use crate::login_item::LaunchAtLoginStatus;
    use crate::model::{MemoryPressure, MemorySnapshot};
    use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};
    use crate::trend::MemoryTrend;

    #[test]
    fn badge_prioritizes_pressure_over_trend() {
        assert_eq!(
            badge_for_state(MemoryPressure::Normal, MemoryTrend::Stable),
            BadgeKind::None
        );
        assert_eq!(
            badge_for_state(MemoryPressure::Normal, MemoryTrend::Rising),
            BadgeKind::Rising
        );
        assert_eq!(
            badge_for_state(MemoryPressure::Normal, MemoryTrend::RisingFast),
            BadgeKind::RisingFast
        );
        assert_eq!(
            badge_for_state(MemoryPressure::Elevated, MemoryTrend::Stable),
            BadgeKind::Elevated
        );
        assert_eq!(
            badge_for_state(MemoryPressure::Elevated, MemoryTrend::RisingFast),
            BadgeKind::Elevated
        );
        assert_eq!(
            badge_for_state(MemoryPressure::High, MemoryTrend::Stable),
            BadgeKind::High
        );
        assert_eq!(
            badge_for_state(MemoryPressure::High, MemoryTrend::RisingFast),
            BadgeKind::High
        );
    }

    fn snapshot() -> MemorySnapshot {
        MemorySnapshot {
            used_bytes: 5_700_000_000,
            total_bytes: 16_000_000_000,
            used_percent: 47,
            pressure: MemoryPressure::Normal,
            swap_used_bytes: 1_200_000_000,
        }
    }

    #[test]
    fn loading_layout_omits_pressure_and_swap_sections() {
        let model = placeholder_dropdown_model();
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert_eq!(
            entries,
            vec![
                MenuEntry::SectionHeader("Memory"),
                MenuEntry::Loading,
                MenuEntry::Separator,
                MenuEntry::Refresh,
                MenuEntry::AutoRefresh { enabled: true },
                MenuEntry::Separator,
                MenuEntry::ShowAppUsage { enabled: false },
                MenuEntry::LaunchAtLogin(LaunchAtLoginStatus::Disabled),
                MenuEntry::Separator,
                MenuEntry::Quit,
            ]
        );
    }

    #[test]
    fn loaded_layout_renders_memory_section_with_pressure_and_swap_rows() {
        let snapshot = MemorySnapshot {
            used_bytes: 5_700_000_000,
            total_bytes: 16_000_000_000,
            used_percent: 47,
            pressure: MemoryPressure::Normal,
            swap_used_bytes: 1_200_000_000,
        };
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Enabled, true);
        assert_eq!(
            entries,
            vec![
                MenuEntry::SectionHeader("Memory"),
                MenuEntry::Stat {
                    primary: "47%",
                    tail: Some("5.7 / 16.0 GB"),
                    is_high: false,
                },
                MenuEntry::Stat {
                    primary: "Pressure",
                    tail: Some("Normal"),
                    is_high: false,
                },
                MenuEntry::Stat {
                    primary: "Swap",
                    tail: Some("1.2 GB"),
                    is_high: false,
                },
                MenuEntry::Separator,
                MenuEntry::Refresh,
                MenuEntry::AutoRefresh { enabled: true },
                MenuEntry::Separator,
                MenuEntry::ShowAppUsage { enabled: false },
                MenuEntry::LaunchAtLogin(LaunchAtLoginStatus::Enabled),
                MenuEntry::Separator,
                MenuEntry::Quit,
            ]
        );
    }

    #[test]
    fn loaded_layout_hides_swap_row_when_zero() {
        let snapshot = MemorySnapshot {
            used_bytes: 5_700_000_000,
            total_bytes: 16_000_000_000,
            used_percent: 47,
            pressure: MemoryPressure::Normal,
            swap_used_bytes: 0,
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
    fn high_pressure_is_marked_for_red_rendering() {
        let snapshot = MemorySnapshot {
            used_bytes: 14_000_000_000,
            total_bytes: 16_000_000_000,
            used_percent: 88,
            pressure: MemoryPressure::High,
            swap_used_bytes: 6_000_000_000,
        };
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, false);
        assert_eq!(
            entries[2],
            MenuEntry::Stat {
                primary: "Pressure",
                tail: Some("High"),
                is_high: true,
            }
        );
        assert!(entries
            .iter()
            .any(|e| matches!(e, MenuEntry::AutoRefresh { enabled: false })));
    }

    #[test]
    fn loaded_with_apps_hidden_omits_apps_section() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Hidden);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert!(!entries
            .iter()
            .any(|e| matches!(e, MenuEntry::SectionHeader("Apps"))));
    }

    #[test]
    fn loaded_with_apps_loading_renders_loading_row() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Loading);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert_eq!(entries[4], MenuEntry::SectionHeader("Apps"));
        assert_eq!(entries[5], MenuEntry::AppLoading);
    }

    #[test]
    fn loaded_with_apps_unavailable_renders_one_row() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Unavailable);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);
        assert_eq!(entries[4], MenuEntry::SectionHeader("Apps"));
        assert_eq!(entries[5], MenuEntry::AppUnavailable);
    }

    #[test]
    fn show_app_usage_state_reflects_toggle() {
        use super::loaded_menu_entries_with_app_usage;
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Hidden);
        let on =
            loaded_menu_entries_with_app_usage(&model, LaunchAtLoginStatus::Disabled, true, true);
        assert!(on
            .iter()
            .any(|e| matches!(e, MenuEntry::ShowAppUsage { enabled: true })));

        let off =
            loaded_menu_entries_with_app_usage(&model, LaunchAtLoginStatus::Disabled, true, false);
        assert!(off
            .iter()
            .any(|e| matches!(e, MenuEntry::ShowAppUsage { enabled: false })));
    }

    #[test]
    fn loaded_with_apps_rows_follow_memory_section() {
        let usage = vec![
            AppMemoryUsage {
                name: "Cursor".to_string(),
                group_key: "/Applications/Cursor.app".to_string(),
                footprint_bytes: 2_000_000_000,
                pids: vec![1],
                can_quit: true,
                delta_bytes: None,
            },
            AppMemoryUsage {
                name: "Chrome".to_string(),
                group_key: "/Applications/Chrome.app".to_string(),
                footprint_bytes: 1_200_000_000,
                pids: vec![2],
                can_quit: true,
                delta_bytes: None,
            },
        ];
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Loaded(usage));
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Disabled, true);

        assert_eq!(entries[0], MenuEntry::SectionHeader("Memory"));
        assert!(matches!(entries[1], MenuEntry::Stat { primary: "47%", .. }));
        assert!(matches!(
            entries[2],
            MenuEntry::Stat {
                primary: "Pressure",
                ..
            }
        ));
        assert!(matches!(
            entries[3],
            MenuEntry::Stat {
                primary: "Swap",
                ..
            }
        ));
        assert_eq!(entries[4], MenuEntry::SectionHeader("Apps"));
        assert_eq!(
            entries[5],
            MenuEntry::AppRow {
                primary: "Cursor",
                tail: Some("2.0 GB"),
                action_tag: Some(0),
            }
        );
        assert_eq!(
            entries[6],
            MenuEntry::AppRow {
                primary: "Chrome",
                tail: Some("1.2 GB"),
                action_tag: Some(1),
            }
        );
        assert_eq!(entries[7], MenuEntry::Separator);
    }
}
