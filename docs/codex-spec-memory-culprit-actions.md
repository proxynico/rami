# Codex Spec - Memory Culprit Actions

## 1. Purpose

This is a Codex implementation spec for the next `rami` behavior layer.

`rami` should stay a tiny macOS menu bar RAM utility. The job is not to become
Activity Monitor. The job is to answer three questions quickly:

1. Is RAM going up?
2. Who is responsible?
3. What quick action can I take?

The app should remain quiet until memory movement matters. When it matters, the
dropdown should point at the likely culprit without making the user interpret a
dashboard.

## 2. Product Stance

Pick the culprit-finder shape.

Keep:

- one menu bar glyph
- one native `NSMenu`
- at most five app rows
- public macOS APIs
- no persistent settings window
- no graphs

Add:

- trend state
- per-app deltas
- one likely culprit row
- one safe quit action
- one high-pressure notification
- automatic app sampling when pressure rises

Do not add CPU, disk, network, per-tab browser memory, charts, a custom popover,
or a broad system dashboard.

## 3. Current Baseline

The shipped app already has:

- global RAM sampling in `src/memory.rs`
- process/app footprint sampling in `src/process_memory.rs`
- app rows grouped under outer `.app` bundles
- top-five app display
- dropdown formatting in `src/format.rs`
- native menu wiring in `src/tray.rs`
- app state and refresh cadence in `src/app.rs`

Build on that shape. Do not replace the native menu.

## 4. Feature Set

### 4.1 Menu Bar Trend Cue

The menu bar should show whether memory is stable, rising, or rising fast.

Use recent global memory samples to compute trend. The cue should be subtle and
readable at menu bar size.

Recommended first version:

- keep the existing RAM gauge symbol
- keep normal template tint for stable/rising
- use red tint only for `High` pressure, as today
- add an optional tiny text suffix only if it looks clean, for example `+`
  for rising and `++` for rising fast

If the text suffix makes the menu bar noisy, do not ship it. Prefer a quiet
menu bar over a clever one.

Trend thresholds:

- `Stable`: used RAM changed by less than 300 MB over the last 2 minutes
- `Rising`: used RAM increased by 300 MB to 999 MB over the last 2 minutes
- `RisingFast`: used RAM increased by 1 GB or more over the last 2 minutes

Use bytes internally. Render labels only where needed.

### 4.2 App Deltas

App usage rows should show movement, not only size.

Current shape:

```text
Zen 516 MB  3%
```

Target shape when there is a meaningful delta:

```text
Zen 516 MB  +120 MB
```

Rules:

- delta compares current footprint against the same app group from the previous
  app sample
- show positive deltas first; those are most useful for culprit detection
- hide tiny deltas below 50 MB
- negative deltas may be omitted in v1 to keep the menu compact
- keep total app rows capped at five
- sort by positive delta descending when any meaningful positive deltas exist
- otherwise sort by footprint descending, as today

This means a smaller app that is growing fast can appear above a large app that
is stable.

### 4.3 Likely Culprit Row

When app usage is available and at least one app has a meaningful positive
delta, insert one row above the top-five app list:

```text
Likely culprit: Zen +120 MB
```

Placement:

```text
MEMORY
53%  9.0 / 17.2 GB

APPS
Likely culprit: Zen +120 MB
Zen 516 MB  +120 MB
Codex 420 MB  +80 MB
...
```

Rules:

- only show the row when the top positive delta is at least 100 MB
- if multiple apps tie, pick the larger footprint
- if app usage is unavailable, do not guess
- if app usage is hidden and pressure is normal, do not show the Apps section

The goal is one clear answer, not a ranked forensic report.

### 4.4 Quick Action - Quit App

Each app row should allow a safe quit action.

Recommended native shape:

- make each app row a submenu
- submenu item: `Quit <App Name>`
- optional submenu item: `Open Activity Monitor` can wait

Quit behavior:

- quit only processes that belong to the selected app group
- prefer a graceful termination signal
- do not force quit in v1
- do not quit `rami`
- do not offer quit for protected/system-ish non-app rows where grouping is
  ambiguous

If graceful quit fails, show no dramatic error UI. The row can remain on the
next refresh. This app should feel like a small control surface, not an alert
machine.

### 4.5 High-Pressure Notification

When memory pressure transitions into `High`, send one macOS notification:

```text
RAM pressure high
Top riser: Zen +420 MB
```

Rules:

- notify only on transition into `High`
- cooldown: 15 minutes
- if no app delta is available, use:
  `Open rami to check top apps`
- do not notify for normal periodic refreshes
- do not notify repeatedly while pressure remains high

If native notification wiring adds too much scope, implement the pressure
transition state first and leave notification delivery as the next small patch.

### 4.6 Auto-Sample Apps When Pressure Rises

If pressure becomes `Elevated` or `High`, `rami` should sample app usage even
when `Show App Usage` is off.

Rules:

- do not permanently turn on the user's `Show App Usage` preference
- keep the dropdown visually unchanged while pressure is normal and app usage
  is off
- when pressure is elevated/high, make app data ready so opening the menu can
  immediately show the likely culprit
- if the user toggles `Show App Usage` off manually, still allow emergency
  background sampling during elevated/high pressure

This feature makes the app useful at exactly the moment the user needs it.

## 5. Data Model

Add a small trend/delta layer without changing the existing public model more
than needed.

Suggested types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTrend {
    Stable,
    Rising,
    RisingFast,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMemoryDelta {
    pub name: String,
    pub footprint_bytes: u64,
    pub delta_bytes: i64,
    pub can_quit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LikelyCulprit {
    pub name: String,
    pub delta_bytes: u64,
}
```

Implementation can choose different names if they fit the current code better,
but the concepts should stay explicit.

## 6. State And Refresh

Keep global memory refresh at 5 seconds.

Keep process sampling at 30 seconds during normal visible app usage.

Add:

- a rolling global memory history covering roughly 2 minutes
- the previous app sample keyed by stable app group
- last notification timestamp
- last pressure state

Sampling rules:

- manual `Refresh`: refresh memory and app usage if visible or pressure is not
  normal
- auto refresh: refresh global memory every 5 seconds
- app usage visible: refresh app usage every 30 seconds
- pressure elevated/high: refresh app usage every 30 seconds even if hidden
- pressure high transition: refresh app usage immediately before notification
  if the last app sample is stale

## 7. Formatting

Use compact labels.

Memory trend in dropdown:

```text
MEMORY
53%  9.0 / 17.2 GB  Rising
```

App rows:

```text
Zen 516 MB  +120 MB
Codex 420 MB  +80 MB
Chrome 1.2 GB
```

Only include a delta tail when useful. If there is no meaningful delta, retain
the current footprint/percent style or a compact footprint-only style.

Recommended display priority:

1. likely culprit row
2. rows with positive deltas
3. largest footprint rows

## 8. Quick Action Safety

Quit action must be conservative.

Safe to offer:

- normal `.app` groups with a bundle path
- user-owned processes grouped under that app

Do not offer:

- `rami`
- empty group names
- kernel/system rows
- rows where the sampler only has a fallback executable name and no stable app
  bundle path

The implementation should not require Accessibility permission.

## 9. Acceptance Criteria

Codex should consider the feature done only when all are true:

- menu bar still launches as an accessory app with one quiet RAM cue
- dropdown still uses native `NSMenu`
- app rows remain capped at five
- app rows can show meaningful positive deltas
- likely culprit row appears only when there is a real positive delta
- high pressure triggers at most one notification per cooldown window
- elevated/high pressure can prepare app samples even when app usage is hidden
- safe app quit is available only for app-bundle groups
- `rami` cannot offer to quit itself
- tests cover trend classification, delta sorting, culprit selection, and quit
  eligibility
- `cargo fmt`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`
  pass
- packaged app builds through `./scripts/build-app.sh`
- installed app can be replaced in `/Applications/rami.app` and launched

## 10. Non-Goals

Do not build:

- graphs
- custom popover
- settings window
- force quit
- CPU/network/disk metrics
- per-tab browser breakdown
- historical memory archive
- cloud sync
- menu bar text dashboard

These are all reasonable apps. They are not `rami`.

## 11. Suggested Implementation Order

1. Add memory history and trend classification.
2. Add app-sample history and delta calculation.
3. Update dropdown formatting with deltas and likely culprit.
4. Add pressure-triggered hidden app sampling.
5. Add notification state and delivery.
6. Add conservative quit submenu for app-bundle groups.
7. Build, install over `/Applications/rami.app`, launch, and verify.

Keep each commit small enough to inspect.
