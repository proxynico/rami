use super::style::{
    color_for_accent, row_paragraph_style, stat_font, DEMOTED_LABEL_ALPHA, ROW_ICON_SIZE,
};
use crate::format::{Accent, LegendRow, StatRow};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSImage,
    NSImageSymbolConfiguration, NSImageSymbolScale, NSMenuItem, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSSize, NSString,
};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LegendIconKey {
    accent: Accent,
    opacity_percent: u8,
}

pub(super) struct RowRenderCache {
    font: Retained<NSFont>,
    paragraph_style: Retained<NSMutableParagraphStyle>,
    legend_icons: RefCell<HashMap<LegendIconKey, Option<Retained<NSImage>>>>,
}

impl RowRenderCache {
    pub(super) fn new() -> Self {
        Self {
            font: stat_font(),
            paragraph_style: row_paragraph_style(),
            legend_icons: RefCell::new(HashMap::new()),
        }
    }

    pub(super) fn legend_icon(
        &self,
        accent: Accent,
        opacity_percent: u8,
    ) -> Option<Retained<NSImage>> {
        let key = LegendIconKey {
            accent,
            opacity_percent,
        };
        if let Some(icon) = self.legend_icons.borrow().get(&key) {
            return icon.clone();
        }
        let color =
            color_for_accent(accent).colorWithAlphaComponent(f64::from(opacity_percent) / 100.0);
        let icon = make_legend_icon(&color);
        self.legend_icons.borrow_mut().insert(key, icon.clone());
        icon
    }
}

pub(super) fn make_stat_item(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(""),
            None,
            &NSString::from_str(""),
        )
    };
    item.setEnabled(true);
    item
}

pub(super) fn app_row_attributed(
    row: &StatRow,
    accent: &NSColor,
    render_cache: &RowRenderCache,
) -> Retained<NSAttributedString> {
    let primary = accent.colorWithAlphaComponent(1.0);
    stat_row_attributed_colored(
        row,
        primary.clone(),
        accent.colorWithAlphaComponent(0.65),
        primary,
        render_cache,
    )
}

pub(super) fn legend_row_attributed(
    row: &LegendRow,
    accent: &NSColor,
    render_cache: &RowRenderCache,
) -> Retained<NSAttributedString> {
    // Row hierarchy (#23): the total (App Memory, User) keeps full label
    // strength; derived breakdown rows are demoted on the opacity ramp so the
    // Accent hue survives Warning/Critical instead of flattening to gray.
    let label_color = if row.primary {
        accent.colorWithAlphaComponent(1.0)
    } else {
        accent.colorWithAlphaComponent(DEMOTED_LABEL_ALPHA)
    };
    stat_row_attributed_colored(
        &StatRow {
            primary: row.label.clone(),
            tail: Some(row.value.clone()),
            bundle_path: None,
        },
        label_color,
        NSColor::secondaryLabelColor(),
        NSColor::labelColor(),
        render_cache,
    )
}

pub(super) fn make_placeholder_icon() -> Retained<NSImage> {
    // Transparent 16pt square so non-icon rows align with app rows that carry an icon.
    NSImage::initWithSize(NSImage::alloc(), NSSize::new(ROW_ICON_SIZE, ROW_ICON_SIZE))
}

fn make_row_icon(name: &str) -> Option<Retained<NSImage>> {
    let symbol_name = NSString::from_str(name);
    let desc = NSString::from_str("");
    let base =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
    let config = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Medium);
    let image = base.imageWithSymbolConfiguration(&config)?;
    image.setSize(NSSize::new(ROW_ICON_SIZE, ROW_ICON_SIZE));
    image.setTemplate(true);
    Some(image)
}

pub(super) fn set_row_icon(item: &NSMenuItem, name: &str, fallback: &NSImage) {
    match make_row_icon(name) {
        Some(icon) => item.setImage(Some(&icon)),
        None => item.setImage(Some(fallback)),
    }
}

pub(super) fn make_action_icon(name: &str) -> Option<Retained<NSImage>> {
    let desc = NSString::from_str("");
    let symbol_name = NSString::from_str(name);
    let base =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
    let config = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Small);
    let image = base.imageWithSymbolConfiguration(&config)?;
    image.setTemplate(true);
    Some(image)
}

fn make_legend_icon(color: &NSColor) -> Option<Retained<NSImage>> {
    let symbol_name = NSString::from_str("circle.fill");
    let desc = NSString::from_str("");
    let base =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, Some(&desc))?;
    let scale = NSImageSymbolConfiguration::configurationWithScale(NSImageSymbolScale::Small);
    let tint = NSImageSymbolConfiguration::configurationWithHierarchicalColor(color);
    let config = scale.configurationByApplyingConfiguration(&tint);
    let image = base.imageWithSymbolConfiguration(&config)?;
    image.setSize(NSSize::new(ROW_ICON_SIZE, ROW_ICON_SIZE));
    Some(image)
}

fn attrs_for(
    color: Retained<NSColor>,
    font: Retained<NSFont>,
) -> Retained<NSDictionary<NSString, AnyObject>> {
    unsafe {
        let color_obj = Retained::cast_unchecked::<AnyObject>(color);
        let font_obj = Retained::cast_unchecked::<AnyObject>(font);
        NSDictionary::from_retained_objects(
            &[NSForegroundColorAttributeName, NSFontAttributeName],
            &[color_obj, font_obj],
        )
    }
}

pub(super) fn stat_row_attributed(
    row: &StatRow,
    primary_color: Retained<NSColor>,
    render_cache: &RowRenderCache,
) -> Retained<NSAttributedString> {
    stat_row_attributed_colored(
        row,
        primary_color.clone(),
        NSColor::secondaryLabelColor(),
        primary_color,
        render_cache,
    )
}

fn stat_row_attributed_colored(
    row: &StatRow,
    primary_color: Retained<NSColor>,
    tail_color: Retained<NSColor>,
    delta_color: Retained<NSColor>,
    render_cache: &RowRenderCache,
) -> Retained<NSAttributedString> {
    let font = render_cache.font.clone();
    let primary_attrs = attrs_for(primary_color, font.clone());
    let primary_str = NSString::from_str(&row.primary);
    let primary = unsafe { NSAttributedString::new_with_attributes(&primary_str, &primary_attrs) };

    let Some(tail) = &row.tail else {
        return primary;
    };

    let result = NSMutableAttributedString::new();
    result.appendAttributedString(&primary);

    let separator_attrs = attrs_for(NSColor::secondaryLabelColor(), font.clone());
    let separator_str = NSString::from_str("\t");
    let separator =
        unsafe { NSAttributedString::new_with_attributes(&separator_str, &separator_attrs) };
    result.appendAttributedString(&separator);

    // Split tail on \t so the delta can use the stronger Accent opacity while sharing
    // a single right-aligned tail column with the footprint. No reserved delta
    // gutter: the row's right edge collapses when nothing is rising.
    let mut tail_parts = tail.splitn(2, '\t');
    let footprint_part = tail_parts.next().unwrap_or("");
    let delta_part = tail_parts.next();

    let tail_attrs = attrs_for(tail_color.clone(), font.clone());
    let footprint_str = NSString::from_str(footprint_part);
    let footprint = unsafe { NSAttributedString::new_with_attributes(&footprint_str, &tail_attrs) };
    result.appendAttributedString(&footprint);

    if let Some(delta) = delta_part {
        let gap_str = NSString::from_str("  ");
        let gap = unsafe { NSAttributedString::new_with_attributes(&gap_str, &tail_attrs) };
        result.appendAttributedString(&gap);

        let delta_attrs = attrs_for(delta_color, font.clone());
        let delta_str = NSString::from_str(delta);
        let delta_attr =
            unsafe { NSAttributedString::new_with_attributes(&delta_str, &delta_attrs) };
        result.appendAttributedString(&delta_attr);
    }

    apply_paragraph_style(&result, &render_cache.paragraph_style);
    Retained::into_super(result)
}

fn apply_paragraph_style(s: &NSMutableAttributedString, style: &Retained<NSMutableParagraphStyle>) {
    let style_obj = unsafe { Retained::cast_unchecked::<AnyObject>(style.clone()) };
    let range = objc2_foundation::NSRange {
        location: 0,
        length: s.length(),
    };
    unsafe {
        s.addAttribute_value_range(NSParagraphStyleAttributeName, &style_obj, range);
    }
}

pub(super) fn loading_attributed_title(
    render_cache: &RowRenderCache,
) -> Retained<NSAttributedString> {
    stat_row_attributed(
        &StatRow {
            primary: "Loading…".to_string(),
            tail: None,
            bundle_path: None,
        },
        NSColor::secondaryLabelColor(),
        render_cache,
    )
}

pub(super) fn unavailable_attributed_title(
    render_cache: &RowRenderCache,
) -> Retained<NSAttributedString> {
    stat_row_attributed(
        &StatRow {
            primary: "Unavailable".to_string(),
            tail: None,
            bundle_path: None,
        },
        NSColor::secondaryLabelColor(),
        render_cache,
    )
}
