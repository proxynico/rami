use crate::format::Accent;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSColor, NSView,
};
use objc2_foundation::{NSArray, NSSize, NSString};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MenuTypeScale {
    pub(crate) map_title: f64,
    pub(crate) map_header: f64,
    pub(crate) caption: f64,
    pub(crate) stat_row: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MapLayout {
    pub(crate) view_width: f64,
    pub(crate) view_height: f64,
    pub(crate) left: f64,
    pub(crate) right: f64,
    pub(crate) header_y: f64,
    pub(crate) board_top: f64,
    pub(crate) board_padding: f64,
    pub(crate) board_radius: f64,
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    pub(crate) cell_height: f64,
    pub(crate) cell_gap: f64,
    pub(crate) cell_radius: f64,
    pub(crate) strip_height: f64,
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
pub(crate) struct MenuMetrics {
    pub(crate) canvas_width: f64,
    pub(crate) content_inset: f64,
    pub(crate) row_height: f64,
    pub(crate) type_scale: MenuTypeScale,
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

impl MenuMetrics {
    pub(crate) const STANDARD: Self = Self {
        canvas_width: 240.0,
        content_inset: 16.0,
        row_height: 24.0,
        type_scale: MenuTypeScale {
            map_title: 15.0,
            map_header: 13.0,
            caption: 10.0,
            stat_row: 13.0,
        },
    };

    pub(crate) fn content_left(&self) -> f64 {
        self.content_inset
    }

    pub(crate) fn content_right(&self) -> f64 {
        self.canvas_width - self.content_inset
    }

    pub(crate) fn map_layout(&self) -> MapLayout {
        MapLayout {
            view_width: self.canvas_width,
            view_height: 94.0,
            left: self.content_left(),
            right: self.content_right(),
            header_y: 6.0,
            board_top: 30.0,
            board_padding: 4.0,
            board_radius: 4.0,
            columns: 16,
            rows: 4,
            cell_height: 10.0,
            cell_gap: 2.0,
            cell_radius: 2.0,
            strip_height: 4.0,
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
            view_height: 36.0,
            band_left: self.content_left(),
            band_right: self.content_right(),
            band_bottom: 14.0,
            band_top: 32.0,
            caption_y: 1.0,
            caption_size: self.type_scale.caption,
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

/// Warning and Critical share one alert red; severity between them is
/// numeric, not a second hue.
pub(crate) fn color_for_accent(accent: Accent) -> Retained<NSColor> {
    match accent {
        Accent::Neutral => NSColor::labelColor(),
        Accent::Warning | Accent::Critical => NSColor::systemRedColor(),
    }
}

/// The hue of every mark: map cells, swatches, the meter, the sparkline, and
/// the contact strip. Orange under Neutral, red under Warning and Critical.
///
/// Call it while drawing. Orange is picked for the current drawing appearance,
/// so a color resolved outside a draw keeps whichever mode was active then.
pub(crate) fn mark_color(accent: Accent) -> Retained<NSColor> {
    match accent {
        Accent::Neutral => {
            if drawing_in_dark_mode() {
                NSColor::colorWithSRGBRed_green_blue_alpha(0.961, 0.553, 0.239, 1.0)
            } else {
                NSColor::colorWithSRGBRed_green_blue_alpha(0.702, 0.337, 0.059, 1.0)
            }
        }
        Accent::Warning | Accent::Critical => NSColor::systemRedColor(),
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
            } else {
                NSColor::secondaryLabelColor()
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

pub(crate) fn set_accessibility_role(view: &NSView, role: &str) {
    let role = NSString::from_str(role);
    unsafe {
        let _: () = msg_send![view, setAccessibilityElement: true];
        let _: () = msg_send![view, setAccessibilityRole: &*role];
    }
}

pub(crate) fn set_accessibility_label(view: &NSView, label: &str) {
    let label = NSString::from_str(label);
    unsafe {
        let _: () = msg_send![view, setAccessibilityLabel: &*label];
    }
}

pub(crate) fn set_accessibility_value(view: &NSView, value: &str) {
    let value = NSString::from_str(value);
    unsafe {
        let _: () = msg_send![view, setAccessibilityValue: &*value];
    }
}

#[cfg(test)]
mod tests {
    use super::{color_for_accent_alpha, MenuMetrics};
    use crate::draw::tests::rasterize;
    use crate::format::Accent;
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSBezierPath, NSColor,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use std::cell::RefCell;

    fn appearance(name: &objc2_foundation::NSString) -> Retained<NSAppearance> {
        NSAppearance::appearanceNamed(name).expect("named appearance")
    }

    fn sample_fill(color: &NSColor, drawing_appearance: &NSAppearance) -> (f64, f64, f64, f64) {
        rasterize(4, 4, Some(drawing_appearance), || {
            color.setFill();
            NSBezierPath::fillRect(NSRect::new(NSPoint::ZERO, NSSize::new(4.0, 4.0)));
        })(1, 1)
    }

    #[test]
    fn accent_alpha_created_in_dark_mode_stays_readable_in_light_menus() {
        // colorWithAlphaComponent on labelColor freezes the creation appearance
        // (white from dark stays white in light). color_for_accent_alpha must
        // keep Neutral adaptive so row text stays readable.

        let light = appearance(unsafe { NSAppearanceNameAqua });
        let dark = appearance(unsafe { NSAppearanceNameDarkAqua });

        let built = RefCell::new(None);
        {
            let build = RcBlock::new(|| {
                *built.borrow_mut() = Some(color_for_accent_alpha(Accent::Neutral, 1.0));
            });
            dark.performAsCurrentDrawingAppearance(&build);
        }
        let color = built.into_inner().expect("color");

        let (r, g, b, _) = sample_fill(&color, &light);
        assert!(
            r < 0.5 && g < 0.5 && b < 0.5,
            "Neutral accent built in dark must draw dark-on-light, got rgba({r:.2},{g:.2},{b:.2})"
        );

        let demoted = RefCell::new(None);
        {
            let build = RcBlock::new(|| {
                *demoted.borrow_mut() = Some(color_for_accent_alpha(Accent::Neutral, 0.55));
            });
            dark.performAsCurrentDrawingAppearance(&build);
        }
        let demoted = demoted.into_inner().expect("demoted color");
        let (r, g, b, a) = sample_fill(&demoted, &light);
        assert!(
            a > 0.2 && r < 0.6 && g < 0.6 && b < 0.6,
            "demoted Neutral built in dark must stay dark-on-light, got rgba({r:.2},{g:.2},{b:.2},{a:.2})"
        );
    }

    #[test]
    fn standard_metrics_line_up_the_dropdown() {
        let m = MenuMetrics::STANDARD;
        assert_eq!(m.canvas_width, 240.0);
        assert_eq!(m.content_left(), 16.0);
        assert_eq!(m.content_right(), 224.0, "right inset matches the left");

        let hist = m.history_layout();
        assert_eq!(hist.band_left, m.content_left());
        assert_eq!(hist.band_right, m.content_right());
        assert_eq!(hist.view_height, 36.0);

        let map = m.map_layout();
        let grid = map.cell_width() * 16.0 + map.cell_gap * 15.0 + map.board_padding * 2.0;
        assert_eq!(grid, m.content_right() - m.content_left());
        assert!(map.board_bottom() + map.strip_height <= map.view_height);
    }
}
