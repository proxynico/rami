use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBezierPath, NSColor, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

const VIEW_WIDTH: f64 = 240.0;
const VIEW_HEIGHT: f64 = 30.0;
const PADDING: f64 = 4.0;
const LINE_WIDTH: f64 = 1.5;
const FILL_ALPHA: f64 = 0.22;

pub struct SparklineIvars {
    samples: RefCell<Vec<u64>>,
    accent: RefCell<Retained<NSColor>>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = SparklineIvars]
    pub struct SparklineView;

    impl SparklineView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render();
        }
    }

    unsafe impl NSObjectProtocol for SparklineView {}
);

impl SparklineView {
    pub fn new(mtm: MainThreadMarker, samples: Vec<u64>) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(VIEW_WIDTH, VIEW_HEIGHT));
        let this = Self::alloc(mtm).set_ivars(SparklineIvars {
            samples: RefCell::new(samples),
            accent: RefCell::new(NSColor::controlAccentColor()),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// Replace the history window and trigger a redraw.
    pub fn update(&self, samples: Vec<u64>, accent: Retained<NSColor>) {
        *self.ivars().samples.borrow_mut() = samples;
        *self.ivars().accent.borrow_mut() = accent;
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let bounds = self.bounds();
        let width = bounds.size.width;
        let height = bounds.size.height;
        let samples = self.ivars().samples.borrow();
        let accent = self.ivars().accent.borrow();

        let pts = normalized_points(&samples, width, height, PADDING);

        if pts.len() < 2 {
            // Not enough history for a shape yet: draw a faint baseline so the
            // row reads as an empty graph rather than a blank menu slot.
            let y = height / 2.0;
            let line = NSBezierPath::bezierPath();
            line.moveToPoint(NSPoint::new(0.0, y));
            line.lineToPoint(NSPoint::new(width, y));
            NSColor::secondaryLabelColor()
                .colorWithAlphaComponent(0.35)
                .setStroke();
            line.setLineWidth(1.0);
            line.stroke();
            return;
        }

        // Filled area under the curve.
        let area = NSBezierPath::bezierPath();
        area.moveToPoint(NSPoint::new(pts[0].0, 0.0));
        for &(x, y) in &pts {
            area.lineToPoint(NSPoint::new(x, y));
        }
        area.lineToPoint(NSPoint::new(pts[pts.len() - 1].0, 0.0));
        area.closePath();
        accent.colorWithAlphaComponent(FILL_ALPHA).setFill();
        area.fill();

        // Stroke the top line on top of the fill.
        let line = NSBezierPath::bezierPath();
        line.moveToPoint(NSPoint::new(pts[0].0, pts[0].1));
        for &(x, y) in &pts {
            line.lineToPoint(NSPoint::new(x, y));
        }
        line.setLineWidth(LINE_WIDTH);
        accent.setStroke();
        line.stroke();
    }
}

/// Normalize a memory-history window into (x, y) points within the view's
/// drawing rect. x spans the full width oldest..newest; y maps the min..max
/// range into the padded height. A flat window maps to the vertical midpoint
/// so a steady state still reads as a line.
fn normalized_points(samples: &[u64], width: f64, height: f64, padding: f64) -> Vec<(f64, f64)> {
    let n = samples.len();
    if n == 0 {
        return Vec::new();
    }
    let min = *samples.iter().min().unwrap();
    let max = *samples.iter().max().unwrap();
    let span = (max - min) as f64;
    let usable = (height - 2.0 * padding).max(0.0);
    (0..n)
        .map(|i| {
            let x = if n == 1 {
                width / 2.0
            } else {
                (i as f64 / (n - 1) as f64) * width
            };
            let frac = if span == 0.0 {
                0.5
            } else {
                (samples[i] - min) as f64 / span
            };
            let y = padding + frac * usable;
            (x, y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_samples_yields_no_points() {
        assert!(normalized_points(&[], 240.0, 30.0, 4.0).is_empty());
    }

    #[test]
    fn single_sample_maps_to_horizontal_midpoint() {
        let pts = normalized_points(&[42], 240.0, 30.0, 4.0);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0].0, 120.0);
        // flat window -> midpoint of padded area
        assert_eq!(pts[0].1, 4.0 + 0.5 * (30.0 - 8.0));
    }

    #[test]
    fn rising_window_spans_bottom_to_top() {
        let pts = normalized_points(&[0, 50, 100], 240.0, 30.0, 4.0);
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0].0, 0.0);
        assert_eq!(pts[1].0, 120.0);
        assert_eq!(pts[2].0, 240.0);
        // min at the bottom padding, max at the top edge of usable area
        assert_eq!(pts[0].1, 4.0);
        assert_eq!(pts[2].1, 4.0 + (30.0 - 8.0));
        assert!(pts[1].1 > pts[0].1 && pts[1].1 < pts[2].1);
    }

    #[test]
    fn flat_window_maps_all_points_to_midline() {
        let pts = normalized_points(&[100, 100, 100, 100], 240.0, 30.0, 4.0);
        let mid = 4.0 + 0.5 * (30.0 - 8.0);
        assert!(pts.iter().all(|(_, y)| (*y - mid).abs() < f64::EPSILON));
    }

    #[test]
    fn point_count_matches_samples() {
        let samples = [10_u64, 20, 30, 40, 50, 60, 70];
        assert_eq!(
            normalized_points(&samples, 240.0, 30.0, 4.0).len(),
            samples.len()
        );
    }
}
