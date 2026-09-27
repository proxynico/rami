use crate::format::Accent;
use crate::model::MemoryPressure;
use crate::presentation::MenuMetrics;
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_app_kit::{
    NSFont, NSFontWeightRegular, NSMutableParagraphStyle, NSTextAlignment, NSTextTab,
};
use objc2_foundation::{NSArray, NSDictionary};

pub(super) use crate::presentation::{color_for_accent, color_for_accent_alpha};

pub(super) const APP_ROW_POOL: usize = 3;
/// App Memory, Wired, Compressed, Other, Cached, Free.
pub(super) const BREAKDOWN_ROW_POOL: usize = 6;
pub(super) const ROW_ICON_SIZE: f64 = MenuMetrics::STANDARD.icon_slot;

/// Label alpha for demoted rows (#23): derived breakdowns render at this
/// step of the opacity ramp so brightness tracks actionability. Matches the
/// dark-mode weight of `secondaryLabelColor` while keeping the Accent hue in
/// Warning and Critical states.
pub(super) const DEMOTED_LABEL_ALPHA: f64 = 0.55;

pub(super) fn status_tint_for_pressure(pressure: MemoryPressure) -> Option<Accent> {
    match pressure {
        MemoryPressure::Normal => None,
        MemoryPressure::Warning => Some(Accent::Warning),
        MemoryPressure::Critical => Some(Accent::Critical),
    }
}

pub(super) fn row_paragraph_style(tail_tab: f64) -> Retained<NSMutableParagraphStyle> {
    let style = NSMutableParagraphStyle::new();
    let tail_tab = unsafe {
        NSTextTab::initWithTextAlignment_location_options(
            NSTextTab::alloc(),
            NSTextAlignment::Right,
            tail_tab,
            &NSDictionary::new(),
        )
    };
    let tabs = NSArray::from_retained_slice(&[tail_tab]);
    style.setTabStops(Some(&tabs));
    style
}

pub(super) fn stat_font(size: f64) -> Retained<NSFont> {
    let weight = unsafe { NSFontWeightRegular };
    NSFont::monospacedDigitSystemFontOfSize_weight(size, weight)
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
