//! The memory-history row: a filled-area sparkline of the trend window,
//! restored by the ADR-0001 amendment (#26). One row, memory-only — the
//! amended bound allows exactly this view and nothing more. Captions under
//! the mark show current used and the signed window delta.

use crate::format::{history_caption, Accent};
use crate::presentation::{ChromeColor, HistoryLayout, MenuMetrics};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontAttributeName, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSStringDrawing, NSView,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;
/// Below this byte span the sparkline collapses to a midline so tiny page
/// jitter does not fill the band. Deliberately lower than the trend
/// classifier's Rising threshold (300 MB): the mark is allowed to swing
/// while the badge still reads Stable.
const SPAN_FLOOR_BYTES: u64 = 32_000_000;
/// Opacity-ramp stops for the mark (CONTEXT.md): faint fill, mid line,
/// full-strength "now" dot.
const FILL_ALPHA: f64 = 0.12;
const LINE_ALPHA: f64 = 0.65;
const LINE_WIDTH: f64 = 1.5;
const NOW_DOT_RADIUS: f64 = 2.0;

/// Normalize the sample window into unit-square points, oldest at x=0,
/// newest at x=1. Returns `None` when there are not yet two samples to
/// connect. A span below `span_floor` collapses to the midline.
fn normalized_points(samples: &[u64], span_floor: u64) -> Option<Vec<(f64, f64)>> {
    if samples.len() < 2 {
        return None;
    }
    let min = *samples.iter().min().expect("len checked above");
    let max = *samples.iter().max().expect("len checked above");
    let span = max - min;
    let last_index = (samples.len() - 1) as f64;
    Some(
        samples
            .iter()
            .enumerate()
            .map(|(index, &value)| {
                let x = index as f64 / last_index;
                let y = if span < span_floor {
                    0.5
                } else {
                    (value - min) as f64 / span as f64
                };
                (x, y)
            })
            .collect(),
    )
}

struct HistoryState {
    samples: Vec<u64>,
    chrome: ChromeColor,
}

pub struct MemoryHistoryIvars {
    metrics: MenuMetrics,
    state: RefCell<HistoryState>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MemoryHistoryIvars]
    pub struct MemoryHistoryView;

    impl MemoryHistoryView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render();
        }
    }

    unsafe impl NSObjectProtocol for MemoryHistoryView {}
);

impl MemoryHistoryView {
    pub fn new(mtm: MainThreadMarker, metrics: MenuMetrics) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, metrics.history_layout().view_size());
        let this = Self::alloc(mtm).set_ivars(MemoryHistoryIvars {
            metrics,
            state: RefCell::new(HistoryState {
                samples: Vec::new(),
                chrome: ChromeColor::resolve(Accent::Neutral),
            }),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let role = NSString::from_str("AXGroup");
        let label = NSString::from_str("Memory history");
        unsafe {
            let _: () = msg_send![&*view, setAccessibilityElement: true];
            let _: () = msg_send![&*view, setAccessibilityRole: &*role];
            let _: () = msg_send![&*view, setAccessibilityLabel: &*label];
        }
        view
    }

    pub fn update(&self, samples: &[u64], chrome: ChromeColor) {
        let value = match history_caption(samples) {
            Some((current, delta)) => format!("{current} used, {delta} over the last two minutes"),
            None => "collecting samples".to_string(),
        };
        *self.ivars().state.borrow_mut() = HistoryState {
            samples: samples.to_vec(),
            chrome,
        };
        let value = NSString::from_str(&value);
        unsafe {
            let _: () = msg_send![self, setAccessibilityValue: &*value];
        }
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let state = self.ivars().state.borrow();
        let layout = self.ivars().metrics.history_layout();
        self.draw_caption(&state.samples, &layout, &state.chrome);

        let Some(points) = normalized_points(&state.samples, SPAN_FLOOR_BYTES) else {
            let baseline = NSBezierPath::bezierPath();
            baseline.moveToPoint(NSPoint::new(layout.band_left, layout.band_bottom));
            baseline.lineToPoint(NSPoint::new(layout.band_right, layout.band_bottom));
            baseline.setLineWidth(1.0);
            NSColor::quaternaryLabelColor().setStroke();
            baseline.stroke();
            return;
        };

        let to_view = |&(x, y): &(f64, f64)| {
            NSPoint::new(
                layout.band_left + x * layout.band_width(),
                layout.band_bottom + y * (layout.band_top - layout.band_bottom),
            )
        };

        let area = NSBezierPath::bezierPath();
        area.moveToPoint(to_view(&points[0]));
        for point in &points[1..] {
            area.lineToPoint(to_view(point));
        }
        area.lineToPoint(NSPoint::new(layout.band_right, layout.band_bottom));
        area.lineToPoint(NSPoint::new(layout.band_left, layout.band_bottom));
        area.closePath();
        state
            .chrome
            .as_nscolor()
            .colorWithAlphaComponent(FILL_ALPHA)
            .setFill();
        area.fill();

        let line = NSBezierPath::bezierPath();
        line.moveToPoint(to_view(&points[0]));
        for point in &points[1..] {
            line.lineToPoint(to_view(point));
        }
        line.setLineWidth(LINE_WIDTH);
        state
            .chrome
            .as_nscolor()
            .colorWithAlphaComponent(LINE_ALPHA)
            .setStroke();
        line.stroke();

        let newest = to_view(points.last().expect("normalized_points is non-empty"));
        let dot = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(newest.x - NOW_DOT_RADIUS, newest.y - NOW_DOT_RADIUS),
            NSSize::new(NOW_DOT_RADIUS * 2.0, NOW_DOT_RADIUS * 2.0),
        ));
        state.chrome.as_nscolor().setFill();
        dot.fill();
    }

    fn draw_caption(&self, samples: &[u64], layout: &HistoryLayout, chrome: &ChromeColor) {
        // drawRect: alpha on the resolved chrome tracks appearance. Same
        // 0.65 stop as the sparkline stroke so captions stay secondary to the mark.
        let color = chrome.as_nscolor().colorWithAlphaComponent(LINE_ALPHA);
        let Some((current, delta)) = history_caption(samples) else {
            draw_caption_text(
                "…",
                layout.band_left,
                layout.caption_y,
                layout.caption_size,
                &color,
            );
            return;
        };
        draw_caption_text(
            &current,
            layout.band_left,
            layout.caption_y,
            layout.caption_size,
            &color,
        );
        let delta_attrs = caption_attrs(&color, layout.caption_size);
        let delta_str = NSString::from_str(&delta);
        let measured = unsafe { delta_str.sizeWithAttributes(Some(&delta_attrs)) };
        draw_caption_text(
            &delta,
            layout.band_right - measured.width,
            layout.caption_y,
            layout.caption_size,
            &color,
        );
    }
}

fn caption_attrs(color: &NSColor, size: f64) -> Retained<NSDictionary<NSString, AnyObject>> {
    let weight = unsafe { NSFontWeightRegular };
    let font = NSFont::monospacedDigitSystemFontOfSize_weight(size, weight);
    unsafe {
        let color_obj = Retained::cast_unchecked::<AnyObject>(color.retain());
        let font_obj = Retained::cast_unchecked::<AnyObject>(font);
        NSDictionary::from_retained_objects(
            &[NSForegroundColorAttributeName, NSFontAttributeName],
            &[color_obj, font_obj],
        )
    }
}

fn draw_caption_text(text: &str, x: f64, y: f64, size: f64, color: &NSColor) {
    let attrs = caption_attrs(color, size);
    let text = NSString::from_str(text);
    unsafe {
        text.drawAtPoint_withAttributes(NSPoint::new(x, y), Some(&attrs));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fewer_than_two_samples_have_no_line_to_draw() {
        assert!(normalized_points(&[], SPAN_FLOOR_BYTES).is_none());
        assert!(normalized_points(&[1_000_000_000], SPAN_FLOOR_BYTES).is_none());
    }

    #[test]
    fn window_normalizes_to_unit_square_oldest_to_newest() {
        let points = normalized_points(
            &[1_000_000_000, 1_500_000_000, 2_000_000_000],
            SPAN_FLOOR_BYTES,
        )
        .unwrap();
        assert_eq!(points, vec![(0.0, 0.0), (0.5, 0.5), (1.0, 1.0)]);
    }

    #[test]
    fn idle_jitter_below_the_span_floor_reads_flat() {
        // A few MB of page wobble should stay flat; meaningful window moves
        // above SPAN_FLOOR_BYTES get the full band even when trend is Stable.
        let points = normalized_points(
            &[1_000_000_000, 1_003_000_000, 999_000_000],
            SPAN_FLOOR_BYTES,
        )
        .unwrap();
        assert!(points.iter().all(|&(_, y)| y == 0.5));
    }

    #[test]
    fn span_at_the_floor_uses_the_full_band() {
        let points = normalized_points(
            &[1_000_000_000, 1_000_000_000 + SPAN_FLOOR_BYTES],
            SPAN_FLOOR_BYTES,
        )
        .unwrap();
        assert_eq!(points, vec![(0.0, 0.0), (1.0, 1.0)]);
    }
}
