use crate::format::Accent;
use crate::model::MemoryPressure;
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_app_kit::{
    NSColor, NSFont, NSFontWeightRegular, NSMutableParagraphStyle, NSTextAlignment, NSTextTab,
};
use objc2_foundation::{NSArray, NSDictionary};

pub(super) const APP_ROW_POOL: usize = 3;
pub(super) const ROW_ICON_SIZE: f64 = 16.0;
pub(super) const ROW_TAIL_TAB: f64 = 180.0;

/// Label alpha for demoted rows (#23): derived breakdowns render at this
/// step of the opacity ramp so brightness tracks actionability. Matches the
/// dark-mode weight of `secondaryLabelColor` while keeping the Accent hue in
/// Warning and Critical states.
pub(super) const DEMOTED_LABEL_ALPHA: f64 = 0.55;

pub(super) fn color_for_accent(accent: Accent) -> Retained<NSColor> {
    match accent {
        Accent::Neutral => NSColor::labelColor(),
        Accent::Warning => NSColor::systemOrangeColor(),
        Accent::Critical => NSColor::systemRedColor(),
    }
}

pub(super) fn status_tint_for_pressure(pressure: MemoryPressure) -> Option<Accent> {
    match pressure {
        MemoryPressure::Normal => None,
        MemoryPressure::Warning => Some(Accent::Warning),
        MemoryPressure::Critical => Some(Accent::Critical),
    }
}

pub(super) fn row_paragraph_style() -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    let tail_tab = unsafe {
        NSTextTab::initWithTextAlignment_location_options(
            NSTextTab::alloc(),
            NSTextAlignment::Right,
            ROW_TAIL_TAB,
            &NSDictionary::new(),
        )
    };
    let tabs = NSArray::from_retained_slice(&[tail_tab]);
    style.setTabStops(Some(&tabs));
    style
}

pub(super) fn stat_font() -> Retained<NSFont> {
    let weight = unsafe { NSFontWeightRegular };
    NSFont::monospacedDigitSystemFontOfSize_weight(13.0, weight)
}

#[cfg(test)]
mod tests {
    use super::status_tint_for_pressure;
    use crate::model::MemoryPressure;

    #[test]
    fn normal_pressure_leaves_the_template_icon_system_adaptive() {
        assert_eq!(status_tint_for_pressure(MemoryPressure::Normal), None);
        assert_eq!(
            status_tint_for_pressure(MemoryPressure::Warning),
            Some(crate::format::Accent::Warning)
        );
        assert_eq!(
            status_tint_for_pressure(MemoryPressure::Critical),
            Some(crate::format::Accent::Critical)
        );
    }
}
