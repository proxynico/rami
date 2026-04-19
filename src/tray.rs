use crate::format::{dropdown_rows, menu_bar_text, placeholder_text};
use crate::model::{DropdownRows, MemorySnapshot};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBar, NSStatusItem};
use objc2_foundation::NSString;

pub struct TrayController {
    status_item: objc2::rc::Retained<NSStatusItem>,
    refresh_target: Retained<AnyObject>,
}

impl TrayController {
    pub fn new(mtm: MainThreadMarker, refresh_target: Retained<AnyObject>) -> Self {
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
        let controller = Self {
            status_item,
            refresh_target,
        };
        controller.set_label(&placeholder_text(), mtm);
        controller.set_menu_rows(
            &DropdownRows {
                ram_used: "RAM Used: 0.0 GB".to_string(),
                ram_total: "RAM Total: 0.0 GB".to_string(),
                temperature: None,
                refresh: "Refresh".to_string(),
                quit: "Quit".to_string(),
            },
            mtm,
        );
        controller
    }

    pub fn set_snapshot(&self, snapshot: MemorySnapshot, mtm: MainThreadMarker) {
        self.set_label(&menu_bar_text(snapshot.used_percent), mtm);
        self.set_menu_rows(&dropdown_rows(snapshot, None), mtm);
    }

    pub fn set_placeholder(&self, mtm: MainThreadMarker) {
        self.set_label(&placeholder_text(), mtm);
    }

    fn set_label(&self, text: &str, mtm: MainThreadMarker) {
        if let Some(button) = self.status_item.button(mtm) {
            let full = format!("{text} ▣");
            let title = NSString::from_str(&full);
            button.setTitle(&title);
        }
    }

    fn set_menu_rows(&self, rows: &DropdownRows, mtm: MainThreadMarker) {
        let menu = NSMenu::new(mtm);
        let empty = NSString::from_str("");
        let used = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.ram_used),
                None,
                &empty,
            )
        };
        used.setEnabled(false);
        let total = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.ram_total),
                None,
                &empty,
            )
        };
        total.setEnabled(false);
        let refresh = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.refresh),
                Some(sel!(refreshNow:)),
                &empty,
            )
        };
        unsafe {
            refresh.setTarget(Some(&self.refresh_target));
        }
        let quit = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&rows.quit),
                Some(sel!(terminate:)),
                &empty,
            )
        };

        menu.addItem(&used);
        menu.addItem(&total);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&refresh);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&quit);
        self.status_item.setMenu(Some(&menu));
    }
}
