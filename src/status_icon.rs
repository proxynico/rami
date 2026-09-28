use crate::format::Accent;
use crate::presentation::{color_for_accent, rising_fast_badge_color};
use crate::trend::MemoryTrend;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::{
    NSBezierPath, NSColor, NSCompositingOperation, NSGraphicsContext, NSImage,
    NSImageSymbolConfiguration, NSImageSymbolScale,
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

/// The status glyph's design grid in points: a RAM stick whose four chips
/// fill left to right with Memory %, over four short legs.
const GLYPH_WIDTH: f64 = 22.0;
const GLYPH_HEIGHT: f64 = 16.0;
const CHIPS_LEFT: f64 = 3.0;
const CHIP_WIDTH: f64 = 3.1;
const CHIP_GAP: f64 = 1.2;
const LEG_WIDTH: f64 = 1.6;
const EMPTY_CHIP_ALPHA: f64 = 0.3;

fn chip_left(k: u8) -> f64 {
    CHIPS_LEFT + f64::from(k) * (CHIP_WIDTH + CHIP_GAP)
}

pub(crate) fn make_status_image(
    used_percent: u8,
    trend: MemoryTrend,
    accent: Accent,
) -> StatusImage {
    let level = f64::from(used_percent.min(100)) / 100.0;
    let rising = badge_for_state(trend) == BadgeKind::RisingFast;
    // Only the calm, steady glyph is a template: macOS picks black or white
    // for it from its alpha. The status button's content tint does not
    // recolor a template image in the menu bar, so Warning, Critical, and
    // RisingFast draw their own colors, resolved against the appearance here.
    let tinted = rising || accent != Accent::Neutral;
    let handler = RcBlock::new(move |rect: NSRect| -> Bool {
        let scale = rect.size.width / GLYPH_WIDTH;
        let at = |x: f64, y: f64, w: f64, h: f64| {
            NSRect::new(
                NSPoint::new(rect.origin.x + x * scale, rect.origin.y + y * scale),
                NSSize::new(w * scale, h * scale),
            )
        };
        let ink = if tinted {
            color_for_accent(accent)
        } else {
            NSColor::blackColor()
        };

        let body = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            at(1.1, 2.1, GLYPH_WIDTH - 2.2, 8.8),
            2.0 * scale,
            2.0 * scale,
        );
        body.setLineWidth(1.3 * scale);
        ink.setStroke();
        body.stroke();

        let chips: Vec<NSRect> = (0..4)
            .map(|k| at(chip_left(k), 4.3, CHIP_WIDTH, 4.4))
            .collect();
        ink.colorWithAlphaComponent(EMPTY_CHIP_ALPHA).setFill();
        for chip in &chips {
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                *chip,
                0.7 * scale,
                0.7 * scale,
            )
            .fill();
        }
        let span = 4.0 * CHIP_WIDTH + 3.0 * CHIP_GAP;
        NSGraphicsContext::saveGraphicsState_class();
        NSBezierPath::clipRect(at(CHIPS_LEFT, 0.0, span * level, GLYPH_HEIGHT));
        ink.setFill();
        for chip in &chips {
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                *chip,
                0.7 * scale,
                0.7 * scale,
            )
            .fill();
        }
        NSGraphicsContext::restoreGraphicsState_class();

        ink.setFill();
        for k in 0..4 {
            let center = chip_left(k) + CHIP_WIDTH / 2.0;
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                at(center - LEG_WIDTH / 2.0, 11.2, LEG_WIDTH, 3.0),
                0.5 * scale,
                0.5 * scale,
            )
            .fill();
        }

        if rising {
            draw_rising_badge(at(GLYPH_WIDTH - 6.2, -0.2, 6.8, 6.8), accent);
        }
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(
        NSSize::new(GLYPH_WIDTH, GLYPH_HEIGHT),
        true,
        &handler,
    );
    StatusImage {
        image,
        template: !tinted,
    }
}

/// The RisingFast badge in the glyph's top-right corner, cut out of the
/// stick so the two shapes stay distinct.
fn draw_rising_badge(rect: NSRect, accent: Accent) {
    let symbol_name = NSString::from_str("arrow.up.right.circle.fill");
    let desc = NSString::from_str("");
    let Some(badge) =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))
            .and_then(|symbol| {
                let scale =
                    NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Small);
                let tint = NSImageSymbolConfiguration::configurationWithHierarchicalColor(
                    &rising_fast_badge_color(accent),
                );
                symbol.imageWithSymbolConfiguration(
                    &scale.configurationByApplyingConfiguration(&tint),
                )
            })
    else {
        return;
    };
    let cutout = NSRect::new(
        NSPoint::new(rect.origin.x - 0.8, rect.origin.y - 0.8),
        NSSize::new(rect.size.width + 1.6, rect.size.height + 1.6),
    );
    if let Some(context) = NSGraphicsContext::currentContext() {
        context.saveGraphicsState();
        context.setCompositingOperation(NSCompositingOperation::Clear);
        NSBezierPath::bezierPathWithOvalInRect(cutout).fill();
        context.restoreGraphicsState();
    }
    // The glyph's handler draws top-down; the plain draw call would mirror
    // the arrow vertically.
    unsafe {
        badge.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
            rect,
            NSRect::ZERO,
            NSCompositingOperation::SourceOver,
            1.0,
            true,
            None,
        );
    }
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
        // the tinted glyph must be drawn inside the drawing handler — an
        // icon created under a light appearance must still draw light glyphs
        // when the menu bar is dark.
        let light = appearance(unsafe { NSAppearanceNameAqua });
        let dark = appearance(unsafe { NSAppearanceNameDarkAqua });

        let built = RefCell::new(None);
        {
            let build = RcBlock::new(|| {
                *built.borrow_mut() = Some(make_status_image(
                    50,
                    MemoryTrend::RisingFast,
                    Accent::Neutral,
                ));
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

    #[test]
    fn status_glyph_fills_its_chips_up_to_memory_percent() {
        let status = make_status_image(30, MemoryTrend::Stable, Accent::Neutral);
        assert!(status.template, "the calm glyph is a template image");
        assert!(
            !make_status_image(30, MemoryTrend::Stable, Accent::Critical).template,
            "Critical draws its own red; a content tint would leave a template glyph black"
        );

        let (width, height) = (GLYPH_WIDTH * 2.0, GLYPH_HEIGHT * 2.0);
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                width as NSInteger,
                height as NSInteger,
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
        status.image.drawInRect_fromRect_operation_fraction(
            NSRect::new(NSPoint::ZERO, NSSize::new(width, height)),
            NSRect::ZERO,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        NSGraphicsContext::restoreGraphicsState_class();
        let alpha_at = |x: f64, y: f64| {
            let color = rep
                .colorAtX_y((x * 2.0) as NSInteger, (y * 2.0) as NSInteger)
                .expect("pixel");
            color.alphaComponent()
        };

        let first_chip = alpha_at(chip_left(0) + CHIP_WIDTH / 2.0, 6.5);
        let last_chip = alpha_at(chip_left(3) + CHIP_WIDTH / 2.0, 6.5);
        assert!(
            first_chip > 0.9,
            "30% fills the first chip, got alpha {first_chip:.2}"
        );
        assert!(
            (0.1..0.5).contains(&last_chip),
            "30% leaves the last chip at the empty step, got alpha {last_chip:.2}"
        );
    }
}
