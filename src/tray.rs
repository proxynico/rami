use crate::format::{dropdown_rows, gauge_symbol_name, placeholder_dropdown_rows};
use crate::login_item::LaunchAtLoginStatus;
use crate::model::{DropdownRows, MemoryPressure, MemorySnapshot};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSCellImagePosition, NSColor, NSControlStateValueOff, NSControlStateValueOn,
    NSForegroundColorAttributeName, NSImage, NSImageSymbolConfiguration, NSImageSymbolScale,
    NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSString};
use std::cell::{Cell, RefCell};

pub struct TrayController {
    status_item: objc2::rc::Retained<NSStatusItem>,
    menu: Retained<NSMenu>,
    ram_item: Retained<NSMenuItem>,
    pressure_item: Retained<NSMenuItem>,
    swap_item: Retained<NSMenuItem>,
    refresh_item: Retained<NSMenuItem>,
    auto_refresh_item: Retained<NSMenuItem>,
    launch_at_login_item: Retained<NSMenuItem>,
    quit_item: Retained<NSMenuItem>,
    last_image_name: RefCell<Option<&'static str>>,
    last_pressure: Cell<MemoryPressure>,
    last_ram_title: RefCell<String>,
    last_pressure_title: RefCell<String>,
    last_swap_title: RefCell<String>,
    last_refresh_title: RefCell<String>,
    last_quit_title: RefCell<String>,
    last_auto_refresh_enabled: Cell<bool>,
    last_launch_title: RefCell<String>,
    last_launch_checked: Cell<bool>,
    last_launch_enabled: Cell<bool>,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum MenuEntry<'a> {
    /// Read-only summary line (enabled for legibility; no action).
    Summary(&'a str),
    Separator,
    Refresh(&'a str),
    LaunchAtLogin(LaunchAtLoginStatus),
    Quit(&'a str),
}

#[cfg(test)]
fn menu_entries(
    rows: &DropdownRows,
    launch_at_login_status: LaunchAtLoginStatus,
) -> [MenuEntry<'_>; 8] {
    [
        MenuEntry::Summary(&rows.ram_summary),
        MenuEntry::Summary(&rows.memory_pressure),
        MenuEntry::Summary(&rows.swap_used),
        MenuEntry::Separator,
        MenuEntry::Refresh(&rows.refresh),
        MenuEntry::LaunchAtLogin(launch_at_login_status),
        MenuEntry::Separator,
        MenuEntry::Quit(&rows.quit),
    ]
}

impl TrayController {
    pub fn new(mtm: MainThreadMarker, refresh_target: Retained<AnyObject>) -> Self {
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
        let menu = NSMenu::new(mtm);
        let rows = placeholder_dropdown_rows();
        let empty = NSString::from_str("");

        let ram_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.ram_summary),
                None,
                &empty,
            )
        };
        menu.addItem(&ram_item);

        let pressure_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.memory_pressure),
                None,
                &empty,
            )
        };
        menu.addItem(&pressure_item);

        let swap_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.swap_used),
                None,
                &empty,
            )
        };
        menu.addItem(&swap_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let refresh_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.refresh),
                Some(sel!(refreshNow:)),
                &empty,
            )
        };
        unsafe {
            refresh_item.setTarget(Some(&refresh_target));
        }
        menu.addItem(&refresh_item);

        let auto_refresh_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Pause Auto Refresh"),
                Some(sel!(toggleAutoRefresh:)),
                &empty,
            )
        };
        unsafe {
            auto_refresh_item.setTarget(Some(&refresh_target));
        }
        auto_refresh_item.setState(NSControlStateValueOn);
        menu.addItem(&auto_refresh_item);

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
        menu.addItem(&launch_at_login_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.quit),
                Some(sel!(terminate:)),
                &empty,
            )
        };
        menu.addItem(&quit_item);
        status_item.setMenu(Some(&menu));

        if let Some(button) = status_item.button(mtm) {
            button.setTitle(&empty);
            button.setImagePosition(NSCellImagePosition::ImageOnly);
        }

        let controller = Self {
            status_item,
            menu,
            ram_item,
            pressure_item,
            swap_item,
            refresh_item,
            auto_refresh_item,
            launch_at_login_item,
            quit_item,
            last_image_name: RefCell::new(None),
            last_pressure: Cell::new(MemoryPressure::Normal),
            last_ram_title: RefCell::new(String::new()),
            last_pressure_title: RefCell::new(String::new()),
            last_swap_title: RefCell::new(String::new()),
            last_refresh_title: RefCell::new(String::new()),
            last_quit_title: RefCell::new(String::new()),
            last_auto_refresh_enabled: Cell::new(true),
            last_launch_title: RefCell::new(String::new()),
            last_launch_checked: Cell::new(false),
            last_launch_enabled: Cell::new(false),
        };
        controller.set_gauge(0, MemoryPressure::Normal, mtm);
        controller.set_menu_rows(&rows, LaunchAtLoginStatus::Disabled, true, mtm);
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
        self.set_menu_rows(
            &dropdown_rows(snapshot),
            launch_at_login_status,
            auto_refresh_enabled,
            mtm,
        );
    }

    pub fn set_placeholder(&self, launch_at_login_status: LaunchAtLoginStatus, mtm: MainThreadMarker) {
        self.set_gauge(0, MemoryPressure::Normal, mtm);
        self.set_menu_rows(&placeholder_dropdown_rows(), launch_at_login_status, true, mtm);
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
            match self.make_symbol_image(name, mtm) {
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

    fn make_symbol_image(
        &self,
        name: &'static str,
        _mtm: MainThreadMarker,
    ) -> Option<Retained<NSImage>> {
        let desc = NSString::from_str("RAM usage");
        let symbol_name = NSString::from_str(name);
        let base = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &symbol_name,
            Some(&desc),
        )?;
        let config = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Large);
        base.imageWithSymbolConfiguration(&config)
    }

    fn set_menu_rows(
        &self,
        rows: &DropdownRows,
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        _mtm: MainThreadMarker,
    ) {
        set_dropdown_summary_if_changed(&self.ram_item, &self.last_ram_title, &rows.ram_summary);
        set_dropdown_summary_if_changed(
            &self.pressure_item,
            &self.last_pressure_title,
            &rows.memory_pressure,
        );
        set_dropdown_summary_if_changed(&self.swap_item, &self.last_swap_title, &rows.swap_used);
        set_menu_item_title_if_changed(&self.refresh_item, &self.last_refresh_title, &rows.refresh);
        set_menu_item_title_if_changed(&self.quit_item, &self.last_quit_title, &rows.quit);

        let auto_refresh_title = if auto_refresh_enabled {
            "Pause Auto Refresh"
        } else {
            "Resume Auto Refresh"
        };
        if self.last_auto_refresh_enabled.get() != auto_refresh_enabled {
            self.auto_refresh_item
                .setTitle(&NSString::from_str(auto_refresh_title));
            self.auto_refresh_item.setState(if auto_refresh_enabled {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            self.last_auto_refresh_enabled.set(auto_refresh_enabled);
        }

        let launch_title = launch_at_login_status.menu_title();
        set_menu_item_title_if_changed(
            &self.launch_at_login_item,
            &self.last_launch_title,
            launch_title,
        );
        let launch_checked = launch_at_login_status.should_show_checked_state();
        if self.last_launch_checked.get() != launch_checked {
            self.launch_at_login_item.setState(if launch_checked {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            self.last_launch_checked.set(launch_checked);
        }
        let launch_enabled = launch_at_login_status.should_enable_menu_item();
        if self.last_launch_enabled.get() != launch_enabled {
            self.launch_at_login_item.setEnabled(launch_enabled);
            self.last_launch_enabled.set(launch_enabled);
        }
        self.status_item.setMenu(Some(&self.menu));
    }
}

fn set_menu_item_title_if_changed(item: &NSMenuItem, cache: &RefCell<String>, value: &str) {
    if *cache.borrow() != value {
        item.setTitle(&NSString::from_str(value));
        *cache.borrow_mut() = value.to_string();
    }
}

fn dropdown_summary_attributed(title: &str) -> Retained<NSAttributedString> {
    unsafe {
        let string = NSString::from_str(title);
        let color = Retained::cast_unchecked::<AnyObject>(NSColor::secondaryLabelColor());
        let attrs = NSDictionary::from_retained_objects(
            &[NSForegroundColorAttributeName],
            &[color],
        );
        NSAttributedString::new_with_attributes(&string, &attrs)
    }
}

fn set_dropdown_summary_if_changed(item: &NSMenuItem, cache: &RefCell<String>, text: &str) {
    if *cache.borrow() != text {
        let attr = dropdown_summary_attributed(text);
        item.setAttributedTitle(Some(&attr));
        *cache.borrow_mut() = text.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::{menu_entries, MenuEntry};
    use crate::format::placeholder_dropdown_rows;
    use crate::login_item::LaunchAtLoginStatus;

    #[test]
    fn menu_entries_include_launch_at_login_before_quit() {
        let rows = placeholder_dropdown_rows();
        let entries = menu_entries(&rows, LaunchAtLoginStatus::Disabled);

        assert_eq!(
            entries,
            [
                MenuEntry::Summary("RAM: --% — 0.0 GB of 0.0 GB"),
                MenuEntry::Summary("Memory Pressure: Normal"),
                MenuEntry::Summary("Swap Used: 0.0 GB"),
                MenuEntry::Separator,
                MenuEntry::Refresh("Refresh"),
                MenuEntry::LaunchAtLogin(LaunchAtLoginStatus::Disabled),
                MenuEntry::Separator,
                MenuEntry::Quit("Quit"),
            ]
        );
    }

    #[test]
    fn menu_entries_mark_launch_at_login_as_enabled_when_requested() {
        let rows = placeholder_dropdown_rows();
        let entries = menu_entries(&rows, LaunchAtLoginStatus::Enabled);

        assert_eq!(entries[5], MenuEntry::LaunchAtLogin(LaunchAtLoginStatus::Enabled));
    }
}
