use crate::format::{Accent, RingDisplay};
use crate::presentation::{ChromeColor, MenuMetrics, RingLayout, RingStrokeColor};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontAttributeName, NSFontWeightMedium, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSLineCapStyle, NSStringDrawing, NSView,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;

#[derive(Clone)]
struct RingsState {
    rings: Option<[RingDisplay; 2]>,
    stroke: RingStrokeColor,
    chrome: ChromeColor,
}

pub struct MemoryRingsIvars {
    metrics: MenuMetrics,
    state: RefCell<RingsState>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MemoryRingsIvars]
    pub struct MemoryRingsView;

    impl MemoryRingsView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render();
        }
    }

    unsafe impl NSObjectProtocol for MemoryRingsView {}
);

impl MemoryRingsView {
    pub fn new(mtm: MainThreadMarker, metrics: MenuMetrics) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, metrics.ring_layout().view_size());
        let this = Self::alloc(mtm).set_ivars(MemoryRingsIvars {
            metrics,
            state: RefCell::new(RingsState {
                rings: None,
                stroke: RingStrokeColor::resolve(Accent::Neutral),
                chrome: ChromeColor::resolve(Accent::Neutral),
            }),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let role = NSString::from_str("AXGroup");
        unsafe {
            let _: () = msg_send![&*view, setAccessibilityElement: true];
            let _: () = msg_send![&*view, setAccessibilityRole: &*role];
        }
        view
    }

    pub fn update(&self, rings: &[RingDisplay; 2], stroke: RingStrokeColor, chrome: ChromeColor) {
        *self.ivars().state.borrow_mut() = RingsState {
            rings: Some(rings.clone()),
            stroke,
            chrome,
        };
        let label = NSString::from_str("Memory");
        let value = NSString::from_str(&format!(
            "{} percent, {}, pressure {} percent",
            rings[0].percent, rings[0].detail, rings[1].percent
        ));
        unsafe {
            let _: () = msg_send![self, setAccessibilityLabel: &*label];
            let _: () = msg_send![self, setAccessibilityValue: &*value];
        }
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let state = self.ivars().state.borrow();
        let Some(rings) = &state.rings else {
            return;
        };
        let layout = self.ivars().metrics.ring_layout();
        let scale = self.ivars().metrics.type_scale;
        for (index, ring) in rings.iter().enumerate() {
            let center = layout.center(index);
            draw_ring(center, ring.percent, layout, state.stroke.as_nscolor());
            draw_centered(
                &format!("{}%", ring.percent),
                center.x,
                center.y - layout.percent_baseline_offset,
                scale.ring_percent,
                true,
                state.chrome.as_nscolor(),
            );
            draw_centered(
                &ring.label,
                center.x,
                layout.label_y,
                scale.ring_label,
                false,
                state.chrome.as_nscolor(),
            );
            if !ring.detail.is_empty() {
                draw_centered(
                    &ring.detail,
                    center.x,
                    layout.detail_y,
                    scale.ring_detail,
                    false,
                    &NSColor::secondaryLabelColor(),
                );
            }
        }
    }
}

fn draw_ring(center: NSPoint, percent: u8, layout: RingLayout, stroke: &NSColor) {
    let bounds = NSRect::new(
        NSPoint::new(center.x - layout.radius, center.y - layout.radius),
        NSSize::new(layout.radius * 2.0, layout.radius * 2.0),
    );
    let track = NSBezierPath::bezierPathWithOvalInRect(bounds);
    track.setLineWidth(layout.stroke_width);
    NSColor::quaternaryLabelColor().setStroke();
    track.stroke();

    if percent == 0 {
        return;
    }
    let progress = NSBezierPath::bezierPath();
    progress.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
        center,
        layout.radius,
        90.0,
        90.0 - f64::from(percent.min(100)) * 3.6,
        true,
    );
    progress.setLineWidth(layout.stroke_width);
    progress.setLineCapStyle(NSLineCapStyle::Round);
    stroke.setStroke();
    progress.stroke();
}

fn draw_centered(text: &str, center_x: f64, y: f64, size: f64, emphasized: bool, color: &NSColor) {
    let weight = unsafe {
        if emphasized {
            NSFontWeightMedium
        } else {
            NSFontWeightRegular
        }
    };
    let font = if emphasized {
        NSFont::monospacedDigitSystemFontOfSize_weight(size, weight)
    } else {
        NSFont::systemFontOfSize_weight(size, weight)
    };
    let attrs = unsafe {
        let color_obj = Retained::cast_unchecked::<AnyObject>(color.retain());
        let font_obj = Retained::cast_unchecked::<AnyObject>(font);
        NSDictionary::from_retained_objects(
            &[NSForegroundColorAttributeName, NSFontAttributeName],
            &[color_obj, font_obj],
        )
    };
    let text = NSString::from_str(text);
    let measured = unsafe { text.sizeWithAttributes(Some(&attrs)) };
    unsafe {
        text.drawAtPoint_withAttributes(
            NSPoint::new(center_x - measured.width / 2.0, y),
            Some(&attrs),
        );
    }
}
