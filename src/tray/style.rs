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
pub(super) const ROW_ICON_SIZE: f64 = MenuMetrics::STANDARD.icon_slot;

/// Label alpha for demoted rows (#23): derived breakdowns render at this
/// step of the opacity ramp so brightness tracks actionability. Matches the
/// dark-mode weight of `secondaryLabelColor` while keeping the Accent hue in
/// Warning and Critical states.
pub(super) const DEMOTED_LABEL_ALPHA: f64 = 0.55;

/// Swatch opacity for demoted rows (E/P cluster splits): the same demotion
/// strength as `DEMOTED_LABEL_ALPHA`, expressed on the 0–100 swatch scale, so
/// a row's dot and its text always carry equal visual weight.
pub(super) const DEMOTED_SWATCH_OPACITY: u8 = 55;

/// Swatch opacity for informational ranking rows (CPU processes): a quiet
/// bullet marker rather than a legend entry, so it sits well below the
/// demoted step and never competes with the row's full-strength text.
pub(super) const INFO_ROW_SWATCH_OPACITY: u8 = 35;

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
