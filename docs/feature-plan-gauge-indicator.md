# Feature Plan — Gauge Indicator (v2)

Supersedes `feature-plan-microchip-indicator-polish.md`. Single template-tinted SF
Symbol gauge replaces the chip + percent + bar combination. No color, no text.

## 1. Goal

The menu bar shows **one** glyph: an SF Symbol fuel-gauge that visually encodes RAM
usage. Fully template-rendered so it follows menu bar foreground color (white on
dark, black on light) like Wi-Fi, Bluetooth, and battery. No percent text. No
pressure tinting. Tooltip still surfaces precise numbers; dropdown is unchanged.

## 2. Decision summary

| # | Decision | Choice |
|---|----------|--------|
| 1 | Glyph family | `gauge.with.dots.needle.{0,33,50,67,100}percent` — verified present on macOS 14. The `.bottom.*percent` half-arc family only ships 0/50/100, so we use the fuller-arc dotted variant for 5-bucket fidelity |
| 2 | Bucketing | 5 equal-width buckets: `0..=19 → 0%`, `20..=39 → 33%`, `40..=59 → 50%`, `60..=79 → 67%`, `80..=100 → 100%` |
| 3 | Pressure tint | Template white at Normal/Elevated. **Red at High pressure only** — single warning state, kernel-pressure driven (not percent-driven), so cached-but-full RAM stays white if the system isn't actually squeezed |
| 4 | Menu bar text | **Removed entirely.** Icon-only |
| 5 | Symbol scale | `NSImageSymbolConfiguration::configurationWithScale(.large)` |
| 6 | Tooltip | **Removed entirely** — user-requested follow-up after first pass. Either click or look |
| 7 | RAM dropdown row | Now reads `RAM: {n}% — {used} of {total}` so the percent is visible on click. Placeholder is `RAM: --% — 0.0 GB of 0.0 GB` |
| 8 | Dropdown contrast work (12e7551, d4e3fee) | Stays as-is; revisit separately |

## 3. Acceptance criteria

- Menu bar shows exactly one glyph (the gauge), no text, no tooltip
- Glyph variant matches the bucket of `snapshot.used_percent`
- Glyph is template-tinted at all RAM levels (no yellow, no red)
- Dropdown RAM row leads with the percent: `RAM: {n}% — {used} of {total}`
- All `cargo test` targets pass
- New unit test covers `gauge_symbol_name` for each bucket boundary
- `PressureTint`, `pressure_tint`, `menu_bar_text`, `placeholder_text`,
  `menu_bar_tooltip`, `ram_meter`, `set_label`, `set_tooltip` (and their caches)
  are removed if they have no remaining callers
- No `eprintln!` or debug prints in shipped code
- `./scripts/build-app.sh` succeeds
- Manual QA: live verification on the user's menu bar across at least two
  RAM levels and both menu bar appearances

## 4. Phases

### Phase 1 — `src/format.rs`

- Add `pub fn gauge_symbol_name(percent: u8) -> &'static str` returning one of
  the five `gauge.with.needle.bottom.*percent` names
- Delete `pub fn menu_bar_text` (no callers after Phase 2)
- Delete `pub fn placeholder_text` (no callers after Phase 2)
- Delete `pub fn pressure_tint` and `pub enum PressureTint` (no callers after Phase 2)
- Keep `ram_meter` (still used by `menu_bar_tooltip`)
- Replace inline tests: drop `menu_bar_text_uses_five_bucket_mapping` and
  `pressure_tint_maps_each_pressure_to_expected_variant`; keep the tooltip test;
  add `gauge_symbol_name_buckets_by_percent` covering boundaries `{0, 19, 20, 39, 40, 59, 60, 79, 80, 100}`

### Phase 2 — `src/tray.rs`

- Drop the `tint: PressureTint` parameter from `set_image`; drop `last_tint`
- Drop the entire match-on-tint block (yellow/red branches)
- Always call `image.setTemplate(true)` after applying the symbol image
- Replace the `memorychip.fill` / `memorychip` symbol resolution with a helper
  that takes `snapshot.used_percent` and returns the gauge variant; cache by
  variant name not by `preferred`/`fallback`
- Delete `set_label`, `last_label`, and the `placeholder_text` call site
- `set_snapshot` no longer calls `set_label`; only `set_image` + `set_tooltip` +
  `set_menu_rows`
- `set_placeholder` no longer calls `set_label`; resolves to the 0% gauge variant
- Delete the now-stale comment at `src/tray.rs:85` about `labelColor` (the
  attributed-title path made it inaccurate)
- Re-verify the change-cache invariant: `set_image` should early-return when
  the requested gauge variant equals the cached one

### Phase 3 — `tests/format_tests.rs`

- Replace `menu_bar_text_returns_percent_only` with
  `gauge_symbol_name_returns_expected_variant_for_each_bucket` — same shape,
  different function under test
- Anything that imported `menu_bar_text` or `placeholder_text` updates to use
  `gauge_symbol_name`

### Phase 4 — Build, QA, ship

- `cargo test`
- `./scripts/build-app.sh`
- Screenshot grid: {0%, 25%, 50%, 75%, 100%} × {light menu bar, dark menu bar}
  — 10 images, paired with the captures from the previous polish PR for diff
- Open PR. Body cites this plan and the screenshot set

## 5. Risks / open verification

- **SF Symbol availability** — `gauge.with.needle.bottom.*percent` was added in
  macOS 13. The repo targets macOS 14, so this is safe, but the implementation
  must still gracefully fall back to `gauge.with.dots.needle.bottom.*percent`
  (verified to exist as long), then to no image, in case naming differs at
  runtime
- **Needle weight at small sizes** — `.large` scale was chosen for the previous
  chip and visually matched the menu bar; reuse it for the gauge. If QA shows
  the needle still reads as too thin, escalate per the polish plan's tiered
  ladder (text-style scale → point-size weight)
- **Template tinting in colored menu bars** — `image.setTemplate(true)` should
  cause AppKit to tint per the menu bar's foreground color. Verify on both a
  light wallpaper-influenced bar and a dark one
- **Reduced glanceability of pressure** — by removing the yellow/red tint, a
  user under memory pressure no longer sees an instant warning. This is a
  deliberate tradeoff for visual consistency with system icons; pressure still
  appears in the tooltip and dropdown. Revisit only if it bites in real use

## 6. Out of scope

- Dropdown contrast / attributed-title work (commits 12e7551, d4e3fee) — left
  intact for v2; if it bothers the user we revert in a separate dedicated plan
- Custom-drawn glyph (the Option 2 / Option 3 paths from the design discussion)
  — only revisit if the gauge family proves visually inadequate
- Committing `docs/` and `.claude/` to the tree — orthogonal repo-hygiene call
