use crate::trend::MemoryTrend;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::{
    NSColor, NSCompositingOperation, NSImage, NSImageSymbolConfiguration, NSImageSymbolScale,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeKind {
    None,
    RisingFast,
}

pub(crate) fn badge_for_state(trend: MemoryTrend) -> BadgeKind {
    match trend {
        MemoryTrend::RisingFast => BadgeKind::RisingFast,
        MemoryTrend::Rising | MemoryTrend::Stable => BadgeKind::None,
    }
}

pub(crate) struct StatusImage {
    pub(crate) image: Retained<NSImage>,
    pub(crate) template: bool,
}

pub(crate) fn make_status_image(
    gauge_name: &'static str,
    trend: MemoryTrend,
) -> Option<StatusImage> {
    let badge = badge_for_state(trend);
    let base_template = render_template_symbol(gauge_name, NSImageSymbolScale::Large)?;
    match badge {
        BadgeKind::None => Some(StatusImage {
            image: base_template,
            template: true,
        }),
        BadgeKind::RisingFast => {
            let label = NSColor::labelColor();
            // Yellow badge: distinct from the orange gauge tint used for memory
            // pressure warnings in the menu bar.
            let climb = NSColor::systemYellowColor();
            let base_colored =
                render_colored_symbol(gauge_name, NSImageSymbolScale::Large, &label)?;
            let badge_image = render_colored_symbol(
                "arrow.up.right.circle.fill",
                NSImageSymbolScale::Small,
                &climb,
            )?;
            let composite = compose_with_badge(base_colored, badge_image)?;
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

fn compose_with_badge(
    base: Retained<NSImage>,
    badge: Retained<NSImage>,
) -> Option<Retained<NSImage>> {
    let size = base.size();
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    // Draw lazily via a drawing handler instead of rasterizing with lockFocus:
    // the handler re-runs whenever the menu bar redraws the icon, so dynamic
    // colors (labelColor) resolve against the menu bar's current light/dark
    // appearance rather than being baked in at creation time.
    let handler = RcBlock::new(move |dest_rect: NSRect| -> Bool {
        let zero_rect = NSRect::ZERO;
        base.drawInRect_fromRect_operation_fraction(
            dest_rect,
            zero_rect,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        let badge_extent = (dest_rect.size.height * 0.65).min(dest_rect.size.width);
        let badge_rect = NSRect::new(
            NSPoint::new(
                dest_rect.origin.x + dest_rect.size.width - badge_extent,
                dest_rect.origin.y,
            ),
            NSSize::new(badge_extent, badge_extent),
        );
        badge.drawInRect_fromRect_operation_fraction(
            badge_rect,
            zero_rect,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        Bool::YES
    });
    Some(NSImage::imageWithSize_flipped_drawingHandler(
        size, false, &handler,
    ))
}
