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
