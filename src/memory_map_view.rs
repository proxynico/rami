//! The memory map: a header, RAM drawn as a board of equal cells in category
//! order, a contact strip, and the cell size. Used cells are solid, Cached is
//! hatched, Free is empty.

use crate::draw::{draw_text, draw_text_right, fill_swatch, text_width};
use crate::format::{Accent, MemoryMapDisplay};
use crate::presentation::{mark_color, MenuMetrics};
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontWeightRegular, NSFontWeightSemibold, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;

/// Share of the strip width before the key notch, off-center like a DIMM's.
const NOTCH_AT: f64 = 0.4;
const NOTCH_WIDTH: f64 = 6.0;
const STRIP_INSET: f64 = 10.0;
const HEADER_GAP: f64 = 8.0;
const MIN_FIGURE_SIZE: f64 = 10.0;

struct MapState {
    map: Option<MemoryMapDisplay>,
    accent: Accent,
}

pub struct MemoryMapIvars {
    metrics: MenuMetrics,
    state: RefCell<MapState>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MemoryMapIvars]
    pub struct MemoryMapView;

    impl MemoryMapView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render();
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }

    unsafe impl NSObjectProtocol for MemoryMapView {}
);

impl MemoryMapView {
    pub fn new(mtm: MainThreadMarker, metrics: MenuMetrics) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, metrics.map_layout().view_size());
        let this = Self::alloc(mtm).set_ivars(MemoryMapIvars {
            metrics,
            state: RefCell::new(MapState {
                map: None,
                accent: Accent::Neutral,
            }),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let role = NSString::from_str("AXGroup");
        let label = NSString::from_str("Memory");
        unsafe {
            let _: () = msg_send![&*view, setAccessibilityElement: true];
            let _: () = msg_send![&*view, setAccessibilityRole: &*role];
            let _: () = msg_send![&*view, setAccessibilityLabel: &*label];
        }
        view
    }

    pub fn update(&self, map: &MemoryMapDisplay, accent: Accent) {
        let value = NSString::from_str(&format!(
            "{} percent, {} used, {}",
            map.used_percent, map.used_of_total, map.cell_caption
        ));
        unsafe {
            let _: () = msg_send![self, setAccessibilityValue: &*value];
        }
        *self.ivars().state.borrow_mut() = MapState {
            map: Some(map.clone()),
            accent,
        };
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let state = self.ivars().state.borrow();
        let Some(map) = &state.map else {
            return;
        };
        let layout = self.ivars().metrics.map_layout();
        let scale = self.ivars().metrics.type_scale;

        let title_font =
            NSFont::systemFontOfSize_weight(scale.map_title, unsafe { NSFontWeightSemibold });
        let figures = format!("{} · {}%", map.used_of_total, map.used_percent);
        let figure_font = header_figure_font(
            &figures,
            scale.map_header,
            layout.right - layout.left - text_width("Memory", &title_font) - HEADER_GAP,
        );
        draw_text(
            "Memory",
            layout.left,
            layout.header_y,
            &title_font,
            &NSColor::labelColor(),
        );
        draw_text_right(
            &figures,
            layout.right,
            layout.header_y + 1.0,
            &figure_font,
            &NSColor::secondaryLabelColor(),
        );

        let board = NSRect::new(
            NSPoint::new(layout.left + 0.5, layout.board_top + 0.5),
            NSSize::new(
                layout.right - layout.left - 1.0,
                layout.board_height() - 1.0,
            ),
        );
        let board_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(board, 4.0, 4.0);
        NSColor::labelColor()
            .colorWithAlphaComponent(0.04)
            .setFill();
        board_path.fill();
        NSColor::labelColor()
            .colorWithAlphaComponent(0.12)
            .setStroke();
        board_path.setLineWidth(1.0);
        board_path.stroke();

        let cell_width = layout.cell_width();
        for (index, category) in map.cells.iter().enumerate() {
            let column = (index / layout.rows) as f64;
            let row = (index % layout.rows) as f64;
            let cell = NSRect::new(
                NSPoint::new(
                    layout.left + layout.board_padding + column * (cell_width + layout.cell_gap),
                    layout.board_top
                        + layout.board_padding
                        + row * (layout.cell_height + layout.cell_gap),
                ),
                NSSize::new(cell_width, layout.cell_height),
            );
            fill_swatch(cell, category.swatch(), state.accent, 2.0);
        }

        let strip_left = layout.left + STRIP_INSET;
        let strip_right = layout.right - STRIP_INSET;
        let notch = strip_left + (strip_right - strip_left) * NOTCH_AT;
        mark_color(state.accent)
            .colorWithAlphaComponent(0.55)
            .setFill();
        for (from, to) in [
            (strip_left, notch - NOTCH_WIDTH / 2.0),
            (notch + NOTCH_WIDTH / 2.0, strip_right),
        ] {
            NSBezierPath::fillRect(NSRect::new(
                NSPoint::new(from, layout.board_bottom()),
                NSSize::new(to - from, layout.strip_height),
            ));
        }

        let caption_font = NSFont::monospacedDigitSystemFontOfSize_weight(scale.caption, unsafe {
            NSFontWeightRegular
        });
        let caption_color = NSColor::secondaryLabelColor();
        draw_text(
            &map.cell_caption,
            layout.left,
            layout.caption_y,
            &caption_font,
            &caption_color,
        );
        draw_text_right(
            "hatched = cached",
            layout.right,
            layout.caption_y,
            &caption_font,
            &caption_color,
        );
    }
}

/// The header figures at their full size, or smaller when a large machine's
/// reading (e.g. "123.4 / 128.0 GB · 100%") would run into the title.
fn header_figure_font(figures: &str, size: f64, available: f64) -> Retained<NSFont> {
    let font =
        |size| NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe { NSFontWeightRegular });
    let mut size = size;
    while size > MIN_FIGURE_SIZE && text_width(figures, &font(size)) > available {
        size -= 1.0;
    }
    font(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_figures_shrink_only_when_they_would_hit_the_title() {
        let title = NSFont::systemFontOfSize_weight(15.0, unsafe { NSFontWeightSemibold });
        let available = 200.0 - text_width("Memory", &title) - HEADER_GAP;

        let usual = header_figure_font("13.0 / 16.0 GB · 81%", 13.0, available);
        assert_eq!(usual.pointSize(), 13.0);

        let wide = "123.4 / 128.0 GB · 100%";
        let shrunk = header_figure_font(wide, 13.0, available);
        assert!(shrunk.pointSize() < 13.0);
        assert!(text_width(wide, &shrunk) <= available);
    }
}
