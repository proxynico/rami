# Feature Spec — Actionable Memory Monitor

**Author:** DeepSeek  
**Date:** 2026-05-06  
**Status:** Draft

---

## 1. Goal

Turn `rami` from a passive observer into a tight feedback loop:

> I see memory climbing. I know who's responsible. I stop them. Done.

Four additions close that loop without adding weight:

1. A **trend indicator** in the menu bar — invisible when memory is stable,
   impossible to miss when it is climbing.
2. A **pinned delta** in the dropdown — quantifies the rate of change so the
   user doesn't have to memorize the last reading.
3. **Kill from the app list** — right-click or keyboard shortcut to `SIGTERM`
   the top offender directly from the dropdown.
4. **Keyboard shortcuts 1–5 on app rows** — press a digit to kill without
   moving the mouse.

The gauge icon, the dropdown layout, the 5-second timer, and the app memory
section all stay exactly as they are. This spec adds signal and action on top
of them, not a replacement.

---

## 2. Current App Shape

`rami` today:

- updates the gauge icon and dropdown every 5 seconds via `NSTimer`
- stores the current `MemorySnapshot` (percent, pressure, swap) in `AppState`
- optionally scans process footprints every 30 seconds when `Show App Usage` is
  enabled
- renders at most five app rows as non-interactive `NSMenuItem`s

The app has no historical memory — it only knows the current sample. All four
features require keeping at least one previous sample in memory.

---

## 3. Recommendation

Build all four features inside the existing Rust/AppKit stack. No new
dependencies. No new permissions. No custom UI.

The trend and delta features need a rolling history of at most two samples and
a timestamp. The kill features need `libc::kill` and a way to map an app row
back to a PID.

This is the right approach because:

- `libc::kill` is already available through the existing `libc` dependency
- trend and delta are pure arithmetic on existing data
- the app rows already carry the data needed to act (PIDs during sampling)
- all four features together add maybe 150 lines of Rust

---

## 4. Alternatives Considered

### Option A — Full process panel with built-in Terminal

A custom popover that lets the user inspect, filter, and kill processes with a
command-line-style interface.

Reject. It is the opposite of minimalism. It replaces the native menu with a
bespoke UI, needs scroll views, text fields, and a completely different
interaction model.

### Option B — `osascript`-based kill

Use `osascript` to tell Finder or System Events to quit an application.

Reject. Shelling out is slow, fragile, and only works for `.app` processes.
Non-app processes like `node` or `python` would be unkillable.

### Option C — `libc::kill` on the main thread with PID mapping

Keep PIDs alongside app names during sampling, and call `libc::kill(pid,
SIGTERM)` from the main thread when the user acts.

Choose this. It is fast, native, zero-dependency, and has no failure modes
beyond the process already being gone.

---

## 5. User Experience

### 5.1 Trend Indicator (Menu Bar)

The menu bar icon stays the same SF Symbol gauge. When memory has changed by
**2 or more percentage points** since the previous sample, a small arrow
appears:

- `arrow.up.right` if memory increased by >= 2pp
- `arrow.down.right` if memory decreased by >= 2pp
- nothing if change < 2pp (stable)

The arrow is template-tinted, monochrome, and placed to the left of the gauge
as a secondary image (via `NSTextAttachment` or a composite approach using
`NSImage`).

When memory pressure is High, the red tint on the gauge takes precedence. The
arrow stays template-colored (does not turn red) so the red gauge remains the
unambiguous "critical" signal.

Implementation approach (choose the one that works with the current button):

- **Preferred:** compose a single `NSImage` by drawing the arrow and gauge
  symbols side by side into a new image context. This avoids fighting with
  `NSStatusBarButton` layout.
- **Fallback:** set the button title to the arrow symbol and the image to the
  gauge. Requires testing for spacing.

### 5.2 Pinned Delta (Dropdown)

Below the Memory row, add a subtle delta line:

```text
MEMORY
  53%  (+3% in 30s)    9.0 / 17.2 GB

PRESSURE
  Normal

SWAP
  1.2 GB
```

The delta `(+3% in 30s)` is appended to the Memory stat row's tail, rendered in
the same secondary label color as the GB pair. It compares the current sample
to the sample from 30 seconds ago (six ticks). When fewer than six ticks have
elapsed since launch, show `(+3% since start)`.

Threshold:

- changes below 1pp show nothing (no delta text)
- increases show `(+N%)` 
- decreases show `(-N%)`
- the time window label is always `in 30s` after the first six ticks

This costs zero lines in the menu — it reuses the existing `StatRow.tail`
field.

### 5.3 Kill from the App List

Each app row becomes actionable. Two gestures:

**Right-click (secondary click):** opens a submenu with one item:

```text
  Cursor  ⟶  Force Quit "Cursor"
  2.0 GB  12%
```

Selecting "Force Quit" sends `SIGTERM` to all PIDs grouped under that app.

**Cmd+K while the app row is highlighted:** same action, no submenu.

After killing, `rami` refreshes the app list immediately and shows the updated
rows.

If the kill fails (process already gone), the row simply disappears on the next
refresh. No error dialog, no alert.

Important safety guard: `rami` can only see and kill processes owned by the
same user, so the user cannot accidentally kill system daemons. This is
enforced by macOS, not by `rami`, so it is airtight.

### 5.4 Keyboard Shortcuts 1–5

Assign key equivalents `1` through `5` to the five app rows. Pressing a digit
while the menu is open sends `SIGTERM` to the corresponding app group.

The key equivalent appears right-aligned in the menu row, using the standard
system font:

```text
  Cursor           2.0 GB  12%   ⌘1
  Google Chrome    1.2 GB   7%   ⌘2
  Codex            0.9 GB   5%   ⌘3
  Granola          0.4 GB   2%   ⌘4
  WindowServer     0.2 GB   1%   ⌘5
```

The shortcut is `Cmd+digit` to match macOS convention for menu items and avoid
accidental triggers while typing. The key equivalent modifier mask is
`NSEventModifierFlags::Command`.

---

## 6. Data Definition

### 6.1 Trend History

Add to `AppState` in `src/app.rs`:

```rust
struct TrendState {
    prev_percent: u8,       // previous sample's used_percent
    prev_ticks_ago: u8,     // how many timer ticks since that sample
    baseline_percent: u8,   // sample from ~30s ago for delta
    baseline_ticks_ago: u8, // ticks since baseline was captured
}
```

`TrendState` is `Option<TrendState>`. It is `None` until the second sample
arrives.

Sampling logic:

```
on each timer tick:
    if trend_state is None:
        trend_state = Some(TrendState {
            prev_percent: current,
            prev_ticks_ago: 0,
            baseline_percent: current,
            baseline_ticks_ago: 0,
        })
    else:
        trend_state.prev_ticks_ago += 1
        trend_state.baseline_ticks_ago += 1
        if trend_state.baseline_ticks_ago >= 6:
            trend_state.baseline_percent = current
            trend_state.baseline_ticks_ago = 0
        // after computing trend:
        trend_state.prev_percent = current
        trend_state.prev_ticks_ago = 0
```

### 6.2 PID Mapping for Kill

Extend `ProcessMemoryRecord` in `src/process_memory.rs`:

```rust
struct ProcessMemoryRecord {
    pid: pid_t,             // already present
    group_key: String,      // already present
    display_name: String,   // already present
    footprint_bytes: u64,   // already present
    // NEW: keep PIDs grouped for killing
}
```

Extend `AppMemoryUsage`:

```rust
pub struct AppMemoryUsage {
    pub name: String,
    pub footprint_bytes: u64,
    pub pids: Vec<pid_t>,   // NEW
}
```

During `aggregate()`, collect PIDs alongside footprint sums. They are needed
for `kill(pid, SIGTERM)`.

### 6.3 Dropdown Model Extensions

Extend `AppSectionDisplay::Rows` and `StatRow`:

```rust
pub struct StatRow {
    pub primary: String,
    pub tail: Option<String>,
    pub key_equivalent: Option<String>,  // NEW: "1".."5"
    pub action_tag: Option<usize>,        // NEW: index for kill dispatch
}
```

Add a kill action selector to `RefreshTarget`:

```rust
#[unsafe(method(killAppAtIndex:))]
fn kill_app_at_index(&self, sender: &AnyObject)
```

The `sender` is the `NSMenuItem` that was clicked. Its `tag` carries the index
into the current app rows.

### 6.4 Trend and Delta Output

Extend `MemorySnapshot` and/or create a companion struct:

```rust
pub struct MemoryDisplay {
    pub snapshot: MemorySnapshot,
    pub trend_arrow: Option<TrendArrow>,  // None = stable
    pub delta_text: Option<String>,       // e.g. "+3% in 30s"
}

pub enum TrendArrow {
    Up,
    Down,
}
```

This keeps the model layer clean: `MemorySnapshot` stays pure data from the OS,
and `MemoryDisplay` carries the derived UI signals.

---

## 7. Implementation Phases

### Phase 1 — Trend History and Delta (Pure Logic)

File: `src/trend.rs` (new)

- `TrendTracker` struct that holds `prev_percent`, `baseline_percent`, and
  tick counters
- `fn record(&mut self, current_percent: u8) -> MemoryDisplay` — called every
  timer tick; returns trend arrow and optional delta text
- unit tests for: no arrow under 2pp change, up arrow at +2pp, down arrow at
  -2pp, delta appears after six ticks, delta empty under 1pp change

Wire into `AppState` in `src/app.rs`: one `RefCell<TrendTracker>`.

### Phase 2 — Trend Arrow in Menu Bar Icon

File: `src/tray.rs` (modify `set_gauge`)

- accept `Option<TrendArrow>` alongside percent and pressure
- when arrow is `Some`, draw a composite image: arrow symbol + gauge symbol
  side by side
- when arrow is `None`, draw only the gauge (current behavior)
- red tint logic unchanged

### Phase 3 — Delta in Dropdown

File: `src/format.rs` (modify `dropdown_model_with_apps`)

- accept `Option<String>` delta text
- append it to the Memory `StatRow.tail`
- existing tail becomes `"5.7 / 16.0 GB  (+3% in 30s)"`

### Phase 4 — PID Preservation for Kill

File: `src/process_memory.rs` (modify `aggregate`)

- `AppMemoryUsage` gains `pids: Vec<pid_t>`
- `aggregate` collects PIDs alongside footprint sums
- new function: `pub fn kill_app_group(usage: &AppMemoryUsage)` that iterates
  `pids` calling `libc::kill(pid, libc::SIGTERM)` and `libc::kill(pid,
  libc::SIGKILL)` as a fallback

Test: `kill_app_group` on a dummy PID returns the expected errno (ESRCH).

### Phase 5 — Kill from App Rows (UI)

File: `src/tray.rs` (modify app row creation)

- app rows get a target/action: `killAppAtIndex:` on `RefreshTarget`
- each row's `tag` is set to its index (0..4)
- key equivalent set to `NSString::from_str("1")` through
  `NSString::from_str("5")` with `NSEventModifierFlags::Command`

File: `src/app.rs` (modify `RefreshTarget`)

- `killAppAtIndex:` reads the sender's tag, looks up the `AppMemoryUsage` at
  that index from `app_memory`, calls `kill_app_group`, then triggers an
  immediate refresh of both memory and apps

### Phase 6 — Right-Click Submenu

File: `src/tray.rs`

- subclass or configure each app `NSMenuItem` to have a submenu
- submenu contains one item: `Force Quit "AppName"` targeting `killAppAtIndex:`
- the submenu item carries the same tag as its parent row

### Phase 7 — Docs and QA

- update `README.md` to describe the trend arrow, delta, and kill actions
- update the "Current scope" section if needed
- run `cargo test`, `./scripts/build-app.sh`, and manual QA

---

## 8. Refresh Behavior (Updated)

| Event | Global memory | Apps | Trend/delta |
|---|---|---|---|
| Timer tick (every 5s) | yes | if visible and 30s elapsed | yes (record and compute) |
| Manual Refresh (Cmd+R) | yes | if visible | yes |
| Toggle Show App Usage on | yes | yes (immediate) | yes |
| Toggle Show App Usage off | yes | clear rows | yes |
| Kill app (Cmd+1..5 or right-click) | yes | yes (immediate) | yes |
| App launch (first sample) | yes | if enabled | initialize tracker |

No change to the existing 30-second app scan cadence.

---

## 9. Error Handling

| Scenario | Behavior |
|---|---|
| `kill(pid, SIGTERM)` fails with ESRCH | process already dead; ignore, refresh will remove it |
| `kill(pid, SIGTERM)` fails with EPERM | process is root-owned; ignore silently (should not happen since we can't see root PIDs anyway) |
| Composite image drawing fails | fall back to gauge-only icon (no arrow) |
| Right-click on a non-app row (Loading/Unavailable) | no submenu; item is disabled |
| Keyboard shortcut on an empty row slot | no-op; the item has no target when disabled |

No alerts, no permission dialogs, no logging in release builds.

---

## 10. Tests

### `src/trend.rs` (new unit tests)

- first sample: no arrow, no delta
- stable (<2pp change): no arrow
- +2pp: `TrendArrow::Up`
- -2pp: `TrendArrow::Down`
- -100pp (ramp down from 100% to 0%): `TrendArrow::Down` (handles underflow)
- delta text empty until six ticks have elapsed
- delta text `"+3% in 30s"` when baseline and current differ by 3pp
- delta text empty when change < 1pp

### `src/process_memory.rs` (extend existing tests)

- `AppMemoryUsage` carries correct `pids` after aggregation
- `aggregate` collects PIDs when multiple processes share a group
- `kill_app_group` on invalid PID returns without panicking

### `src/format.rs` (extend existing tests)

- Memory row tail includes delta when present
- app rows carry `key_equivalent` and `action_tag`
- delta text absent when not provided

### `src/tray.rs` (extend existing tests)

- `MenuEntry` enriched to carry key equivalent and kill submenu presence
- loaded layout with kill actions verifies each app row has correct tag

### Existing tests

All existing tests must pass without modification to their assertions, since
the new fields default to `None`/empty and the feature is additive.

---

## 11. Verification

```sh
cargo test
cargo clippy -- -D warnings
./scripts/build-app.sh
open rami.app
```

Manual QA checklist:

- [ ] Menu bar shows gauge only on first launch (no arrow)
- [ ] After 5s, arrow appears if memory changed >= 2pp
- [ ] Open a memory-heavy app; watch arrow appear within 10s
- [ ] Close the app; watch downward arrow appear
- [ ] Memory delta appears in dropdown after ~30s
- [ ] Enable Show App Usage; app rows appear
- [ ] Press Cmd+1 while menu is open; top app is killed; rows refresh
- [ ] Right-click an app row; submenu shows "Force Quit"
- [ ] Select "Force Quit"; rows refresh
- [ ] Cmd+K on highlighted app row kills it
- [ ] Kill all app rows; menu still shows pressure/swap without crashing
- [ ] Red tint still works when memory pressure is High
- [ ] No Accessibility or Full Disk Access prompts

---

## 12. Acceptance Criteria

- **Trend arrow** appears in menu bar when memory changes by >= 2pp between
  consecutive 5-second samples; absent otherwise.
- **Trend arrow** is template-tinted (monochrome), never red.
- **Red gauge on High pressure** is unaffected by the arrow.
- **Delta text** appears in the Memory row tail after 30 seconds of runtime,
  showing signed percentage change against the sample from 30 seconds ago.
- **App rows are interactive**: every visible app row is a real `NSMenuItem`
  with a target and action.
- **Cmd+1..5** kills the corresponding app group from the dropdown.
- **Right-click** any app row shows a one-item submenu "Force Quit 'Name'".
- **Cmd+K** on a highlighted app row triggers the kill.
- **Immediate refresh** after any kill so the UI never shows stale rows.
- **No new runtime dependencies.** `libc` is already a dependency.
- **No new macOS permissions.** `kill` on own processes needs no entitlements.
- **`cargo test` and `cargo clippy` pass.**
- **`./scripts/build-app.sh` succeeds.**
- **README updated** to reflect the four new capabilities.

---

## 13. Out of Scope

- SIGKILL as the default action (SIGTERM is gentler and gives apps a chance to
  save state)
- killing individual PIDs instead of app groups
- "kill all apps" bulk action
- process restart/reopen after kill
- memory usage history graph or sparkline
- trend prediction ("memory will be full in X minutes")
- customizable keyboard shortcuts
- confirmation dialogs before kill
- undo for kills
- per-app CPU, network, or disk metrics
- any UI beyond the native `NSMenu`
