use super::style::{
    color_for_accent, color_for_accent_alpha, row_paragraph_style, stat_font, DEMOTED_LABEL_ALPHA,
    ROW_ICON_SIZE,
};
use crate::format::{Accent, LegendRow, StatRow};
use crate::presentation::MenuMetrics;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSImage,
    NSImageSymbolConfiguration, NSImageSymbolScale, NSMenuItem, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName,
};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString,
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
    pub(super) fn new(metrics: MenuMetrics) -> Self {
        Self {
            font: stat_font(metrics.type_scale.stat_row),
            paragraph_style: row_paragraph_style(metrics.row_tail_tab()),
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
        // Pass the dynamic accent color and opacity separately. Applying alpha
        // up front (or tinting via hierarchical SF Symbols) bakes labelColor
        // against the creation appearance and leaves black swatches in dark menus.
        let icon = make_legend_icon(&color_for_accent(accent), opacity_percent);
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
    accent: Accent,
    render_cache: &RowRenderCache,
) -> Retained<NSAttributedString> {
    let primary = color_for_accent_alpha(accent, 1.0);
    stat_row_attributed_colored(
        row,
        primary.clone(),
        // Footprint tails are demoted detail: same single demotion strength
        // as every other demoted row (#23), not a bespoke alpha.
        color_for_accent_alpha(accent, DEMOTED_LABEL_ALPHA),
        primary,
        render_cache,
    )
}

pub(super) fn legend_row_attributed(
    row: &LegendRow,
    accent: Accent,
    render_cache: &RowRenderCache,
) -> Retained<NSAttributedString> {
    // Row hierarchy (#23): the total (App Memory, User) keeps full label
    // strength; derived breakdown rows are demoted on the opacity ramp so the
    // Accent hue survives Warning/Critical instead of flattening to gray.
    let label_color = if row.primary {
        color_for_accent_alpha(accent, 1.0)
    } else {
        color_for_accent_alpha(accent, DEMOTED_LABEL_ALPHA)
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

fn make_legend_icon(color: &NSColor, opacity_percent: u8) -> Option<Retained<NSImage>> {
    // Draw lazily so dynamic accent colors (labelColor under Neutral) resolve
    // against the menu's current light/dark appearance. Hierarchical SF Symbol
    // tints bake the creation-time appearance and leave black swatches in dark
    // menus after a light-mode create — same trap status_icon documents.
    let color = color.retain();
    let alpha = f64::from(opacity_percent) / 100.0;
    let size = NSSize::new(ROW_ICON_SIZE, ROW_ICON_SIZE);
    let handler = RcBlock::new(move |rect: NSRect| -> Bool {
        color.colorWithAlphaComponent(alpha).setFill();
        // Keep the swatch inside the 16pt menu-item slot at roughly SF Symbol
        // Small scale — inset leaves a quiet margin around the filled dot.
        let inset = 4.0;
        let dot = NSRect::new(
            NSPoint::new(rect.origin.x + inset, rect.origin.y + inset),
            NSSize::new(
                rect.size.width - inset * 2.0,
                rect.size.height - inset * 2.0,
            ),
        );
        NSBezierPath::bezierPathWithOvalInRect(dot).fill();
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(size, false, &handler);
    image.setTemplate(false);
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

#[cfg(test)]
mod tests {
    use super::*;
    use block2::RcBlock;
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSBezierPath,
        NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext,
    };
    use objc2_foundation::{NSInteger, NSPoint, NSRect};

    fn appearance(name: &objc2_foundation::NSString) -> Retained<NSAppearance> {
        NSAppearance::appearanceNamed(name).expect("named appearance")
    }

    fn sample_center(image: &NSImage, drawing_appearance: &NSAppearance) -> (f64, f64, f64, f64) {
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                16,
                16,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
        }
        .expect("bitmap");
        NSGraphicsContext::saveGraphicsState_class();
        let ctx =
            NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).expect("graphics context");
        NSGraphicsContext::setCurrentContext(Some(&ctx));
        NSColor::clearColor().setFill();
        let bounds = NSRect::new(NSPoint::ZERO, NSSize::new(16.0, 16.0));
        NSBezierPath::bezierPathWithRect(bounds).fill();
        let image = image.retain();
        let draw = RcBlock::new(move || {
            image.drawInRect_fromRect_operation_fraction(
                bounds,
                NSRect::ZERO,
                NSCompositingOperation::SourceOver,
                1.0,
            );
        });
        drawing_appearance.performAsCurrentDrawingAppearance(&draw);
        NSGraphicsContext::restoreGraphicsState_class();

        let color = rep
            .colorAtX_y(8 as NSInteger, 8 as NSInteger)
            .expect("center pixel");
        let mut r = 0.0;
        let mut g = 0.0;
        let mut b = 0.0;
        let mut a = 0.0;
        unsafe {
            color.getRed_green_blue_alpha(&mut r, &mut g, &mut b, &mut a);
        }
        (r, g, b, a)
    }

    fn sample_fill(color: &NSColor, drawing_appearance: &NSAppearance) -> (f64, f64, f64, f64) {
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                4,
                4,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
        }
        .expect("bitmap");
        NSGraphicsContext::saveGraphicsState_class();
        let ctx =
            NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).expect("graphics context");
        NSGraphicsContext::setCurrentContext(Some(&ctx));
        let color = color.retain();
        let draw = RcBlock::new(move || {
            color.setFill();
            let bounds = NSRect::new(NSPoint::ZERO, NSSize::new(4.0, 4.0));
            NSBezierPath::bezierPathWithRect(bounds).fill();
        });
        drawing_appearance.performAsCurrentDrawingAppearance(&draw);
        NSGraphicsContext::restoreGraphicsState_class();
        let sampled = rep
            .colorAtX_y(1 as NSInteger, 1 as NSInteger)
            .expect("pixel");
        let mut r = 0.0;
        let mut g = 0.0;
        let mut b = 0.0;
        let mut a = 0.0;
        unsafe {
            sampled.getRed_green_blue_alpha(&mut r, &mut g, &mut b, &mut a);
        }
        (r, g, b, a)
    }

    #[test]
    fn accent_alpha_created_in_dark_mode_stays_readable_in_light_menus() {
        // colorWithAlphaComponent on labelColor freezes the creation appearance
        // (white from dark stays white in light). color_for_accent_alpha must
        // keep Neutral adaptive so attributed titles stay readable.
        use super::super::style::color_for_accent_alpha;
        use crate::format::Accent;

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
                *demoted.borrow_mut() =
                    Some(color_for_accent_alpha(Accent::Neutral, DEMOTED_LABEL_ALPHA));
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
    fn legend_icon_created_in_light_mode_stays_visible_in_dark_menus() {
        // Hierarchical SF Symbol tints bake labelColor at creation time. Icons
        // built while the process drawing appearance is light stay black when
        // later drawn into a dark menu — the failure mode from the dropdown.
        let light = appearance(unsafe { NSAppearanceNameAqua });
        let dark = appearance(unsafe { NSAppearanceNameDarkAqua });
        let built = RefCell::new(None);
        {
            let build = RcBlock::new(|| {
                *built.borrow_mut() = make_legend_icon(&NSColor::labelColor(), 100);
            });
            light.performAsCurrentDrawingAppearance(&build);
        }
        let image = built.into_inner().expect("legend icon");

        let (r, g, b, a) = sample_center(&image, &dark);
        assert!(a > 0.5, "legend swatch should be opaque, got alpha {a:.2}");
        assert!(
            r > 0.5 && g > 0.5 && b > 0.5,
            "dark menu must not keep a light-baked black swatch, got rgba({r:.2},{g:.2},{b:.2},{a:.2})"
        );
    }
}
