# Feature Plan — Dropdown Polish (Battery-menu style)

The menu bar gauge (v2 / `f98a3a1`) stays as-is. This plan rebuilds the
dropdown to match the polish of Apple's own status-bar menus (Battery, Sound,
Control Center extras): section headers, primary-value-with-secondary-tail
rows, SF Symbols on actions, tabular digits, and key equivalents.

## 1. Goal

Open the dropdown and have it read as native macOS Sonoma+, not as a Rust app
imitating one. Specifically: the number is the visual anchor, supporting
detail is dim, structure is enforced by section headers (not separators), and
every action row carries the affordances Apple's own menus do.

## 2. Target visual

```
MEMORY                           ← NSMenuItem.sectionHeaderWithTitle (macOS 14+)
  47%   5.7 / 16.0 GB            ← primary "47%" labelColor + tail secondaryLabel

PRESSURE                         ← sectionHeader
  Normal                         ← red label when High; otherwise default

SWAP                             ← sectionHeader
  1.2 GB
─────────────────────
↻  Refresh                  ⌘R
⏸  Auto-Refresh             ✓    ← icon flips ⏸/▶ with state; title static; ✓ on
   Launch at Login          ✓
─────────────────────
   Quit                     ⌘Q
```

Loading state (before first sample lands):

```
MEMORY
  Loading…                       ← single dim row replacing the 3 stat rows
─────────────────────
↻  Refresh                  ⌘R
⏸  Auto-Refresh             ✓
   Launch at Login
─────────────────────
   Quit                     ⌘Q
```

## 3. Decision summary

| # | Decision | Choice |
|---|----------|--------|
| 1 | Section headers | `NSMenuItem.sectionHeaderWithTitle:` (macOS 14+ API). Three sections: `MEMORY`, `PRESSURE`, `SWAP` |
| 2 | Stat row composition | One `NSMenuItem` per stat with an `NSAttributedString` title combining primary value (`labelColor`, default size) and secondary detail (`secondaryLabelColor`, same size). Two leading spaces of indent so the row visually nests under its section header |
| 3 | RAM row format | `47%   5.7 / 16.0 GB` — three spaces between primary and tail. Tabular digits |
| 4 | Pressure row | Value only (`Normal` / `Elevated` / `High`). At `High`, color the value `systemRedColor` to match the menu bar glyph |
| 5 | Swap row | `1.2 GB` only. No "Used" prefix |
| 6 | Tabular digits | `NSFont.monospacedDigitSystemFontOfSize_weight` applied to all numeric titles so `47%` and `9%` align between refreshes |
| 7 | Action SF Symbols | `arrow.clockwise` on Refresh; `pause.fill`/`play.fill` on Auto-Refresh (flips with state); none on Launch at Login or Quit (Apple omits) |
| 8 | Auto-Refresh title | Static `Auto-Refresh`. State conveyed by checkmark + icon flip, not by title swap |
| 9 | Key equivalents | `⌘R` on Refresh, `⌘Q` on Quit (set explicitly so the shortcut renders next to the row) |
| 10 | Loading placeholder | A single dim `Loading…` row under the `MEMORY` header. `PRESSURE` and `SWAP` headers + rows are hidden (or omitted entirely) until the first sample. No more `RAM: --%` text |
| 11 | Removed | The three current secondary-label printf rows; the `Pause/Resume` title flip; the `Refresh`/`Quit` strings flowing through `DropdownRows` (they're constants, not data) |

## 4. Acceptance criteria

- Dropdown opens with three section headers (`MEMORY`, `PRESSURE`, `SWAP`)
  rendered in macOS 14's native section-header style — small uppercase grey,
  no leading separator
- The number in the MEMORY row is in default text color; the `5.7 / 16.0 GB`
  tail is in `secondaryLabelColor`
- At `MemoryPressure::High`, the PRESSURE row value renders in
  `systemRedColor`; otherwise default
- `47%` and `9%` align column-wise between refreshes (visually verifiable by
  watching the dropdown across a few samples)
- Refresh row shows a leading `arrow.clockwise` SF Symbol and `⌘R` on the
  trailing edge
- Auto-Refresh row shows a leading `pause.fill` (when running) or `play.fill`
  (when paused) SF Symbol; title is the static string `Auto-Refresh`;
  checkmark is `on` when running
- Quit row shows `⌘Q` on the trailing edge
- Before the first sample, the menu shows `MEMORY` → `Loading…` and the
  action rows; no `RAM: --%` placeholder anywhere
- `cargo test` passes; `./scripts/build-app.sh` succeeds
- Manual QA: open the menu on light + dark menu bars at three RAM levels
  (~10%, ~50%, ~90%) and once at `High` pressure (force via `memory_pressure
  -S 4` or by loading the system); verify against screenshots from the
  current build

## 5. Phases

### Phase 1 — `src/format.rs`

- Replace `dropdown_rows` and `placeholder_dropdown_rows` with a typed model
  the tray can consume directly:
  ```rust
  pub struct StatRow { pub primary: String, pub tail: Option<String> }
  pub struct DropdownModel {
      pub memory: Option<StatRow>,   // None = loading
      pub pressure: Option<(String, bool)>, // (text, is_high)
      pub swap: Option<StatRow>,
  }
  pub fn dropdown_model(snapshot: MemorySnapshot) -> DropdownModel
  pub fn placeholder_dropdown_model() -> DropdownModel
  ```
- `gb_text` stays. New helper `gb_pair(used, total) -> String` returning
  `"5.7 / 16.0 GB"`.
- Drop `refresh` / `quit` from the model — they're constants, belong in
  `tray.rs`.
- Tests cover the loading model, a normal snapshot, and the High-pressure
  flag.

### Phase 2 — `src/tray.rs`

- Replace the constructor's straight-line `addItem` calls with section-aware
  construction. Helper `add_section(menu, title)` calls
  `NSMenuItem::sectionHeaderWithTitle:` and adds it.
- New helper `make_stat_item(primary: &str, tail: Option<&str>, color: Option<&NSColor>) -> Retained<NSMenuItem>`
  that builds the attributed title with tabular digits via
  `NSFont::monospacedDigitSystemFontOfSize_weight`. The item is disabled-but-
  enabled-via-`setEnabled(true)` + no action (read-only row, same trick the
  current code uses).
- `set_menu_rows` becomes `apply_model(model: &DropdownModel, ...)` and
  rebuilds only the stat rows when the model shape changes (loading → loaded
  or vice versa). When shape is stable, mutates titles in place via the
  existing change-cache pattern.
- Action row construction pulls SF Symbols via the existing
  `make_symbol_image` path with `.small` or `.medium` scale and template on.
  Cache the two icons (`pause.fill`, `play.fill`) and swap on toggle.
- Set `setKeyEquivalent` to `"r"` on Refresh and `"q"` on Quit; both with
  `NSEventModifierFlagCommand` (the default modifier for one-letter
  equivalents).
- Drop the title-flip on Auto-Refresh; only checkmark + icon swap.
- Drop the `Refresh: --` and `Quit: --` flow through `DropdownRows`.

### Phase 3 — `tests/`

- `menu_entries` integration test rewritten to assert the new shape:
  section headers, stat rows, action rows. Drop direct string equality
  on the printf rows; assert structural shape (3 sections, 3 stat rows or
  1 loading row, 4 action rows, separators in the right places).
- `format` tests cover `dropdown_model` for normal + loading + High-pressure
  cases.

### Phase 4 — Build & QA

- `cargo test`
- `./scripts/build-app.sh`
- Screenshot the dropdown in 6 states for personal verification:
  light + dark menu bar × {loading, normal ~50%, High pressure}

## 6. Risks / open verification

- **`NSMenuItem.sectionHeaderWithTitle:` exposure in `objc2-app-kit`** —
  the Cargo.toml-pinned version of `objc2-app-kit` may not expose this
  macOS 14 API. Verify first; if missing, fall back to a manually styled
  disabled `NSMenuItem` with an attributed title in small caps
  `secondaryLabelColor` (close visual match, slightly less native).
- **`monospacedDigitSystemFontOfSize:weight:` exposure** — same concern.
  Fallback: use the default system font and accept slight column jitter
  between refresh ticks (acceptable but less polished).
- **SF Symbol on menu item** — `NSMenuItem.setImage` puts an image on the
  leading edge. Verify `.small` scale renders at the right size next to
  menu text; `.medium` may overpower. Set template on so it picks up the
  menu's current text color.
- **Red pressure value on hover** — when the user moves the mouse over the
  `PRESSURE` stat row, the row gets a blue selection background. Red text
  on blue is poor contrast. Mitigation: the stat rows are read-only and
  shouldn't highlight on hover; verify by setting their action to `nil`
  and confirming AppKit suppresses the hover state. If it still
  highlights, consider disabling the row (greys out the value but kills
  the red signal too — accept the tradeoff or drop the red).
- **Loading → loaded transition** — switching from 1 row to 3 rows under
  the `MEMORY` header requires removing/inserting menu items, not just
  retitling. Implement once and cache the shape so subsequent refreshes
  hit the in-place mutation path.

## 7. Out of scope

- Any change to the menu bar gauge (icon, buckets, colors, tinting)
- New data sources (CPU, network, temperature, per-process breakdowns)
- Replacing `NSMenu` with a custom popover or NSPanel
- Settings panel or preferences window
- Committing `docs/` and `.claude/` to the tree
