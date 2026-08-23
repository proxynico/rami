use crate::format::Accent;
use objc2::rc::Retained;
use objc2_app_kit::NSColor;
use objc2_foundation::{NSPoint, NSSize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MenuTypeScale {
    pub(crate) ring_percent: f64,
    pub(crate) ring_label: f64,
    pub(crate) ring_detail: f64,
    pub(crate) history_caption: f64,
    pub(crate) module_title: f64,
    pub(crate) stat_row: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RingMetrics {
    radius: f64,
    stroke_width: f64,
    center_y: f64,
    percent_baseline_offset: f64,
    label_y: f64,
    detail_y: f64,
    view_height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HistoryMetrics {
    view_height: f64,
    band_top_inset: f64,
    band_bottom: f64,
    caption_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MenuMetrics {
    pub(crate) canvas_width: f64,
    pub(crate) content_inset: f64,
    pub(crate) icon_slot: f64,
    pub(crate) row_text_gap: f64,
    pub(crate) type_scale: MenuTypeScale,
    ring: RingMetrics,
    history: HistoryMetrics,
    title_height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RingLayout {
    pub(crate) centers: [NSPoint; 2],
    pub(crate) radius: f64,
    pub(crate) stroke_width: f64,
    pub(crate) center_y: f64,
    pub(crate) percent_baseline_offset: f64,
    pub(crate) label_y: f64,
    pub(crate) detail_y: f64,
    pub(crate) view_width: f64,
    pub(crate) view_height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HistoryLayout {
    pub(crate) view_width: f64,
    pub(crate) view_height: f64,
    pub(crate) band_left: f64,
    pub(crate) band_right: f64,
    pub(crate) band_bottom: f64,
    pub(crate) band_top: f64,
    pub(crate) caption_y: f64,
    pub(crate) caption_size: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TitleLayout {
    pub(crate) view_width: f64,
    pub(crate) view_height: f64,
    pub(crate) font_size: f64,
    pub(crate) origin_x: f64,
}

impl MenuMetrics {
    pub(crate) const STANDARD: Self = Self {
        canvas_width: 240.0,
        content_inset: 16.0,
        icon_slot: 16.0,
        row_text_gap: 8.0,
        type_scale: MenuTypeScale {
            ring_percent: 15.0,
            ring_label: 12.0,
            ring_detail: 10.0,
            history_caption: 10.0,
            module_title: 13.0,
            stat_row: 13.0,
        },
        ring: RingMetrics {
            radius: 30.0,
            stroke_width: 6.0,
            center_y: 72.0,
            percent_baseline_offset: 7.0,
            label_y: 25.0,
            detail_y: 8.0,
            view_height: 118.0,
        },
        history: HistoryMetrics {
            view_height: 36.0,
            band_top_inset: 4.0,
            band_bottom: 14.0,
            caption_y: 1.0,
        },
        title_height: 24.0,
    };

    pub(crate) fn content_left(&self) -> f64 {
        self.content_inset
    }

    pub(crate) fn content_right(&self) -> f64 {
        self.canvas_width - self.content_inset
    }

    pub(crate) fn content_width(&self) -> f64 {
        self.content_right() - self.content_left()
    }

    pub(crate) fn value_column_x(&self) -> f64 {
        self.content_right()
    }

    pub(crate) fn row_label_origin_x(&self) -> f64 {
        self.icon_slot + self.row_text_gap
    }

    pub(crate) fn row_tail_tab(&self) -> f64 {
        self.value_column_x() - self.row_label_origin_x()
    }

    pub(crate) fn ring_layout(&self) -> RingLayout {
        let left = self.content_left();
        let column = self.content_width() / 2.0;
        RingLayout {
            centers: [
                NSPoint::new(left + column * 0.5, self.ring.center_y),
                NSPoint::new(left + column * 1.5, self.ring.center_y),
            ],
            radius: self.ring.radius,
            stroke_width: self.ring.stroke_width,
            center_y: self.ring.center_y,
            percent_baseline_offset: self.ring.percent_baseline_offset,
            label_y: self.ring.label_y,
            detail_y: self.ring.detail_y,
            view_width: self.canvas_width,
            view_height: self.ring.view_height,
        }
    }

    pub(crate) fn history_layout(&self) -> HistoryLayout {
        HistoryLayout {
            view_width: self.canvas_width,
            view_height: self.history.view_height,
            band_left: self.content_left(),
            band_right: self.content_right(),
            band_bottom: self.history.band_bottom,
            band_top: self.history.view_height - self.history.band_top_inset,
            caption_y: self.history.caption_y,
            caption_size: self.type_scale.history_caption,
        }
    }

    pub(crate) fn title_layout(&self) -> TitleLayout {
        TitleLayout {
            view_width: self.canvas_width,
            view_height: self.title_height,
            font_size: self.type_scale.module_title,
            origin_x: self.row_label_origin_x(),
        }
    }
}

impl RingLayout {
    pub(crate) fn center(&self, index: usize) -> NSPoint {
        self.centers[index]
    }

    pub(crate) fn view_size(&self) -> NSSize {
        NSSize::new(self.view_width, self.view_height)
    }
}

impl HistoryLayout {
    pub(crate) fn band_width(&self) -> f64 {
        self.band_right - self.band_left
    }

    pub(crate) fn view_size(&self) -> NSSize {
        NSSize::new(self.view_width, self.view_height)
    }
}

impl TitleLayout {
    pub(crate) fn view_size(&self) -> NSSize {
        NSSize::new(self.view_width, self.view_height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccentPaint {
    Label,
    AlertRed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RingStroke {
    CalmOrange,
    AlertRed,
}

fn accent_paint(accent: Accent) -> AccentPaint {
    match accent {
        Accent::Neutral => AccentPaint::Label,
        Accent::Warning | Accent::Critical => AccentPaint::AlertRed,
    }
}

fn ring_stroke_for_accent(accent: Accent) -> RingStroke {
    match accent {
        Accent::Neutral => RingStroke::CalmOrange,
        Accent::Warning | Accent::Critical => RingStroke::AlertRed,
    }
}

pub(crate) fn color_for_accent(accent: Accent) -> Retained<NSColor> {
    match accent_paint(accent) {
        AccentPaint::Label => NSColor::labelColor(),
        AccentPaint::AlertRed => NSColor::systemRedColor(),
    }
}

pub(crate) fn color_for_rings(accent: Accent) -> Retained<NSColor> {
    match ring_stroke_for_accent(accent) {
        RingStroke::CalmOrange => NSColor::systemOrangeColor(),
        RingStroke::AlertRed => NSColor::systemRedColor(),
    }
}

/// `colorWithAlphaComponent` on catalog colors (especially `labelColor`)
/// resolves against the *creation* appearance and freezes that RGB — so a
/// title built in dark mode stays white after switching to light.
pub(crate) fn color_for_accent_alpha(accent: Accent, alpha: f64) -> Retained<NSColor> {
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
                base.colorWithAlphaComponent(alpha)
            }
        }
    }
}

pub(crate) fn rising_fast_badge_color(accent: Accent) -> Retained<NSColor> {
    color_for_accent_alpha(accent, 0.65)
}

#[derive(Clone)]
pub(crate) struct ChromeColor(Retained<NSColor>);

#[derive(Clone)]
pub(crate) struct RingStrokeColor(Retained<NSColor>);

impl ChromeColor {
    pub(crate) fn resolve(accent: Accent) -> Self {
        Self(color_for_accent(accent))
    }

    pub(crate) fn as_nscolor(&self) -> &NSColor {
        &self.0
    }
}

impl RingStrokeColor {
    pub(crate) fn resolve(accent: Accent) -> Self {
        Self(color_for_rings(accent))
    }

    pub(crate) fn as_nscolor(&self) -> &NSColor {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{accent_paint, ring_stroke_for_accent, AccentPaint, MenuMetrics, RingStroke};

    #[test]
    fn standard_metrics_line_up_the_dropdown() {
        let m = MenuMetrics::STANDARD;
        assert_eq!(m.canvas_width, 240.0);
        assert_eq!(m.content_left(), 16.0);
        assert_eq!(m.content_right(), 224.0);
        assert_eq!(m.value_column_x(), 224.0);
        assert_eq!(m.row_label_origin_x(), 24.0);
        assert_eq!(m.row_tail_tab(), 200.0);

        let hist = m.history_layout();
        assert_eq!(hist.band_left, 16.0);
        assert_eq!(hist.band_right, 224.0);
        assert_eq!(hist.band_right, m.value_column_x());
        assert_eq!(hist.view_height, 36.0);

        let rings = m.ring_layout();
        assert_eq!(rings.stroke_width, 6.0);
        let left_gap = rings.center(0).x - m.content_left();
        let right_gap = m.content_right() - rings.center(1).x;
        assert_eq!(left_gap, right_gap);
        assert_eq!(
            rings.center(0).x,
            m.content_left() + m.content_width() / 4.0
        );
        assert_eq!(
            rings.center(1).x,
            m.content_left() + m.content_width() * 0.75
        );

        let title = m.title_layout();
        assert_eq!(title.origin_x, m.row_label_origin_x());
        assert_eq!(title.view_height, 24.0);
        assert_eq!(title.font_size, 13.0);
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
