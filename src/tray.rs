use crate::format::{
    dropdown_model, gauge_symbol_name, placeholder_dropdown_model, DropdownModel, PressureDisplay,
    StatRow,
};
use crate::login_item::LaunchAtLoginStatus;
use crate::model::{MemoryPressure, MemorySnapshot};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSCellImagePosition, NSColor, NSControlStateValueOff, NSControlStateValueOn,
    NSEventModifierFlags, NSFont, NSFontAttributeName, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSImage, NSImageSymbolConfiguration, NSImageSymbolScale,
    NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSString,
};
use std::cell::{Cell, RefCell};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuShape {
    Uninitialized,
    Loading,
    Loaded,
}

pub struct TrayController {
    status_item: Retained<NSStatusItem>,
    menu: Retained<NSMenu>,
    memory_section: Retained<NSMenuItem>,
    pressure_section: Retained<NSMenuItem>,
    swap_section: Retained<NSMenuItem>,
    memory_item: Retained<NSMenuItem>,
    pressure_item: Retained<NSMenuItem>,
    swap_item: Retained<NSMenuItem>,
    loading_item: Retained<NSMenuItem>,
    refresh_item: Retained<NSMenuItem>,
    auto_refresh_item: Retained<NSMenuItem>,
    launch_at_login_item: Retained<NSMenuItem>,
    quit_item: Retained<NSMenuItem>,
    pause_icon: Option<Retained<NSImage>>,
    play_icon: Option<Retained<NSImage>>,
    last_image_name: RefCell<Option<&'static str>>,
    last_pressure: Cell<MemoryPressure>,
    shape: Cell<MenuShape>,
    last_memory_row: RefCell<Option<StatRow>>,
    last_pressure_display: RefCell<Option<PressureDisplay>>,
    last_swap_row: RefCell<Option<StatRow>>,
    last_auto_refresh_enabled: Cell<bool>,
    last_launch_title: RefCell<String>,
    last_launch_checked: Cell<bool>,
    last_launch_enabled: Cell<bool>,
}

impl TrayController {
    pub fn new(mtm: MainThreadMarker, refresh_target: Retained<AnyObject>) -> Self {
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let empty = NSString::from_str("");

        let memory_section =
            NSMenuItem::sectionHeaderWithTitle(&NSString::from_str("Memory"), mtm);
        let pressure_section =
            NSMenuItem::sectionHeaderWithTitle(&NSString::from_str("Pressure"), mtm);
        let swap_section = NSMenuItem::sectionHeaderWithTitle(&NSString::from_str("Swap"), mtm);

        let memory_item = make_stat_item(mtm);
        let pressure_item = make_stat_item(mtm);
        let swap_item = make_stat_item(mtm);
        let loading_item = make_stat_item(mtm);
        loading_item.setAttributedTitle(Some(&loading_attributed_title()));

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
            pressure_section,
            swap_section,
            memory_item,
            pressure_item,
            swap_item,
            loading_item,
            refresh_item,
            auto_refresh_item,
            launch_at_login_item,
            quit_item,
            pause_icon,
            play_icon,
            last_image_name: RefCell::new(None),
            last_pressure: Cell::new(MemoryPressure::Normal),
            shape: Cell::new(MenuShape::Uninitialized),
            last_memory_row: RefCell::new(None),
            last_pressure_display: RefCell::new(None),
            last_swap_row: RefCell::new(None),
            last_auto_refresh_enabled: Cell::new(true),
            last_launch_title: RefCell::new(String::new()),
            last_launch_checked: Cell::new(false),
            last_launch_enabled: Cell::new(false),
        };
        controller.set_gauge(0, MemoryPressure::Normal, mtm);
        controller.apply_model(
            &placeholder_dropdown_model(),
            LaunchAtLoginStatus::Disabled,
            true,
            mtm,
        );
        controller
    }

    pub fn set_snapshot(
        &self,
        snapshot: MemorySnapshot,
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        mtm: MainThreadMarker,
    ) {
        self.set_gauge(snapshot.used_percent, snapshot.pressure, mtm);
        self.apply_model(
            &dropdown_model(snapshot),
            launch_at_login_status,
            auto_refresh_enabled,
            mtm,
        );
    }

    pub fn set_placeholder(
        &self,
        launch_at_login_status: LaunchAtLoginStatus,
        mtm: MainThreadMarker,
    ) {
        self.set_gauge(0, MemoryPressure::Normal, mtm);
        self.apply_model(
            &placeholder_dropdown_model(),
            launch_at_login_status,
            true,
            mtm,
        );
    }

    fn set_gauge(&self, percent: u8, pressure: MemoryPressure, mtm: MainThreadMarker) {
        let name = gauge_symbol_name(percent);
        let name_unchanged = *self.last_image_name.borrow() == Some(name);
        let pressure_unchanged = self.last_pressure.get() == pressure;
        if name_unchanged && pressure_unchanged {
            return;
        }

        if let Some(button) = self.status_item.button(mtm) {
            let warning = matches!(pressure, MemoryPressure::High);
            match self.make_symbol_image(name) {
                Some(image) => {
                    image.setTemplate(!warning);
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
        }
    }

    fn make_symbol_image(&self, name: &'static str) -> Option<Retained<NSImage>> {
        let desc = NSString::from_str("RAM usage");
        let symbol_name = NSString::from_str(name);
        let base = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &symbol_name,
            Some(&desc),
        )?;
        let config = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Large);
        base.imageWithSymbolConfiguration(&config)
    }

    fn apply_model(
        &self,
        model: &DropdownModel,
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        mtm: MainThreadMarker,
    ) {
        let new_shape = match model {
            DropdownModel::Loading => MenuShape::Loading,
            DropdownModel::Loaded { .. } => MenuShape::Loaded,
        };
        if self.shape.get() != new_shape {
            self.rebuild_menu(new_shape, mtm);
            self.shape.set(new_shape);
            self.last_memory_row.borrow_mut().take();
            self.last_pressure_display.borrow_mut().take();
            self.last_swap_row.borrow_mut().take();
        }

        if let DropdownModel::Loaded { memory, pressure, swap } = model {
            if self.last_memory_row.borrow().as_ref() != Some(memory) {
                self.memory_item.setAttributedTitle(Some(&stat_row_attributed(
                    memory,
                    NSColor::labelColor(),
                )));
                *self.last_memory_row.borrow_mut() = Some(memory.clone());
            }
            if self.last_pressure_display.borrow().as_ref() != Some(pressure) {
                self.pressure_item
                    .setAttributedTitle(Some(&pressure_attributed(pressure)));
                *self.last_pressure_display.borrow_mut() = Some(pressure.clone());
            }
            if self.last_swap_row.borrow().as_ref() != Some(swap) {
                self.swap_item.setAttributedTitle(Some(&stat_row_attributed(
                    swap,
                    NSColor::labelColor(),
                )));
                *self.last_swap_row.borrow_mut() = Some(swap.clone());
            }
        }

        self.update_auto_refresh(auto_refresh_enabled);
        self.update_launch_at_login(launch_at_login_status);
        self.status_item.setMenu(Some(&self.menu));
    }

    fn rebuild_menu(&self, shape: MenuShape, mtm: MainThreadMarker) {
        self.menu.removeAllItems();
        match shape {
            MenuShape::Uninitialized => {}
            MenuShape::Loading => {
                self.menu.addItem(&self.memory_section);
                self.menu.addItem(&self.loading_item);
            }
            MenuShape::Loaded => {
                self.menu.addItem(&self.memory_section);
                self.menu.addItem(&self.memory_item);
                self.menu.addItem(&self.pressure_section);
                self.menu.addItem(&self.pressure_item);
                self.menu.addItem(&self.swap_section);
                self.menu.addItem(&self.swap_item);
            }
        }
        self.menu.addItem(&NSMenuItem::separatorItem(mtm));
        self.menu.addItem(&self.refresh_item);
        self.menu.addItem(&self.auto_refresh_item);
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

fn make_action_icon(name: &str) -> Option<Retained<NSImage>> {
    let desc = NSString::from_str("");
    let symbol_name = NSString::from_str(name);
    let base = NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
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
    let font = stat_font();
    let primary_attrs = attrs_for(primary_color, font.clone());
    let primary_str = NSString::from_str(&row.primary);
    let primary =
        unsafe { NSAttributedString::new_with_attributes(&primary_str, &primary_attrs) };

    let Some(tail) = &row.tail else {
        return primary;
    };

    let result = NSMutableAttributedString::new();
    result.appendAttributedString(&primary);

    let tail_attrs = attrs_for(NSColor::secondaryLabelColor(), font);
    let tail_str = NSString::from_str(&format!(" {tail}"));
    let tail_attr = unsafe { NSAttributedString::new_with_attributes(&tail_str, &tail_attrs) };
    result.appendAttributedString(&tail_attr);

    Retained::into_super(result)
}

fn pressure_attributed(display: &PressureDisplay) -> Retained<NSAttributedString> {
    let color = if display.is_high {
        NSColor::systemRedColor()
    } else {
        NSColor::labelColor()
    };
    stat_row_attributed(
        &StatRow {
            primary: display.text.clone(),
            tail: None,
        },
        color,
    )
}

fn loading_attributed_title() -> Retained<NSAttributedString> {
    stat_row_attributed(
        &StatRow {
            primary: "Loading…".to_string(),
            tail: None,
        },
        NSColor::secondaryLabelColor(),
    )
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MenuEntry<'a> {
    SectionHeader(&'a str),
    Stat { primary: &'a str, tail: Option<&'a str>, is_high: bool },
    Loading,
    Separator,
    Refresh,
    AutoRefresh { enabled: bool },
    LaunchAtLogin(LaunchAtLoginStatus),
    Quit,
}

#[cfg(test)]
pub(crate) fn loaded_menu_entries<'a>(
    model: &'a DropdownModel,
    launch_at_login_status: LaunchAtLoginStatus,
    auto_refresh_enabled: bool,
) -> Vec<MenuEntry<'a>> {
    let mut entries = Vec::new();
    match model {
        DropdownModel::Loading => {
            entries.push(MenuEntry::SectionHeader("Memory"));
            entries.push(MenuEntry::Loading);
        }
        DropdownModel::Loaded { memory, pressure, swap } => {
            entries.push(MenuEntry::SectionHeader("Memory"));
            entries.push(MenuEntry::Stat {
                primary: &memory.primary,
                tail: memory.tail.as_deref(),
                is_high: false,
            });
            entries.push(MenuEntry::SectionHeader("Pressure"));
            entries.push(MenuEntry::Stat {
                primary: &pressure.text,
                tail: None,
                is_high: pressure.is_high,
            });
            entries.push(MenuEntry::SectionHeader("Swap"));
            entries.push(MenuEntry::Stat {
                primary: &swap.primary,
                tail: swap.tail.as_deref(),
                is_high: false,
            });
        }
    }
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Refresh);
    entries.push(MenuEntry::AutoRefresh {
        enabled: auto_refresh_enabled,
    });
    entries.push(MenuEntry::LaunchAtLogin(launch_at_login_status));
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Quit);
    entries
}

#[cfg(test)]
mod tests {
    use super::{loaded_menu_entries, MenuEntry};
    use crate::format::{dropdown_model, placeholder_dropdown_model};
    use crate::login_item::LaunchAtLoginStatus;
    use crate::model::{MemoryPressure, MemorySnapshot};

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
                MenuEntry::LaunchAtLogin(LaunchAtLoginStatus::Disabled),
                MenuEntry::Separator,
                MenuEntry::Quit,
            ]
        );
    }

    #[test]
    fn loaded_layout_renders_three_sections_with_stat_rows() {
        let snapshot = MemorySnapshot {
            used_bytes: 5_700_000_000,
            total_bytes: 16_000_000_000,
            used_percent: 47,
            pressure: MemoryPressure::Normal,
            swap_used_bytes: 1_200_000_000,
        };
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model, LaunchAtLoginStatus::Enabled, true);
        assert_eq!(entries[0], MenuEntry::SectionHeader("Memory"));
        assert_eq!(
            entries[1],
            MenuEntry::Stat {
                primary: "47%",
                tail: Some("5.7 / 16.0 GB"),
                is_high: false,
            }
        );
        assert_eq!(entries[2], MenuEntry::SectionHeader("Pressure"));
        assert_eq!(
            entries[3],
            MenuEntry::Stat {
                primary: "Normal",
                tail: None,
                is_high: false,
            }
        );
        assert_eq!(entries[4], MenuEntry::SectionHeader("Swap"));
        assert_eq!(
            entries[5],
            MenuEntry::Stat {
                primary: "1.2 GB",
                tail: None,
                is_high: false,
            }
        );
        assert_eq!(entries[6], MenuEntry::Separator);
        assert_eq!(entries[7], MenuEntry::Refresh);
        assert_eq!(entries[8], MenuEntry::AutoRefresh { enabled: true });
        assert_eq!(
            entries[9],
            MenuEntry::LaunchAtLogin(LaunchAtLoginStatus::Enabled)
        );
        assert_eq!(entries[10], MenuEntry::Separator);
        assert_eq!(entries[11], MenuEntry::Quit);
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
            entries[3],
            MenuEntry::Stat {
                primary: "High",
                tail: None,
                is_high: true,
            }
        );
        assert_eq!(entries[8], MenuEntry::AutoRefresh { enabled: false });
    }
}
