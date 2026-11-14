//! The shared Mac's section: its title and memory figures, its 64 map cells
//! as a two-row strip, then Pressure and Swap on one row and CPU and GPU on
//! the next. It takes the shared Mac's own accent, so only this section turns
//! red when that Mac is short of memory.

use crate::draw::{centered_text_y, draw_text, draw_text_right, text_width};
use crate::format::{SharedMacDisplay, MAP_CELLS};
use crate::memory_map_view::{draw_board, header_figure_font, HEADER_GAP};
use crate::presentation::{
    color_for_accent_alpha, set_accessibility_label, set_accessibility_role,
    set_accessibility_value, MapLayout, MenuMetrics,
};
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSColor, NSFont, NSFontWeightRegular, NSFontWeightSemibold, NSView};
use objc2_foundation::{NSPoint, NSRect};
use std::cell::RefCell;

const PAIR_TOP: f64 = 48.0;
const PAIR_HEIGHT: f64 = 22.0;
const PAIR_GAP: f64 = 16.0;

fn strip_layout() -> MapLayout {
    MapLayout {
        view_height: 94.0,
        header_y: 4.0,
        board_top: 26.0,
        board_padding: 3.0,
        board_radius: 3.0,
        columns: MAP_CELLS / 2,
        rows: 2,
        cell_height: 6.0,
        cell_gap: 2.0,
        cell_radius: 1.5,
        ..MenuMetrics::STANDARD.map_layout()
    }
}

pub struct SharedMacIvars {
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
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, strip_layout().view_size());
        let this = Self::alloc(mtm).set_ivars(SharedMacIvars {
            display: RefCell::new(None),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        set_accessibility_role(&view, "AXGroup");
        view
    }

    /// Show `display`, redrawing only when it changed.
    pub fn update(&self, display: &SharedMacDisplay) {
        if self.ivars().display.borrow().as_ref() == Some(display) {
            return;
        }
        set_accessibility_label(self, &display.title);
        set_accessibility_value(
            self,
            &format!(
                "{}, pressure {}, swap {}, CPU {}, GPU {}",
                display.figures, display.pressure, display.swap, display.cpu, display.gpu
            ),
        );
        *self.ivars().display.borrow_mut() = Some(display.clone());
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let display = self.ivars().display.borrow();
        let Some(display) = display.as_ref() else {
            return;
        };
        let layout = strip_layout();
        let (left, right) = (layout.left, layout.right);
        let size = MenuMetrics::STANDARD.type_scale.stat_row;
        let text = color_for_accent_alpha(display.accent, 1.0);

        let title_font = NSFont::systemFontOfSize_weight(size, unsafe { NSFontWeightSemibold });
        let figure_font =
            NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe { NSFontWeightRegular });
        let header_figures_font = header_figure_font(
            &display.figures,
            size,
            right - left - text_width(&display.title, &title_font) - HEADER_GAP,
        );
        draw_text(&display.title, left, layout.header_y, &title_font, &text);
        draw_text_right(
            &display.figures,
            right,
            layout.header_y + (size - header_figures_font.pointSize()) / 2.0,
            &header_figures_font,
            &NSColor::secondaryLabelColor(),
        );

        draw_board(&layout, &display.cells, display.accent);

        let value_font =
            NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe { NSFontWeightSemibold });
        let middle = (left + right) / 2.0;
        for (row, [(first, first_value), (second, second_value)]) in [
            [("Pressure", &display.pressure), ("Swap", &display.swap)],
            [("CPU", &display.cpu), ("GPU", &display.gpu)],
        ]
        .into_iter()
        .enumerate()
        {
            let y = PAIR_TOP + row as f64 * PAIR_HEIGHT + centered_text_y(PAIR_HEIGHT);
            draw_text(first, left, y, &figure_font, &text);
            draw_text_right(first_value, middle - PAIR_GAP / 2.0, y, &value_font, &text);
            draw_text(second, middle + PAIR_GAP / 2.0, y, &figure_font, &text);
            draw_text_right(second_value, right, y, &value_font, &text);
        }
    }
}
