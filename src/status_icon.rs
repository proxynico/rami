use crate::format::Accent;
use crate::presentation::{color_for_accent, rising_fast_badge_color};
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
    accent: Accent,
) -> Option<StatusImage> {
    let badge = badge_for_state(trend);
    let base_template = render_template_symbol(gauge_name, NSImageSymbolScale::Large)?;
    match badge {
        BadgeKind::None => Some(StatusImage {
            image: base_template,
            template: true,
        }),
        BadgeKind::RisingFast => {
            let composite = compose_rising_fast(gauge_name, accent, base_template.size())?;
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

fn compose_rising_fast(
    gauge_name: &'static str,
    accent: Accent,
    size: NSSize,
) -> Option<Retained<NSImage>> {
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    // Hierarchical SF Symbol tints and Neutral alpha resolve here, against
    // the menu bar's current appearance, not at image-create time.
    let handler = RcBlock::new(move |dest_rect: NSRect| -> Bool {
        let chrome = color_for_accent(accent);
        let climb = rising_fast_badge_color(accent);
        let Some(base) = render_colored_symbol(gauge_name, NSImageSymbolScale::Large, &chrome)
        else {
            return Bool::NO;
        };
        let Some(badge) = render_colored_symbol(
            "arrow.up.right.circle.fill",
            NSImageSymbolScale::Small,
            &climb,
        ) else {
            return Bool::NO;
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Accent;
    use objc2::{AnyThread, Message};
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSBezierPath,
        NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext,
    };
    use objc2_foundation::NSInteger;
    use std::cell::RefCell;

    fn appearance(name: &objc2_foundation::NSString) -> Retained<NSAppearance> {
        NSAppearance::appearanceNamed(name).expect("named appearance")
    }

    /// Rasterize the image under the given appearance and return the average
    /// color of its opaque pixels (alpha > 0.5), plus how many there were.
    fn sample_opaque_average(
        image: &NSImage,
        drawing_appearance: &NSAppearance,
    ) -> (f64, f64, f64, usize) {
        let side: NSInteger = 24;
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                side,
                side,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
        }
        .expect("bitmap");
        NSGraphicsContext::saveGraphicsState_class();
        let ctx =
            NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).expect("graphics context");
        NSGraphicsContext::setCurrentContext(Some(&ctx));
        NSColor::clearColor().setFill();
        let bounds = NSRect::new(NSPoint::ZERO, NSSize::new(side as f64, side as f64));
        NSBezierPath::bezierPathWithRect(bounds).fill();
        let image = image.retain();
        let draw = RcBlock::new(move || {
            image.drawInRect_fromRect_operation_fraction(
                bounds,
                NSRect::ZERO,
                NSCompositingOperation::SourceOver,
                1.0,
            );
        });
        drawing_appearance.performAsCurrentDrawingAppearance(&draw);
        NSGraphicsContext::restoreGraphicsState_class();

        let (mut sum_r, mut sum_g, mut sum_b) = (0.0, 0.0, 0.0);
        let mut count = 0usize;
        for x in 0..side {
            for y in 0..side {
                let Some(color) = rep.colorAtX_y(x, y) else {
                    continue;
                };
                let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
                unsafe {
                    color.getRed_green_blue_alpha(&mut r, &mut g, &mut b, &mut a);
                }
                if a > 0.5 {
                    sum_r += r;
                    sum_g += g;
                    sum_b += b;
                    count += 1;
                }
            }
        }
        if count == 0 {
            return (0.0, 0.0, 0.0, 0);
        }
        let n = count as f64;
        (sum_r / n, sum_g / n, sum_b / n, count)
    }

    #[test]
    fn rising_fast_icon_created_in_light_mode_stays_visible_in_dark_menu_bars() {
        // The RisingFast composite carries the Neutral accent (labelColor).
        // Hierarchical SF Symbol tints bake the creation-time appearance, so
        // the tinted symbols must be built inside the drawing handler — an
        // icon created under a light appearance must still draw light glyphs
        // when the menu bar is dark.
        let light = appearance(unsafe { NSAppearanceNameAqua });
        let dark = appearance(unsafe { NSAppearanceNameDarkAqua });

        let built = RefCell::new(None);
        {
            let build = RcBlock::new(|| {
                *built.borrow_mut() = make_status_image(
                    "gauge.with.dots.needle.50percent",
                    MemoryTrend::RisingFast,
                    Accent::Neutral,
                );
            });
            light.performAsCurrentDrawingAppearance(&build);
        }
        let status = built.into_inner().expect("status image");
        assert!(!status.template, "RisingFast icon is a colored composite");

        // Rasterize under light first — the real-world sequence is "drawn in
        // a light menu bar, then the appearance flips" — so a cached
        // light-baked rendering would be reused by the dark draw below.
        let (_, _, _, light_count) = sample_opaque_average(&status.image, &light);
        assert!(light_count > 0, "composite drew nothing under light");

        let (r, g, b, count) = sample_opaque_average(&status.image, &dark);
        assert!(count > 0, "composite drew no opaque pixels");
        assert!(
            r > 0.5 && g > 0.5 && b > 0.5,
            "dark menu bar must not keep a light-baked black glyph, got avg rgba({r:.2},{g:.2},{b:.2}) over {count} px"
        );
    }
}
