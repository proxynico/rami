//! The pressure row: label, a meter with ticks where Warning and Critical
//! begin, and the percent.

use crate::draw::{draw_text, draw_text_right};
use crate::format::Accent;
use crate::model::{CRITICAL_PRESSURE_PCT, WARNING_PRESSURE_PCT};
use crate::presentation::{mark_color, MenuMetrics};
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBezierPath, NSColor, NSFont, NSFontWeightRegular, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::cell::Cell;

const TICK_HEIGHT: f64 = 10.0;

pub struct PressureIvars {
    metrics: MenuMetrics,
    percent: Cell<u8>,
    accent: Cell<Accent>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PressureIvars]
    pub struct PressureView;

    impl PressureView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render();
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }

    unsafe impl NSObjectProtocol for PressureView {}
);

impl PressureView {
    pub fn new(mtm: MainThreadMarker, metrics: MenuMetrics) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, metrics.meter_layout().view_size());
        let this = Self::alloc(mtm).set_ivars(PressureIvars {
            metrics,
            percent: Cell::new(0),
            accent: Cell::new(Accent::Neutral),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let role = NSString::from_str("AXGroup");
        let label = NSString::from_str("Pressure");
        unsafe {
            let _: () = msg_send![&*view, setAccessibilityElement: true];
            let _: () = msg_send![&*view, setAccessibilityRole: &*role];
            let _: () = msg_send![&*view, setAccessibilityLabel: &*label];
        }
        view
    }

    pub fn update(&self, percent: u8, accent: Accent) {
        self.ivars().percent.set(percent);
        self.ivars().accent.set(accent);
        let value = NSString::from_str(&format!("{percent} percent"));
        unsafe {
            let _: () = msg_send![self, setAccessibilityValue: &*value];
        }
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let layout = self.ivars().metrics.meter_layout();
        let percent = self.ivars().percent.get().min(100);
        let accent = self.ivars().accent.get();
        let font = NSFont::monospacedDigitSystemFontOfSize_weight(
            self.ivars().metrics.type_scale.stat_row,
            unsafe { NSFontWeightRegular },
        );
        let text_y = (layout.view_height - 16.0) / 2.0;
        let (label_color, value_color) = match accent {
            Accent::Neutral => (NSColor::labelColor(), NSColor::secondaryLabelColor()),
            Accent::Warning | Accent::Critical => {
                (NSColor::systemRedColor(), NSColor::systemRedColor())
            }
        };
        draw_text("Pressure", layout.label_x, text_y, &font, &label_color);
        draw_text_right(
            &format!("{percent}%"),
            layout.value_right,
            text_y,
            &font,
            &value_color,
        );

        let width = layout.meter_right - layout.meter_left;
        let top = (layout.view_height - layout.meter_height) / 2.0;
        let radius = layout.meter_height / 2.0;
        let track = NSRect::new(
            NSPoint::new(layout.meter_left, top),
            NSSize::new(width, layout.meter_height),
        );
        NSColor::quaternaryLabelColor().setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(track, radius, radius).fill();
        if percent > 0 {
            let fill = NSRect::new(
                NSPoint::new(layout.meter_left, top),
                NSSize::new(width * f64::from(percent) / 100.0, layout.meter_height),
            );
            mark_color(accent).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(fill, radius, radius).fill();
        }

        NSColor::tertiaryLabelColor().setFill();
        for threshold in [WARNING_PRESSURE_PCT, CRITICAL_PRESSURE_PCT] {
            let x = layout.meter_left + width * f64::from(threshold) / 100.0;
            NSBezierPath::fillRect(NSRect::new(
                NSPoint::new(x - 0.5, (layout.view_height - TICK_HEIGHT) / 2.0),
                NSSize::new(1.0, TICK_HEIGHT),
            ));
        }
    }
}
