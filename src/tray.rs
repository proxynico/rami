use crate::format::{
    dropdown_rows, menu_bar_text, menu_bar_tooltip, placeholder_dropdown_rows, placeholder_text,
    pressure_tint, PressureTint,
};
use crate::login_item::LaunchAtLoginStatus;
use crate::model::{DropdownRows, MemorySnapshot};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSControlStateValueOff, NSControlStateValueOn, NSImageSymbolConfiguration, NSImageSymbolScale,
    NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
};
use objc2_foundation::NSString;
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
    last_label: RefCell<String>,
    last_tooltip: RefCell<String>,
    last_image_name: RefCell<Option<&'static str>>,
    last_tint: Cell<PressureTint>,
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
        // Enabled + no selector: full `labelColor` contrast on dark menus. Disabled items are
        // drawn too dim for read-only stats rows.
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
            last_label: RefCell::new(String::new()),
            last_tooltip: RefCell::new(String::new()),
            last_image_name: RefCell::new(None),
            last_tint: Cell::new(PressureTint::Template),
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
        controller.set_image(PressureTint::Template, mtm);
        controller.set_label(&placeholder_text(), mtm);
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
        self.set_image(pressure_tint(snapshot.pressure), mtm);
        self.set_label(&menu_bar_text(snapshot.used_percent), mtm);
        self.set_tooltip(&menu_bar_tooltip(snapshot), mtm);
        self.set_menu_rows(
            &dropdown_rows(snapshot),
            launch_at_login_status,
            auto_refresh_enabled,
            mtm,
        );
    }

    pub fn set_placeholder(&self, launch_at_login_status: LaunchAtLoginStatus, mtm: MainThreadMarker) {
        self.set_image(PressureTint::Template, mtm);
        self.set_label(&placeholder_text(), mtm);
        self.set_tooltip("RAM data is loading...", mtm);
        self.set_menu_rows(&placeholder_dropdown_rows(), launch_at_login_status, true, mtm);
    }

    fn set_label(&self, text: &str, mtm: MainThreadMarker) {
        if let Some(button) = self.status_item.button(mtm) {
            if *self.last_label.borrow() != text {
                let title = NSString::from_str(text);
                button.setTitle(&title);
                *self.last_label.borrow_mut() = text.to_string();
            }
        }
    }

    fn set_image(&self, tint: PressureTint, mtm: MainThreadMarker) {
        let preferred = "memorychip.fill";
        let fallback = "memorychip";
        let current_name = *self.last_image_name.borrow();
        let current_tint = self.last_tint.get();
        if current_name == Some(preferred) && current_tint == tint {
            return;
        }

        if let Some(button) = self.status_item.button(mtm) {
            let symbol_name = self
                .make_symbol_image(preferred, mtm)
                .map(|image| {
                    button.setImage(Some(&image));
                    preferred
                })
                .or_else(|| {
                    self.make_symbol_image(fallback, mtm).map(|image| {
                        button.setImage(Some(&image));
                        fallback
                    })
                });

            if symbol_name.is_none() {
                button.setImage(None);
            }

            button.setImagePosition(objc2_app_kit::NSCellImagePosition::ImageLeading);
            match tint {
                PressureTint::Template => {
                    if let Some(image) = button.image() {
                        image.setTemplate(true);
                    }
                    button.setContentTintColor(None);
                }
                PressureTint::Yellow => {
                    if let Some(image) = button.image() {
                        image.setTemplate(false);
                    }
                    button.setContentTintColor(Some(&objc2_app_kit::NSColor::systemYellowColor()));
                }
                PressureTint::Red => {
                    if let Some(image) = button.image() {
                        image.setTemplate(false);
                    }
                    button.setContentTintColor(Some(&objc2_app_kit::NSColor::systemRedColor()));
                }
            }

            *self.last_image_name.borrow_mut() = symbol_name;
            self.last_tint.set(tint);
        }
    }

    fn make_symbol_image(
        &self,
        name: &'static str,
        _mtm: MainThreadMarker,
    ) -> Option<Retained<objc2_app_kit::NSImage>> {
        let desc = NSString::from_str("Memory usage");
        let symbol_name = NSString::from_str(name);
        let base = objc2_app_kit::NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &symbol_name,
            Some(&desc),
        )?;
        let config = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Large);
        base.imageWithSymbolConfiguration(&config)
    }

    fn set_tooltip(&self, tooltip: &str, mtm: MainThreadMarker) {
        if let Some(button) = self.status_item.button(mtm) {
            if *self.last_tooltip.borrow() != tooltip {
                button.setToolTip(Some(&NSString::from_str(tooltip)));
                *self.last_tooltip.borrow_mut() = tooltip.to_string();
            }
        }
    }

    fn set_menu_rows(
        &self,
        rows: &DropdownRows,
        launch_at_login_status: LaunchAtLoginStatus,
        auto_refresh_enabled: bool,
        _mtm: MainThreadMarker,
    ) {
        set_menu_item_title_if_changed(&self.ram_item, &self.last_ram_title, &rows.ram_summary);
        set_menu_item_title_if_changed(
            &self.pressure_item,
            &self.last_pressure_title,
            &rows.memory_pressure,
        );
        set_menu_item_title_if_changed(&self.swap_item, &self.last_swap_title, &rows.swap_used);
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
                MenuEntry::Summary("RAM: 0.0 GB of 0.0 GB"),
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
