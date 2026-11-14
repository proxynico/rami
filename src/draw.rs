//! Drawing shared by the dropdown's custom views and swatch images. Every
//! function here must run inside a draw so catalog and mark colors resolve
//! against the current appearance.

use crate::format::{Accent, Swatch};
use crate::presentation::mark_color;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::Message;
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName,
    NSGraphicsContext, NSStringDrawing,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

const HATCH_SPACING: f64 = 3.0;

fn text_attrs(font: &NSFont, color: &NSColor) -> Retained<NSDictionary<NSString, AnyObject>> {
    unsafe {
        let color_obj = Retained::cast_unchecked::<AnyObject>(color.retain());
        let font_obj = Retained::cast_unchecked::<AnyObject>(font.retain());
        NSDictionary::from_retained_objects(
            &[NSForegroundColorAttributeName, NSFontAttributeName],
            &[color_obj, font_obj],
        )
    }
}

pub(crate) fn text_width(text: &str, font: &NSFont) -> f64 {
    let attrs = text_attrs(font, &NSColor::labelColor());
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attrs)) }.width
}

pub(crate) fn draw_text(text: &str, x: f64, y: f64, font: &NSFont, color: &NSColor) {
    let attrs = text_attrs(font, color);
    unsafe {
        NSString::from_str(text).drawAtPoint_withAttributes(NSPoint::new(x, y), Some(&attrs));
    }
}

pub(crate) fn draw_text_right(text: &str, right: f64, y: f64, font: &NSFont, color: &NSColor) {
    draw_text(text, right - text_width(text, font), y, font, color);
}

pub(crate) fn centered_text_y(height: f64) -> f64 {
    (height - 16.0) / 2.0
}

pub(crate) fn fill_swatch(rect: NSRect, swatch: Swatch, accent: Accent, radius: f64) {
    let shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius);
    match swatch {
        Swatch::Accent(opacity_percent) => {
            mark_color(accent)
                .colorWithAlphaComponent(f64::from(opacity_percent) / 100.0)
                .setFill();
            shape.fill();
        }
        Swatch::Neutral => {
            NSColor::tertiaryLabelColor().setFill();
            shape.fill();
        }
        Swatch::Empty => {
            NSColor::quaternaryLabelColor().setFill();
            shape.fill();
        }
        Swatch::Hatched => {
            let mark = mark_color(accent);
            if let Some(context) = NSGraphicsContext::currentContext() {
                context.saveGraphicsState();
                shape.addClip();
                let stripes = NSBezierPath::bezierPath();
                let mut x = rect.origin.x - rect.size.height;
                while x < rect.origin.x + rect.size.width {
                    stripes.moveToPoint(NSPoint::new(x, rect.origin.y));
                    stripes.lineToPoint(NSPoint::new(
                        x + rect.size.height,
                        rect.origin.y + rect.size.height,
                    ));
                    x += HATCH_SPACING;
                }
                stripes.setLineWidth(1.2);
                mark.colorWithAlphaComponent(0.5).setStroke();
                stripes.stroke();
                context.restoreGraphicsState();
            }
            let outline = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                NSRect::new(
                    NSPoint::new(rect.origin.x + 0.5, rect.origin.y + 0.5),
                    NSSize::new(rect.size.width - 1.0, rect.size.height - 1.0),
                ),
                radius,
                radius,
            );
            outline.setLineWidth(1.0);
            mark.colorWithAlphaComponent(0.35).setStroke();
            outline.stroke();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use block2::RcBlock;
    use objc2::AnyThread;
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameDarkAqua, NSBitmapImageRep, NSDeviceRGBColorSpace,
    };
    use objc2_foundation::NSInteger;

    pub(crate) fn rasterize(
        width: NSInteger,
        height: NSInteger,
        appearance: Option<&NSAppearance>,
        draw: impl Fn(),
    ) -> impl Fn(NSInteger, NSInteger) -> (f64, f64, f64, f64) {
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                width,
                height,
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
        match appearance {
            Some(appearance) => appearance.performAsCurrentDrawingAppearance(&RcBlock::new(draw)),
            None => draw(),
        }
        NSGraphicsContext::restoreGraphicsState_class();
        move |x, y| {
            let color = rep.colorAtX_y(x, y).expect("pixel");
            let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
            unsafe {
                color.getRed_green_blue_alpha(&mut r, &mut g, &mut b, &mut a);
            }
            (r, g, b, a)
        }
    }

    #[test]
    fn orange_swatch_resolves_to_dark_orange_in_a_dark_menu() {
        // A mark color resolved outside the draw keeps whichever mode was
        // active then. The swatch must pick orange for the drawing appearance.
        let dark = NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua })
            .expect("dark appearance");
        let (r, g, b, a) = rasterize(4, 4, Some(&dark), || {
            let rect = NSRect::new(NSPoint::ZERO, NSSize::new(4.0, 4.0));
            fill_swatch(rect, Swatch::Accent(100), Accent::Neutral, 0.0);
        })(1, 1);
        assert!(
            r > 0.8 && (0.45..0.65).contains(&g) && b < 0.4,
            "dark menu must draw the dark-mode orange, got rgba({r:.2},{g:.2},{b:.2},{a:.2})"
        );
    }
}
