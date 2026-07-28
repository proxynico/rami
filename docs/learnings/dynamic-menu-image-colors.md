## Context

Legend swatches (`circle.fill` via `configurationWithHierarchicalColor`) turned
solid black in dark menus after being created under a light drawing appearance.
The status-item badge path had already hit the same trap.

Attributed menu titles hit the sibling failure: Neutral rows called
`labelColor.colorWithAlphaComponent(...)` when building `NSAttributedString`s.
That freezes white (from dark) or black (from light) into the title, so after a
system appearance flip the dropdown text vanishes against the new menu chrome.

## Lesson

Do not tint menu `NSImage`s with hierarchical SF Symbol configs when the color
is dynamic (`labelColor`, opacity-ramped accents). Those configs bake the
creation-time appearance. Prefer `imageWithSize_flipped_drawingHandler` and
apply `colorWithAlphaComponent` inside the handler so the color resolves on
each draw.

Do not call `colorWithAlphaComponent` on `labelColor` (or other catalog colors)
when building attributed titles either — even `alpha: 1.0` bakes. Use
`color_for_accent_alpha`: full Neutral stays `labelColor`; demoted Neutral maps
to `secondaryLabelColor` / `tertiaryLabelColor`; Warning/Critical may alpha-ramp
the semantic system colors.

## When It Applies

Any dropdown or menu-bar image that must carry Neutral/Warning/Critical accent
color or an opacity ramp, rather than a template glyph. Any attributed
`NSMenuItem` title that carries the Accent opacity ramp.

## Evidence or Example

- Bug: hierarchical create-under-light → sample-under-dark stays `rgba(0,0,0,1)`
- Bug: `labelColor.colorWithAlphaComponent(1)` create-under-dark → sample-under-light stays `rgba(1,1,1,1)`
- Fix: `src/tray/render.rs` (`make_legend_icon`), `src/status_icon.rs`,
  `src/tray/style.rs` (`color_for_accent_alpha`)
- Regression: `legend_icon_created_in_light_mode_stays_visible_in_dark_menus`,
  `accent_alpha_created_in_dark_mode_stays_readable_in_light_menus`

## Related Lessons

None.
