//! The memory-history row: a filled-area sparkline of the trend window,
//! restored by the ADR-0001 amendment (#26). One row, memory-only — the
//! amended bound allows exactly this view and nothing more.

use crate::format::mem_text;
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBezierPath, NSColor, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;

const VIEW_WIDTH: f64 = 240.0;
const VIEW_HEIGHT: f64 = 28.0;
const BAND_LEFT: f64 = 16.0;
const BAND_RIGHT: f64 = VIEW_WIDTH - 16.0;
const BAND_BOTTOM: f64 = 6.0;
const BAND_TOP: f64 = VIEW_HEIGHT - 6.0;
/// Windows whose byte span stays under this render as a flat midline, so idle
/// jitter reads as flat. Mirrors the trend classifier's Rising threshold —
/// the sparkline must not dramatize what the trend badge calls Stable.
const SPAN_FLOOR_BYTES: u64 = 300_000_000;
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
    accent: Retained<NSColor>,
}

pub struct MemoryHistoryIvars {
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
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(VIEW_WIDTH, VIEW_HEIGHT));
        let this = Self::alloc(mtm).set_ivars(MemoryHistoryIvars {
            state: RefCell::new(HistoryState {
                samples: Vec::new(),
                accent: NSColor::labelColor(),
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

    pub fn update(&self, samples: &[u64], accent: Retained<NSColor>) {
        let value = match (samples.iter().min(), samples.iter().max()) {
            (Some(&min), Some(&max)) if samples.len() >= 2 => format!(
                "{} to {} used over the last two minutes",
                mem_text(min),
                mem_text(max)
            ),
            _ => "collecting samples".to_string(),
        };
        *self.ivars().state.borrow_mut() = HistoryState {
            samples: samples.to_vec(),
            accent,
        };
        let value = NSString::from_str(&value);
        unsafe {
            let _: () = msg_send![self, setAccessibilityValue: &*value];
        }
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let state = self.ivars().state.borrow();
        let Some(points) = normalized_points(&state.samples, SPAN_FLOOR_BYTES) else {
            // Warming up: a faint baseline so the row reads as an empty graph,
            // never a blank slot.
            let baseline = NSBezierPath::bezierPath();
            baseline.moveToPoint(NSPoint::new(BAND_LEFT, BAND_BOTTOM));
            baseline.lineToPoint(NSPoint::new(BAND_RIGHT, BAND_BOTTOM));
            baseline.setLineWidth(1.0);
            NSColor::quaternaryLabelColor().setStroke();
            baseline.stroke();
            return;
        };

        let to_view = |&(x, y): &(f64, f64)| {
            NSPoint::new(
                BAND_LEFT + x * (BAND_RIGHT - BAND_LEFT),
                BAND_BOTTOM + y * (BAND_TOP - BAND_BOTTOM),
            )
        };

        // Area fill first, then the line over it, then the "now" dot.
        let area = NSBezierPath::bezierPath();
        area.moveToPoint(to_view(&points[0]));
        for point in &points[1..] {
            area.lineToPoint(to_view(point));
        }
        area.lineToPoint(NSPoint::new(BAND_RIGHT, BAND_BOTTOM));
        area.lineToPoint(NSPoint::new(BAND_LEFT, BAND_BOTTOM));
        area.closePath();
        state.accent.colorWithAlphaComponent(FILL_ALPHA).setFill();
        area.fill();

        let line = NSBezierPath::bezierPath();
        line.moveToPoint(to_view(&points[0]));
        for point in &points[1..] {
            line.lineToPoint(to_view(point));
        }
        line.setLineWidth(LINE_WIDTH);
        state.accent.colorWithAlphaComponent(LINE_ALPHA).setStroke();
        line.stroke();

        let newest = to_view(points.last().expect("normalized_points is non-empty"));
        let dot = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(newest.x - NOW_DOT_RADIUS, newest.y - NOW_DOT_RADIUS),
            NSSize::new(NOW_DOT_RADIUS * 2.0, NOW_DOT_RADIUS * 2.0),
        ));
        state.accent.setFill();
        dot.fill();
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
        // A few MB of wobble is Stable to the trend classifier; the sparkline
        // must agree instead of stretching noise across the full band.
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
