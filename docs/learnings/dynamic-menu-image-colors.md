## Context

Legend swatches (`circle.fill` via `configurationWithHierarchicalColor`) turned
solid black in dark menus after being created under a light drawing appearance.
The status-item badge path had already hit the same trap.

## Lesson

Do not tint menu `NSImage`s with hierarchical SF Symbol configs when the color
is dynamic (`labelColor`, opacity-ramped accents). Those configs bake the
creation-time appearance. Prefer `imageWithSize_flipped_drawingHandler` and
apply `colorWithAlphaComponent` inside the handler so the color resolves on
each draw.

## When It Applies

Any dropdown or menu-bar image that must carry Neutral/Warning/Critical accent
color or an opacity ramp, rather than a template glyph.

## Evidence or Example

- Bug: hierarchical create-under-light → sample-under-dark stays `rgba(0,0,0,1)`
- Fix: `src/tray/render.rs` (`make_legend_icon`) and `src/status_icon.rs`
- Regression: `legend_icon_created_in_light_mode_stays_visible_in_dark_menus`

## Related Lessons

None.
