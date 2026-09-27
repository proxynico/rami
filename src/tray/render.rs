use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSImage, NSImageSymbolConfiguration, NSImageSymbolScale, NSMenuItem};
use objc2_foundation::NSString;

pub(super) fn make_stat_item(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
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

pub(super) fn make_action_icon(name: &str) -> Option<Retained<NSImage>> {
    let desc = NSString::from_str("");
    let symbol_name = NSString::from_str(name);
    let base =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
    let config = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Small);
    let image = base.imageWithSymbolConfiguration(&config)?;
    image.setTemplate(true);
    Some(image)
}
