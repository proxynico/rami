## Context

A pressure-accent palette change updated the accent decision and the
glossary, but `BUILDING.md`'s "Previewing the pressure accents" section
still said Warning was orange. Whole-branch review caught it because that
section is what people read while running `RAMI_FORCE_PRESSURE=warning`.

## Lesson

Pressure-accent vocabulary lives in three places: the Accent and Status
gauge entries in `CONCEPTS.md`, the Pressure-driven accent decision in
`AGENTS.md`, and the preview paragraph in `BUILDING.md`. Changing
Neutral, Warning, or Critical colors requires updating all three in the
same change. Otherwise forced-pressure previews look broken against
stale instructions.

Also: `RisingFast` is trend-driven (`badge_for_state` on `MemoryTrend`),
not a Critical-vs-Warning differentiator. Do not document it as the
thing that keeps Critical distinct when Warning and Critical share one
alert color.

## When It Applies

Any change to `color_for_accent`, `color_for_rings`, status tinting, or
pressure-band copy in the dropdown or menu bar.

## Evidence or Example

- Stale line: `BUILDING.md` Previewing the pressure accents (pre-fix)
- Fix commit: `1d3f52e` on `calm-orange-memory-rings`
- Badge truth: `src/status_icon.rs` (`badge_for_state`)

## Related Lessons

- [dynamic-menu-image-colors.md](dynamic-menu-image-colors.md)
