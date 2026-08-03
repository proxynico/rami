# Calm Orange Memory Rings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use nicopowers:subagent-driven-development to implement this plan task-by-task. Use nicopowers:executing-plans only when the user explicitly asks for inline execution. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give memory ring strokes a calm system-orange tint under Normal pressure, move Warning (and keep Critical) to system red so calm and alert stay distinct, and amend the monochrome accent ADR/CONTEXT to match.

**Architecture:** Keep the `Accent` enum and pressure mapping. Change `color_for_accent` so Warning shares Critical’s red (legend, history, status tint, RisingFast composite). Add a rings-only helper that maps Neutral → `systemOrangeColor` and Warning/Critical → `systemRedColor`, and pass that into `MemoryRingsView::update` instead of `color_for_accent`. Labels, tracks, and Neutral legend stay monochrome.

**Tech Stack:** Rust, objc2 AppKit (`NSColor`), existing tray/style helpers, `cargo test` / `cargo fmt` / `cargo clippy --all-targets -- -D warnings`.

## Global Constraints

- Scope is memory ring strokes + accent palette shift (Warning → red) + ADR-0002 / CONTEXT.md vocabulary; do not recolor CPU/GPU sections beyond what `color_for_accent` already drives.
- Do not introduce the user’s macOS accent color.
- Follow `docs/learnings/dynamic-menu-image-colors.md`: do not bake Neutral via `labelColor.colorWithAlphaComponent`; catalog colors for orange/red may use alpha only where Warning/Critical already do.
- Verify with `cargo fmt`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`.
- Commit attribution is Nico only; no agent Co-Authored-By trailers.

---

## File Structure

- `src/tray/style.rs` — accent → NSColor mapping; new rings stroke helper + pure enum for tests
- `src/tray/mod.rs` — pass rings color from `color_for_rings` into `MemoryRingsView::update`
- `docs/adr/0002-monochrome-pressure-driven-accent.md` — amend Neutral rings exception + Warning→red
- `CONTEXT.md` — Accent / status gauge vocabulary

No new source files. `memory_view.rs` keeps taking `Retained<NSColor>`; it does not learn Accent.

---

### Task 1: Accent palette + rings stroke mapping

**Files:**
- Modify: `src/tray/style.rs`
- Test: `src/tray/style.rs` (`#[cfg(test)]` module)
- Modify: `docs/adr/0002-monochrome-pressure-driven-accent.md`
- Modify: `CONTEXT.md`

**Interfaces:**
- Consumes: `crate::format::Accent`, `crate::model::MemoryPressure`, existing `color_for_accent_alpha` / `status_tint_for_pressure`
- Produces:
  - `pub(super) enum RingStroke { CalmOrange, AlertRed }`
  - `pub(super) fn ring_stroke_for_accent(accent: Accent) -> RingStroke`
  - `pub(super) fn color_for_rings(accent: Accent) -> Retained<NSColor>`
  - `color_for_accent`: Neutral → `labelColor`; Warning | Critical → `systemRedColor`
  - Docs describing calm orange rings + red alert

- [ ] **Step 1: Write the failing tests**

In `src/tray/style.rs` test module, add (keep existing `status_tint_for_pressure` tests):

```rust
#[test]
fn ring_stroke_is_calm_orange_under_neutral() {
    assert_eq!(
        ring_stroke_for_accent(crate::format::Accent::Neutral),
        RingStroke::CalmOrange
    );
}

#[test]
fn ring_stroke_is_alert_red_under_warning_and_critical() {
    assert_eq!(
        ring_stroke_for_accent(crate::format::Accent::Warning),
        RingStroke::AlertRed
    );
    assert_eq!(
        ring_stroke_for_accent(crate::format::Accent::Critical),
        RingStroke::AlertRed
    );
}

#[test]
fn warning_and_critical_share_the_alert_red_accent_path() {
    // Palette contract: Warning no longer uses orange — orange is calm rings only.
    // Both map through color_for_accent to systemRedColor (cannot assert CGColor
    // equality portably); lock the rings stroke enum instead and document the
    // shared AlertRed path here.
    assert_eq!(
        ring_stroke_for_accent(crate::format::Accent::Warning),
        ring_stroke_for_accent(crate::format::Accent::Critical)
    );
}
```

Also add a small pure helper used by `color_for_accent` tests if you introduce `AccentPaint` — preferred shape in Step 3:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccentPaint {
    Label,
    AlertRed,
}

pub(super) fn accent_paint(accent: Accent) -> AccentPaint {
    match accent {
        Accent::Neutral => AccentPaint::Label,
        Accent::Warning | Accent::Critical => AccentPaint::AlertRed,
    }
}
```

And test:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tray::style::tests -- --nocapture`

Expected: FAIL — `ring_stroke_for_accent` / `RingStroke` / `accent_paint` not found (or wrong mappings if stubs exist).

- [ ] **Step 3: Implement mapping helpers and retarget `color_for_accent`**

Replace / extend `src/tray/style.rs` color section to:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccentPaint {
    Label,
    AlertRed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RingStroke {
    CalmOrange,
    AlertRed,
}

pub(super) fn accent_paint(accent: Accent) -> AccentPaint {
    match accent {
        Accent::Neutral => AccentPaint::Label,
        Accent::Warning | Accent::Critical => AccentPaint::AlertRed,
    }
}

pub(super) fn ring_stroke_for_accent(accent: Accent) -> RingStroke {
    match accent {
        Accent::Neutral => RingStroke::CalmOrange,
        Accent::Warning | Accent::Critical => RingStroke::AlertRed,
    }
}

pub(super) fn color_for_accent(accent: Accent) -> Retained<NSColor> {
    match accent_paint(accent) {
        AccentPaint::Label => NSColor::labelColor(),
        AccentPaint::AlertRed => NSColor::systemRedColor(),
    }
}

pub(super) fn color_for_rings(accent: Accent) -> Retained<NSColor> {
    match ring_stroke_for_accent(accent) {
        RingStroke::CalmOrange => NSColor::systemOrangeColor(),
        RingStroke::AlertRed => NSColor::systemRedColor(),
    }
}
```

Keep `color_for_accent_alpha` and `status_tint_for_pressure` behavior; Warning/Critical alpha path still calls `color_for_accent` (now red). Update the demotion comment that mentions “Accent hue in Warning and Critical” — still true, hue is red.

- [ ] **Step 4: Amend ADR and CONTEXT**

Rewrite `docs/adr/0002-monochrome-pressure-driven-accent.md` body to state:

- Dropdown text/legend/history stay monochrome under Normal (`labelColor` opacity ramp).
- Memory **ring strokes** are the calm exception: Normal → system orange; Warning/Critical → system red.
- Warning no longer uses orange (orange is reserved for calm rings). Status gauge: Normal stays untinted template; Warning/Critical tint red; Critical still uses RisingFast badge when trend says so.
- Rejected still: multi-hue category palettes; user’s macOS accent.

Update `CONTEXT.md` Presentation section:

- **Accent:** calm Neutral = adaptive label color for chrome/legend; memory ring strokes use system orange under Normal; Warning and Critical use system red for accent chrome (including rings). User’s macOS accent ignored.
- **Status gauge:** Normal = template; Warning/Critical = red tint; Critical RisingFast badge unchanged in role.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib tray::style::tests`

Expected: PASS (all style tests green).

- [ ] **Step 6: Commit**

```bash
git add src/tray/style.rs docs/adr/0002-monochrome-pressure-driven-accent.md CONTEXT.md
git commit -m "$(cat <<'EOF'
Map calm rings to orange and collapse Warning accent to red.

EOF
)"
```

---

### Task 2: Wire memory rings to `color_for_rings`

**Files:**
- Modify: `src/tray/mod.rs` (imports + `apply_model` rings update)
- Test: rely on Task 1 unit tests; run full `cargo test` / clippy as proof the call site compiles and nothing regresses

**Interfaces:**
- Consumes: `color_for_rings(Accent) -> Retained<NSColor>` from Task 1; `MemoryRingsView::update(&self, rings: &[RingDisplay; 2], accent: Retained<NSColor>)`
- Produces: rings view receives calm orange under Neutral; history/legend/status still use `color_for_accent`

- [ ] **Step 1: Write a failing compile-guard / call-site note test (optional thin)**

No new UI screenshot test. If the crate has no tray integration test for this, skip a new test file — instead change the call site and use the full suite as the proof. Prefer adding one unit-level assertion already covered in Task 1; this task is wiring only.

- [ ] **Step 2: Change `apply_model` to color rings via `color_for_rings`**

In `src/tray/mod.rs`:

1. Extend the `style` import list to include `color_for_rings`.
2. In `apply_model`, replace the rings update block so history keeps `color_for_accent` but rings use `color_for_rings`:

```rust
let accent_changed = self.last_accent.get() != *accent;
let accent_color = color_for_accent(*accent);
let rings_color = color_for_rings(*accent);
if accent_changed || self.last_rings.borrow().as_ref() != Some(&memory.rings) {
    self.rings_view.update(&memory.rings, rings_color);
    *self.last_rings.borrow_mut() = Some(memory.rings.clone());
}
if accent_changed || self.last_history.borrow().as_ref() != Some(&memory.history) {
    self.history_view
        .update(&memory.history, accent_color.clone());
    *self.last_history.borrow_mut() = Some(memory.history.clone());
}
```

Do not pass orange into `history_view` or legend helpers.

- [ ] **Step 3: Format, test, clippy**

Run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: all pass; no warnings.

- [ ] **Step 4: Commit**

```bash
git add src/tray/mod.rs
git commit -m "$(cat <<'EOF'
Tint memory rings with the calm-orange stroke mapping.

EOF
)"
```

---

## Self-Review

1. **Spec coverage:** Calm orange rings under Normal → Task 1 mapping + Task 2 wiring. Warning→red (rings, legend, status) → Task 1 `accent_paint` / `color_for_accent`. Critical red + RisingFast unchanged in role → status path still uses `color_for_accent` + existing `make_status_image`. ADR/CONTEXT → Task 1. Legend Neutral monochrome → unchanged Neutral `labelColor` path. No CPU/GPU special casing required.
2. **Placeholders:** none.
3. **Type consistency:** `RingStroke`, `AccentPaint`, `color_for_rings`, `ring_stroke_for_accent`, `accent_paint` names match across tasks; `MemoryRingsView::update` still takes `Retained<NSColor>`.
