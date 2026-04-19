use crate::format::{
    dropdown_rows, menu_bar_icon, menu_bar_text, placeholder_dropdown_rows, placeholder_text,
};
use crate::model::{DropdownRows, MemoryPressure, MemorySnapshot};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBar, NSStatusItem};
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
    Quit(&'a str),
}

fn menu_entries(rows: &DropdownRows) -> [MenuEntry<'_>; 8] {
    [
        MenuEntry::Disabled(&rows.ram_used),
        MenuEntry::Disabled(&rows.ram_total),
        MenuEntry::Disabled(&rows.memory_pressure),
        MenuEntry::Disabled(&rows.swap_used),
        MenuEntry::Separator,
        MenuEntry::Refresh(&rows.refresh),
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
        controller.set_menu_rows(&placeholder_dropdown_rows(), mtm);
        controller
    }

    pub fn set_snapshot(&self, snapshot: MemorySnapshot, mtm: MainThreadMarker) {
        self.set_label(
            &menu_bar_text(snapshot.used_percent),
            menu_bar_icon(snapshot.pressure),
            mtm,
        );
        self.set_menu_rows(&dropdown_rows(snapshot), mtm);
    }

    pub fn set_placeholder(&self, mtm: MainThreadMarker) {
        self.set_label(&placeholder_text(), menu_bar_icon(MemoryPressure::Normal), mtm);
        self.set_menu_rows(&placeholder_dropdown_rows(), mtm);
    }

    fn set_label(&self, text: &str, icon: &str, mtm: MainThreadMarker) {
        if let Some(button) = self.status_item.button(mtm) {
            let full = format!("{text} {icon}");
            let title = NSString::from_str(&full);
            button.setTitle(&title);
        }
    }

    fn set_menu_rows(&self, rows: &DropdownRows, mtm: MainThreadMarker) {
        let menu = NSMenu::new(mtm);
        let empty = NSString::from_str("");
        for entry in menu_entries(rows) {
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

    #[test]
    fn menu_entries_keep_the_v2_row_and_action_order() {
        let rows = placeholder_dropdown_rows();

        assert_eq!(
            menu_entries(&rows),
            [
                MenuEntry::Disabled("RAM Used: 0.0 GB"),
                MenuEntry::Disabled("RAM Total: 0.0 GB"),
                MenuEntry::Disabled("Memory Pressure: Normal"),
                MenuEntry::Disabled("Swap Used: 0.0 GB"),
                MenuEntry::Separator,
                MenuEntry::Refresh("Refresh"),
                MenuEntry::Separator,
                MenuEntry::Quit("Quit"),
            ]
        );
    }
}
