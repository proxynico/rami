//! The shared Mac's section: its title and memory figures, its 64 map cells
//! as a two-row strip, then Pressure and Swap on one row and CPU and GPU on
//! the next. It takes the shared Mac's own accent, so only this section turns
//! red when that Mac is short of memory.

use crate::draw::{draw_text, draw_text_right, fill_swatch, text_width};
use crate::format::{SharedMacDisplay, MAP_CELLS};
use crate::memory_map_view::{header_figure_font, HEADER_GAP};
use crate::presentation::{color_for_accent_alpha, MenuMetrics};
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontWeightRegular, NSFontWeightSemibold, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;

const VIEW_HEIGHT: f64 = 94.0;
const HEADER_Y: f64 = 4.0;
const STRIP_TOP: f64 = 26.0;
const STRIP_ROWS: usize = 2;
const STRIP_PADDING: f64 = 3.0;
const CELL_HEIGHT: f64 = 6.0;
const CELL_GAP: f64 = 2.0;
const PAIR_TOP: f64 = 48.0;
const PAIR_HEIGHT: f64 = 22.0;
const PAIR_GAP: f64 = 16.0;

pub struct SharedMacIvars {
    metrics: MenuMetrics,
    display: RefCell<Option<SharedMacDisplay>>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = SharedMacIvars]
    pub struct SharedMacView;

    impl SharedMacView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render();
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }

    unsafe impl NSObjectProtocol for SharedMacView {}
);

impl SharedMacView {
    pub fn new(mtm: MainThreadMarker, metrics: MenuMetrics) -> Retained<Self> {
        let frame = NSRect::new(
            NSPoint::ZERO,
            NSSize::new(metrics.canvas_width, VIEW_HEIGHT),
        );
        let this = Self::alloc(mtm).set_ivars(SharedMacIvars {
            metrics,
            display: RefCell::new(None),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let role = NSString::from_str("AXGroup");
        unsafe {
            let _: () = msg_send![&*view, setAccessibilityElement: true];
            let _: () = msg_send![&*view, setAccessibilityRole: &*role];
        }
        view
    }

    /// Show `display`, redrawing only when it changed.
    pub fn update(&self, display: &SharedMacDisplay) {
        if self.ivars().display.borrow().as_ref() == Some(display) {
            return;
        }
        let label = NSString::from_str(&display.title);
        let value = NSString::from_str(&format!(
            "{}, pressure {}, swap {}, CPU {}, GPU {}",
            display.figures, display.pressure, display.swap, display.cpu, display.gpu
        ));
        unsafe {
            let _: () = msg_send![self, setAccessibilityLabel: &*label];
            let _: () = msg_send![self, setAccessibilityValue: &*value];
        }
        *self.ivars().display.borrow_mut() = Some(display.clone());
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let display = self.ivars().display.borrow();
        let Some(display) = display.as_ref() else {
            return;
        };
        let metrics = self.ivars().metrics;
        let (left, right) = (metrics.content_left(), metrics.content_right());
        let size = metrics.type_scale.stat_row;
        let text = color_for_accent_alpha(display.accent, 1.0);

        let title_font = NSFont::systemFontOfSize_weight(size, unsafe { NSFontWeightSemibold });
        let figure_font =
            NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe { NSFontWeightRegular });
        let header_figures_font = header_figure_font(
            &display.figures,
            size,
            right - left - text_width(&display.title, &title_font) - HEADER_GAP,
        );
        draw_text(&display.title, left, HEADER_Y, &title_font, &text);
        draw_text_right(
            &display.figures,
            right,
            HEADER_Y + (size - header_figures_font.pointSize()) / 2.0,
            &header_figures_font,
            &NSColor::secondaryLabelColor(),
        );

        let columns = MAP_CELLS / STRIP_ROWS;
        let strip_height = STRIP_ROWS as f64 * CELL_HEIGHT
            + CELL_GAP * (STRIP_ROWS - 1) as f64
            + STRIP_PADDING * 2.0;
        let board = NSRect::new(
            NSPoint::new(left + 0.5, STRIP_TOP + 0.5),
            NSSize::new(right - left - 1.0, strip_height - 1.0),
        );
        let board_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(board, 3.0, 3.0);
        NSColor::labelColor()
            .colorWithAlphaComponent(0.04)
            .setFill();
        board_path.fill();
        NSColor::labelColor()
            .colorWithAlphaComponent(0.12)
            .setStroke();
        board_path.stroke();
        let cell_width =
            (right - left - STRIP_PADDING * 2.0 - CELL_GAP * (columns - 1) as f64) / columns as f64;
        for (index, category) in display.cells.iter().enumerate() {
            let column = (index / STRIP_ROWS) as f64;
            let row = (index % STRIP_ROWS) as f64;
            let cell = NSRect::new(
                NSPoint::new(
                    left + STRIP_PADDING + column * (cell_width + CELL_GAP),
                    STRIP_TOP + STRIP_PADDING + row * (CELL_HEIGHT + CELL_GAP),
                ),
                NSSize::new(cell_width, CELL_HEIGHT),
            );
            fill_swatch(cell, category.swatch(), display.accent, 1.5);
        }

        let value_font =
            NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe { NSFontWeightSemibold });
        let middle = (left + right) / 2.0;
        let text_inset = (PAIR_HEIGHT - 16.0) / 2.0;
        for (row, [(first, first_value), (second, second_value)]) in [
            [("Pressure", &display.pressure), ("Swap", &display.swap)],
            [("CPU", &display.cpu), ("GPU", &display.gpu)],
        ]
        .into_iter()
        .enumerate()
        {
            let y = PAIR_TOP + row as f64 * PAIR_HEIGHT + text_inset;
            draw_text(first, left, y, &figure_font, &text);
            draw_text_right(first_value, middle - PAIR_GAP / 2.0, y, &value_font, &text);
            draw_text(second, middle + PAIR_GAP / 2.0, y, &figure_font, &text);
            draw_text_right(second_value, right, y, &value_font, &text);
        }
    }
}
