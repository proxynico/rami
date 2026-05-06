use crate::model::MemoryPressure;
use crate::trend::MemoryTrend;
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_app_kit::{
    NSColor, NSCompositingOperation, NSImage, NSImageSymbolConfiguration, NSImageSymbolScale,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeKind {
    None,
    Rising,
    RisingFast,
    Elevated,
    High,
}

pub(crate) fn badge_for_state(pressure: MemoryPressure, trend: MemoryTrend) -> BadgeKind {
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

pub(crate) struct StatusImage {
    pub(crate) image: Retained<NSImage>,
    pub(crate) template: bool,
}

pub(crate) fn make_status_image(
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
