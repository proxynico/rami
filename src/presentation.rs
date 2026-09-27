use crate::format::Accent;
use objc2::rc::Retained;
use objc2_app_kit::{NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSColor};
use objc2_foundation::{NSArray, NSSize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MenuTypeScale {
    pub(crate) map_header: f64,
    pub(crate) caption: f64,
    pub(crate) module_title: f64,
    pub(crate) stat_row: f64,
}

/// Top-down geometry of the memory map view: header, a 16×4 cell grid on a
/// board, a contact strip, and a caption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MapLayout {
    pub(crate) view_width: f64,
    pub(crate) view_height: f64,
    pub(crate) left: f64,
    pub(crate) right: f64,
    pub(crate) header_y: f64,
    pub(crate) board_top: f64,
    pub(crate) board_padding: f64,
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    pub(crate) cell_height: f64,
    pub(crate) cell_gap: f64,
    pub(crate) strip_height: f64,
    pub(crate) caption_y: f64,
}

/// One row: "Pressure", a meter with Warning and Critical ticks, the value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MeterLayout {
    pub(crate) view_width: f64,
    pub(crate) view_height: f64,
    pub(crate) label_x: f64,
    pub(crate) meter_left: f64,
    pub(crate) meter_right: f64,
    pub(crate) meter_height: f64,
    pub(crate) value_right: f64,
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
    /// Menu rows end their text here short of the canvas edge, leaving room
    /// NSMenu keeps for submenu arrows.
    pub(crate) trailing_inset: f64,
    pub(crate) icon_slot: f64,
    pub(crate) type_scale: MenuTypeScale,
    history: HistoryMetrics,
    title_height: f64,
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
        trailing_inset: 24.0,
        icon_slot: 16.0,
        type_scale: MenuTypeScale {
            map_header: 12.0,
            caption: 10.0,
            module_title: 13.0,
            stat_row: 13.0,
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
        self.canvas_width - self.trailing_inset
    }

    pub(crate) fn value_column_x(&self) -> f64 {
        self.content_right()
    }

    pub(crate) fn row_label_origin_x(&self) -> f64 {
        self.content_left()
    }

    pub(crate) fn row_tail_tab(&self) -> f64 {
        self.value_column_x() - self.row_label_origin_x()
    }

    pub(crate) fn map_layout(&self) -> MapLayout {
        MapLayout {
            view_width: self.canvas_width,
            view_height: 108.0,
            left: self.content_left(),
            right: self.content_right(),
            header_y: 6.0,
            board_top: 28.0,
            board_padding: 4.0,
            columns: 16,
            rows: 4,
            cell_height: 10.0,
            cell_gap: 2.0,
            strip_height: 4.0,
            caption_y: 91.0,
        }
    }

    pub(crate) fn meter_layout(&self) -> MeterLayout {
        MeterLayout {
            view_width: self.canvas_width,
            view_height: 22.0,
            label_x: self.content_left(),
            meter_left: 86.0,
            meter_right: self.content_right() - 44.0,
            meter_height: 4.0,
            value_right: self.content_right(),
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
            caption_size: self.type_scale.caption,
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

impl MapLayout {
    pub(crate) fn view_size(&self) -> NSSize {
        NSSize::new(self.view_width, self.view_height)
    }

    pub(crate) fn board_height(&self) -> f64 {
        let rows = self.rows as f64;
        rows * self.cell_height + (rows - 1.0) * self.cell_gap + self.board_padding * 2.0
    }

    pub(crate) fn cell_width(&self) -> f64 {
        let columns = self.columns as f64;
        let grid = self.right - self.left - self.board_padding * 2.0;
        (grid - (columns - 1.0) * self.cell_gap) / columns
    }

    pub(crate) fn board_bottom(&self) -> f64 {
        self.board_top + self.board_height()
    }
}

impl MeterLayout {
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

fn accent_paint(accent: Accent) -> AccentPaint {
    match accent {
        Accent::Neutral => AccentPaint::Label,
        Accent::Warning | Accent::Critical => AccentPaint::AlertRed,
    }
}

pub(crate) fn color_for_accent(accent: Accent) -> Retained<NSColor> {
    match accent_paint(accent) {
        AccentPaint::Label => NSColor::labelColor(),
        AccentPaint::AlertRed => NSColor::systemRedColor(),
    }
}

/// The hue of every mark: map cells, swatches, the meter, the sparkline, and
/// the contact strip. Gold under Neutral, red under Warning and Critical.
///
/// Call it while drawing. Gold is picked for the current drawing appearance,
/// so a color resolved outside a draw keeps whichever mode was active then.
pub(crate) fn mark_color(accent: Accent) -> Retained<NSColor> {
    match accent_paint(accent) {
        AccentPaint::Label => {
            if drawing_in_dark_mode() {
                NSColor::colorWithSRGBRed_green_blue_alpha(0.957, 0.706, 0.243, 1.0)
            } else {
                NSColor::colorWithSRGBRed_green_blue_alpha(0.718, 0.467, 0.059, 1.0)
            }
        }
        AccentPaint::AlertRed => NSColor::systemRedColor(),
    }
}

fn drawing_in_dark_mode() -> bool {
    let appearance = NSAppearance::currentDrawingAppearance();
    let names = unsafe { NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) };
    appearance
        .bestMatchFromAppearancesWithNames(&names)
        .is_some_and(|name| unsafe { &*name == NSAppearanceNameDarkAqua })
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

#[cfg(test)]
mod tests {
    use super::{accent_paint, AccentPaint, MenuMetrics};

    #[test]
    fn standard_metrics_line_up_the_dropdown() {
        let m = MenuMetrics::STANDARD;
        assert_eq!(m.canvas_width, 240.0);
        assert_eq!(m.content_left(), 16.0);
        assert_eq!(m.content_right(), 216.0);
        assert_eq!(m.value_column_x(), 216.0);
        assert_eq!(m.row_label_origin_x(), 16.0);
        assert_eq!(m.row_tail_tab(), 200.0);

        let hist = m.history_layout();
        assert_eq!(hist.band_left, m.content_left());
        assert_eq!(hist.band_right, m.value_column_x());
        assert_eq!(hist.view_height, 36.0);

        let map = m.map_layout();
        let grid = map.cell_width() * 16.0 + map.cell_gap * 15.0 + map.board_padding * 2.0;
        assert_eq!(grid, m.content_right() - m.content_left());
        assert!(map.board_bottom() + map.strip_height <= map.caption_y);
        assert!(map.caption_y + 13.0 <= map.view_height);

        let title = m.title_layout();
        assert_eq!(title.origin_x, m.row_label_origin_x());
        assert_eq!(title.view_height, 24.0);
        assert_eq!(title.font_size, 13.0);
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
