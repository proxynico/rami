use crate::presentation::MenuMetrics;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSFontWeightMedium, NSForegroundColorAttributeName,
    NSStringDrawing, NSView,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSString};

pub struct ModuleTitleIvars {
    metrics: MenuMetrics,
    label: String,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ModuleTitleIvars]
    pub struct ModuleTitleView;

    impl ModuleTitleView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render();
        }
    }

    unsafe impl NSObjectProtocol for ModuleTitleView {}
);

impl ModuleTitleView {
    pub fn new(mtm: MainThreadMarker, metrics: MenuMetrics, label: &str) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, metrics.title_layout().view_size());
        let this = Self::alloc(mtm).set_ivars(ModuleTitleIvars {
            metrics,
            label: label.to_string(),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let accessibility_label = NSString::from_str(label);
        let accessibility_role = NSString::from_str("AXHeading");
        unsafe {
            let _: () = msg_send![&*view, setAccessibilityElement: true];
            let _: () = msg_send![&*view, setAccessibilityLabel: &*accessibility_label];
            let _: () = msg_send![&*view, setAccessibilityRole: &*accessibility_role];
        }
        view
    }

    fn render(&self) {
        let layout = self.ivars().metrics.title_layout();
        let label = NSString::from_str(&self.ivars().label);
        let font = NSFont::systemFontOfSize_weight(layout.font_size, unsafe { NSFontWeightMedium });
        let attrs = unsafe {
            let color = Retained::cast_unchecked::<AnyObject>(NSColor::labelColor());
            let font = Retained::cast_unchecked::<AnyObject>(font);
            NSDictionary::from_retained_objects(
                &[NSForegroundColorAttributeName, NSFontAttributeName],
                &[color, font],
            )
        };
        let measured = unsafe { label.sizeWithAttributes(Some(&attrs)) };
        let bounds = self.bounds();
        let origin = NSPoint::new(
            layout.origin_x,
            (bounds.size.height - measured.height) / 2.0,
        );
        unsafe {
            label.drawAtPoint_withAttributes(origin, Some(&attrs));
        }
    }
}
