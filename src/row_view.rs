//! One dropdown data row: an optional swatch, a label, secondary detail, and
//! an emphasized value, drawn edge to edge inside the content insets. Data
//! rows are views rather than titled menu items because NSMenu reserves a
//! submenu-arrow column on the right of every titled item once any item has
//! a submenu, which left the rows short of the custom views' right edge.

use crate::draw::{draw_text, draw_text_right, fill_swatch, text_width};
use crate::format::{Accent, LegendRow, StatRow, Swatch};
use crate::presentation::{color_for_accent_alpha, MenuMetrics};
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSColor, NSFont, NSFontWeightRegular, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;

const SWATCH_SIZE: f64 = 9.0;
const SWATCH_GAP: f64 = 7.0;
const VALUE_GAP: f64 = 6.0;
/// Demoted rows sit at this step of the opacity ramp so brightness tracks
/// actionability (#23).
const DEMOTED_ALPHA: f64 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowStrength {
    Primary,
    Demoted,
    Muted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowContent {
    pub(crate) swatch: Option<Swatch>,
    pub(crate) label: String,
    pub(crate) strength: RowStrength,
    pub(crate) detail: Option<String>,
    pub(crate) value: Option<String>,
}

impl RowContent {
    pub(crate) fn legend(row: &LegendRow) -> Self {
        Self {
            swatch: Some(row.swatch),
            label: row.label.clone(),
            strength: if row.primary {
                RowStrength::Primary
            } else {
                RowStrength::Demoted
            },
            detail: Some(row.value.clone()),
            value: None,
        }
    }

    /// A stat row's tail splits on a tab: secondary detail, then an
    /// emphasized value (an app's growth, CPU busy %, GPU utilization).
    pub(crate) fn stat(row: &StatRow) -> Self {
        let (detail, value) = match row.tail.as_deref().map(|tail| tail.split_once('\t')) {
            Some(Some((detail, value))) => (Some(detail.to_string()), Some(value.to_string())),
            Some(None) => (row.tail.clone(), None),
            None => (None, None),
        };
        Self {
            swatch: None,
            label: row.primary.clone(),
            strength: RowStrength::Primary,
            detail,
            value,
        }
    }

    pub(crate) fn muted(label: &str) -> Self {
        Self {
            swatch: None,
            label: label.to_string(),
            strength: RowStrength::Muted,
            detail: None,
            value: None,
        }
    }
}

struct RowState {
    content: Option<RowContent>,
    accent: Accent,
}

pub struct RowIvars {
    metrics: MenuMetrics,
    state: RefCell<RowState>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = RowIvars]
    pub struct RowView;

    impl RowView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            self.render();
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }

    unsafe impl NSObjectProtocol for RowView {}
);

impl RowView {
    pub fn new(mtm: MainThreadMarker, metrics: MenuMetrics) -> Retained<Self> {
        let frame = NSRect::new(
            NSPoint::ZERO,
            NSSize::new(metrics.canvas_width, metrics.row_height),
        );
        let this = Self::alloc(mtm).set_ivars(RowIvars {
            metrics,
            state: RefCell::new(RowState {
                content: None,
                accent: Accent::Neutral,
            }),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let role = NSString::from_str("AXStaticText");
        unsafe {
            let _: () = msg_send![&*view, setAccessibilityElement: true];
            let _: () = msg_send![&*view, setAccessibilityRole: &*role];
        }
        view
    }

    pub fn update(&self, content: &RowContent, accent: Accent) {
        let spoken: Vec<&str> = [content.detail.as_deref(), content.value.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        let label = NSString::from_str(&content.label);
        let value = NSString::from_str(&spoken.join(" "));
        unsafe {
            let _: () = msg_send![self, setAccessibilityLabel: &*label];
            let _: () = msg_send![self, setAccessibilityValue: &*value];
        }
        *self.ivars().state.borrow_mut() = RowState {
            content: Some(content.clone()),
            accent,
        };
        self.setNeedsDisplay(true);
    }

    fn render(&self) {
        let state = self.ivars().state.borrow();
        let Some(content) = &state.content else {
            return;
        };
        let metrics = self.ivars().metrics;
        let font =
            NSFont::monospacedDigitSystemFontOfSize_weight(metrics.type_scale.stat_row, unsafe {
                NSFontWeightRegular
            });
        let text_y = (metrics.row_height - 16.0) / 2.0;
        let mut label_x = metrics.content_left();

        if let Some(swatch) = content.swatch {
            let swatch_rect = NSRect::new(
                NSPoint::new(label_x, (metrics.row_height - SWATCH_SIZE) / 2.0),
                NSSize::new(SWATCH_SIZE, SWATCH_SIZE),
            );
            fill_swatch(swatch_rect, swatch, state.accent, 2.0);
            label_x += SWATCH_SIZE + SWATCH_GAP;
        }

        let mut right = metrics.content_right();
        if let Some(value) = &content.value {
            draw_text_right(
                value,
                right,
                text_y,
                &font,
                &color_for_accent_alpha(state.accent, 1.0),
            );
            right -= text_width(value, &font) + VALUE_GAP;
        }
        if let Some(detail) = &content.detail {
            draw_text_right(
                detail,
                right,
                text_y,
                &font,
                &color_for_accent_alpha(state.accent, DEMOTED_ALPHA),
            );
            right -= text_width(detail, &font) + VALUE_GAP;
        }

        let label_color = match content.strength {
            RowStrength::Primary => color_for_accent_alpha(state.accent, 1.0),
            RowStrength::Demoted => color_for_accent_alpha(state.accent, DEMOTED_ALPHA),
            RowStrength::Muted => NSColor::secondaryLabelColor(),
        };
        let label = fit_text(&content.label, &font, right - label_x);
        draw_text(&label, label_x, text_y, &font, &label_color);
    }
}

/// The text, cut with an ellipsis if it would run past `max_width`.
fn fit_text(text: &str, font: &NSFont, max_width: f64) -> String {
    if text_width(text, font) <= max_width {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>() + "…";
        if text_width(&candidate, font) <= max_width {
            return candidate;
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_tails_split_into_detail_and_emphasized_value() {
        let row = |tail: Option<&str>| StatRow {
            primary: "CPU".to_string(),
            tail: tail.map(str::to_string),
        };
        let cpu = RowContent::stat(&row(Some("12 usr · 6 sys\t18%")));
        assert_eq!(cpu.detail.as_deref(), Some("12 usr · 6 sys"));
        assert_eq!(cpu.value.as_deref(), Some("18%"));

        let swap = RowContent::stat(&row(Some("4.7 GB")));
        assert_eq!(swap.detail.as_deref(), Some("4.7 GB"));
        assert_eq!(swap.value, None);
    }

    #[test]
    fn long_labels_are_cut_to_leave_room_for_the_values() {
        let font =
            NSFont::monospacedDigitSystemFontOfSize_weight(13.0, unsafe { NSFontWeightRegular });
        let label = "Codex Computer U…";
        let fitted = fit_text(label, &font, 80.0);
        assert!(fitted.ends_with('…'));
        assert!(text_width(&fitted, &font) <= 80.0);
        assert_eq!(fit_text("Zen", &font, 80.0), "Zen");
    }
}
