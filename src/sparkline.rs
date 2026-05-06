use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_app_kit::{NSBezierPath, NSColor, NSImage};
use objc2_foundation::{NSPoint, NSSize};

const PADDING_Y: f64 = 1.0;
const LINE_WIDTH: f64 = 1.0;
const FILL_ALPHA: f64 = 0.18;

pub fn render(samples: &[u8], width: f64, height: f64) -> Option<Retained<NSImage>> {
    if samples.is_empty() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let size = NSSize::new(width, height);
    let image = NSImage::initWithSize(NSImage::alloc(), size);
    let plot_h = (height - 2.0 * PADDING_Y).max(1.0);
    let n = samples.len();
    let line_path = NSBezierPath::bezierPath();
    let fill_path = NSBezierPath::bezierPath();
    fill_path.moveToPoint(NSPoint::new(0.0, 0.0));

    for (i, &percent) in samples.iter().enumerate() {
        let x = if n == 1 {
            width / 2.0
        } else {
            i as f64 * (width / (n as f64 - 1.0))
        };
        let y = PADDING_Y + plot_h * (percent.min(100) as f64 / 100.0);
        let point = NSPoint::new(x, y);
        if i == 0 {
            line_path.moveToPoint(point);
            fill_path.lineToPoint(point);
        } else {
            line_path.lineToPoint(point);
            fill_path.lineToPoint(point);
        }
    }
    fill_path.lineToPoint(NSPoint::new(width, 0.0));
    fill_path.closePath();
    line_path.setLineWidth(LINE_WIDTH);

    let stroke_color = NSColor::systemBlueColor();
    let fill_color = stroke_color.colorWithAlphaComponent(FILL_ALPHA);

    #[allow(deprecated)]
    image.lockFocus();
    fill_color.set();
    fill_path.fill();
    stroke_color.set();
    line_path.stroke();
    #[allow(deprecated)]
    image.unlockFocus();

    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_returns_none_for_empty_samples() {
        assert!(render(&[], 100.0, 16.0).is_none());
    }

    #[test]
    fn render_returns_none_for_zero_dimensions() {
        assert!(render(&[10, 20, 30], 0.0, 16.0).is_none());
        assert!(render(&[10, 20, 30], 100.0, 0.0).is_none());
    }
}
