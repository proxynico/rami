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
pub(super) const ROW_TAIL_TAB: f64 = 200.0;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccentPaint {
    Label,
    AlertRed,
}

// Ring stroke palette — resolved by `color_for_rings` and wired into
// `MemoryRingsView::update` from `tray/mod.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RingStroke {
    CalmOrange,
    AlertRed,
}

pub(super) fn accent_paint(accent: Accent) -> AccentPaint {
    match accent {
        Accent::Neutral => AccentPaint::Label,
        Accent::Warning | Accent::Critical => AccentPaint::AlertRed,
    }
}

pub(super) fn ring_stroke_for_accent(accent: Accent) -> RingStroke {
    match accent {
        Accent::Neutral => RingStroke::CalmOrange,
        Accent::Warning | Accent::Critical => RingStroke::AlertRed,
    }
}

pub(super) fn color_for_accent(accent: Accent) -> Retained<NSColor> {
    match accent_paint(accent) {
        AccentPaint::Label => NSColor::labelColor(),
        AccentPaint::AlertRed => NSColor::systemRedColor(),
    }
}

pub(super) fn color_for_rings(accent: Accent) -> Retained<NSColor> {
    match ring_stroke_for_accent(accent) {
        RingStroke::CalmOrange => NSColor::systemOrangeColor(),
        RingStroke::AlertRed => NSColor::systemRedColor(),
    }
}

/// Accent color at `alpha`, still adaptive across light/dark.
///
/// `colorWithAlphaComponent` on catalog colors (especially `labelColor`)
/// resolves against the *creation* appearance and freezes that RGB — so a
/// title built in dark mode stays white after switching to light. Full
/// opacity returns the catalog color as-is; partial opacity maps Neutral onto
/// adaptive secondary/tertiary labels and only alpha-ramps Warning/Critical.
pub(super) fn color_for_accent_alpha(accent: Accent, alpha: f64) -> Retained<NSColor> {
    match accent {
        Accent::Neutral => {
            if alpha >= 0.99 {
                NSColor::labelColor()
            } else if alpha >= 0.45 {
                NSColor::secondaryLabelColor()
            } else {
                NSColor::tertiaryLabelColor()
            }
        }
        Accent::Warning | Accent::Critical => {
            let base = color_for_accent(accent);
            if alpha >= 0.99 {
                base
            } else {
                // Semantic oranges/reds stay visible if baked; Neutral is the
                // white-on-light failure mode this helper exists to prevent.
                base.colorWithAlphaComponent(alpha)
            }
        }
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
    use super::{
        accent_paint, ring_stroke_for_accent, status_tint_for_pressure, AccentPaint, RingStroke,
    };
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

    #[test]
    fn ring_stroke_is_calm_orange_under_neutral() {
        assert_eq!(
            ring_stroke_for_accent(crate::format::Accent::Neutral),
            RingStroke::CalmOrange
        );
    }

    #[test]
    fn ring_stroke_is_alert_red_under_warning_and_critical() {
        assert_eq!(
            ring_stroke_for_accent(crate::format::Accent::Warning),
            RingStroke::AlertRed
        );
        assert_eq!(
            ring_stroke_for_accent(crate::format::Accent::Critical),
            RingStroke::AlertRed
        );
    }

    #[test]
    fn warning_and_critical_share_the_alert_red_accent_path() {
        assert_eq!(
            accent_paint(crate::format::Accent::Warning),
            AccentPaint::AlertRed
        );
        assert_eq!(
            accent_paint(crate::format::Accent::Critical),
            AccentPaint::AlertRed
        );
    }

    #[test]
    fn accent_paint_warning_matches_critical_alert_red() {
        assert_eq!(
            accent_paint(crate::format::Accent::Warning),
            AccentPaint::AlertRed
        );
        assert_eq!(
            accent_paint(crate::format::Accent::Critical),
            AccentPaint::AlertRed
        );
        assert_eq!(
            accent_paint(crate::format::Accent::Neutral),
            AccentPaint::Label
        );
    }
}
