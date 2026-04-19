use crate::format::{
    dropdown_rows, menu_bar_icon, menu_bar_text, placeholder_dropdown_rows, placeholder_text,
};
use crate::login_item::LaunchAtLoginStatus;
use crate::model::{DropdownRows, MemoryPressure, MemorySnapshot};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
};
use objc2_foundation::NSString;

pub struct TrayController {
    status_item: objc2::rc::Retained<NSStatusItem>,
    refresh_target: Retained<AnyObject>,
}

#[derive(Debug, PartialEq, Eq)]
enum MenuEntry<'a> {
    Disabled(&'a str),
    Separator,
    Refresh(&'a str),
    LaunchAtLogin(LaunchAtLoginStatus),
    Quit(&'a str),
}

fn menu_entries(rows: &DropdownRows, launch_at_login_status: LaunchAtLoginStatus) -> [MenuEntry<'_>; 8] {
    [
        MenuEntry::Disabled(&rows.ram_summary),
        MenuEntry::Disabled(&rows.memory_pressure),
        MenuEntry::Disabled(&rows.swap_used),
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
        let controller = Self {
            status_item,
            refresh_target,
        };
        controller.set_label(&placeholder_text(), menu_bar_icon(MemoryPressure::Normal), mtm);
        controller.set_menu_rows(&placeholder_dropdown_rows(), LaunchAtLoginStatus::Disabled, mtm);
        controller
    }

    pub fn set_snapshot(
        &self,
        snapshot: MemorySnapshot,
        launch_at_login_status: LaunchAtLoginStatus,
        mtm: MainThreadMarker,
    ) {
        self.set_label(
            &menu_bar_text(snapshot.used_percent),
            menu_bar_icon(snapshot.pressure),
            mtm,
        );
        self.set_menu_rows(&dropdown_rows(snapshot), launch_at_login_status, mtm);
    }

    pub fn set_placeholder(&self, launch_at_login_status: LaunchAtLoginStatus, mtm: MainThreadMarker) {
        self.set_label(&placeholder_text(), menu_bar_icon(MemoryPressure::Normal), mtm);
        self.set_menu_rows(&placeholder_dropdown_rows(), launch_at_login_status, mtm);
    }

    fn set_label(&self, text: &str, icon: &str, mtm: MainThreadMarker) {
        if let Some(button) = self.status_item.button(mtm) {
            let full = format!("{text} {icon}");
            let title = NSString::from_str(&full);
            button.setTitle(&title);
        }
    }

    fn set_menu_rows(
        &self,
        rows: &DropdownRows,
        launch_at_login_status: LaunchAtLoginStatus,
        mtm: MainThreadMarker,
    ) {
        let menu = NSMenu::new(mtm);
        let empty = NSString::from_str("");
        for entry in menu_entries(rows, launch_at_login_status) {
            match entry {
                MenuEntry::Disabled(title) => {
                    let item = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            NSMenuItem::alloc(mtm),
                            &NSString::from_str(title),
                            None,
                            &empty,
                        )
                    };
                    item.setEnabled(false);
                    menu.addItem(&item);
                }
                MenuEntry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
                MenuEntry::Refresh(title) => {
                    let item = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            NSMenuItem::alloc(mtm),
                            &NSString::from_str(title),
                            Some(sel!(refreshNow:)),
                            &empty,
                        )
                    };
                    unsafe {
                        item.setTarget(Some(&self.refresh_target));
                    }
                    menu.addItem(&item);
                }
                MenuEntry::LaunchAtLogin(status) => {
                    let item = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            NSMenuItem::alloc(mtm),
                            &NSString::from_str(status.menu_title()),
                            Some(sel!(toggleLaunchAtLogin:)),
                            &empty,
                        )
                    };
                    unsafe {
                        item.setTarget(Some(&self.refresh_target));
                    }
                    item.setState(if status.should_show_checked_state() {
                        NSControlStateValueOn
                    } else {
                        NSControlStateValueOff
                    });
                    item.setEnabled(status.should_enable_menu_item());
                    menu.addItem(&item);
                }
                MenuEntry::Quit(title) => {
                    let item = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            NSMenuItem::alloc(mtm),
                            &NSString::from_str(title),
                            Some(sel!(terminate:)),
                            &empty,
                        )
                    };
                    menu.addItem(&item);
                }
            }
        }
        self.status_item.setMenu(Some(&menu));
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
                MenuEntry::Disabled("RAM: 0.0 GB of 0.0 GB"),
                MenuEntry::Disabled("Memory Pressure: Normal"),
                MenuEntry::Disabled("Swap Used: 0.0 GB"),
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
