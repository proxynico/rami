# Feature Plan - App Memory Breakdown

Implementation plan for `docs/feature-spec-app-memory-breakdown.md`. Encodes
the spec's intent plus the simplifications from the critique pass.

## 0. Departures From the Spec

These are intentional changes to the spec. Adopt them as-is.

1. **Public model holds raw bytes only.** Drop `total_percent_tenths` from
   `AppMemoryUsage`. The renderer computes `<1%` vs whole percent from
   `footprint_bytes / total_bytes` at format time. One unit, no decipercents.
2. **No `skipped_processes` on the snapshot.** Per-pid sampling failures get
   counted in a local `usize` inside the sampler and asserted in tests. The
   field is never displayed, so it does not belong on the public type.
3. **No `last_show_app_usage` cache on `TrayController`.** The
   hidden/visible distinction already lives in `AppSectionDisplay::Hidden`;
   diff against `last_app_section`.
4. **Reuse `StatRow` for app rows.** `StatRow { primary, tail: Option<String> }`
   already matches `name + " 2.0 GB  12%"`. No new `AppRowDisplay` type.
5. **Drop `WindowServer` from the §5 example.** `proc_pid_rusage` only
   succeeds for processes the user owns. Root daemons (`WindowServer`,
   `kernel_task`, `mds`, `launchd`) will be skipped on a normal user account.
   The mockup shouldn't imply otherwise.
6. **Unavailable row text is `Unavailable`.** No `Try Refresh` tail — the
   `Refresh` row sits two items below; restating it reads like a button.
7. **Acknowledge over-100% aggregate.** Add one line to §7's "important"
   note: `ri_phys_footprint` includes compressed and swapped pages, so
   summed app percents can exceed the global Memory percent. This is
   expected, not a bug.
8. **Empty rows collapse to Unavailable.** If `aggregate` returns an empty
   `Vec` (zero pids returned, or every pid skipped), the sampler returns
   `Err`. The sampler never emits `Loaded(vec![])`. One code path for
   "nothing to show" — avoids the "scanned and found nothing" UI confusion.
9. **Filter rami's own pid.** The sampler skips `libc::getpid()` before
   the rusage call. Removes the "rami: 0.0 GB  <1%" noise from rami's
   own dropdown. Two lines + one test.
10. **Replace `set_snapshot`, don't sibling.** Change the existing
    signature to take `&AppMemorySnapshot`. `set_placeholder` doesn't go
    through `set_snapshot` (it calls `set_gauge` + `apply_model` directly,
    `tray.rs:196-208`), so nothing else breaks. Avoids a method with no
    callers.

Everything else in the spec stands.

## 1. Architecture Overview

```
src/process_memory.rs      NEW   sampler + grouping + format helpers
src/format.rs              EDIT  AppSectionDisplay, dropdown_model_with_apps
src/tray.rs                EDIT  Apps section items, Show App Usage row
src/app.rs                 EDIT  ProcessMemorySampler, toggle, 30s cadence
tests/process_memory.rs    NEW   pure-logic tests
tests/format_app_section.rs NEW  app section rendering tests
```

Data flow:

```
NSTimer 5s ─► AppState::refresh(false)
                 ├─ MemorySampler::sample()             (every tick)
                 ├─ if show_app_usage && tick%6==0:
                 │     ProcessMemorySampler::sample()    (every 30s)
                 └─ TrayController::set_snapshot(...)
```

## 2. Phase 1 — Process Memory Core

**Files:** `src/process_memory.rs` (new), `tests/process_memory.rs` (new),
`src/lib.rs` (add `pub mod process_memory;`).

### 2.1 Public types

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMemoryUsage {
    pub name: String,
    pub footprint_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMemorySnapshot {
    Hidden,
    Loading,
    Loaded(Vec<AppMemoryUsage>),
    Unavailable,
}
```

No `skipped_processes`. No `total_percent_tenths`. Both are computed at
their point of use.

### 2.2 Internal record + grouping

```rust
#[derive(Debug, Clone)]
struct ProcessMemoryRecord {
    group_key: String,
    display_name: String,
    footprint_bytes: u64,
}

pub(crate) fn group_key_and_name(exec_path: &str, proc_name: &str)
    -> (String, String);
pub(crate) fn aggregate(records: Vec<ProcessMemoryRecord>, top_n: usize)
    -> Vec<AppMemoryUsage>;
```

`group_key_and_name` is pure and tested. Rules:

- if `exec_path` contains `.app/Contents/`, walk up to the outermost `.app`
  segment; `group_key` = that absolute path, `display_name` = file stem
  ("Cursor.app" → "Cursor")
- else `group_key` = `proc_name`, `display_name` = `proc_name`
- empty `proc_name` falls back to the basename of `exec_path`

`aggregate` sums by `group_key`, sorts by `(footprint_bytes desc, name asc)`,
truncates to `top_n`.

### 2.3 Sampler

```rust
pub struct ProcessMemorySampler {
    self_pid: libc::pid_t,
}

impl ProcessMemorySampler {
    pub fn new() -> Self {
        Self { self_pid: unsafe { libc::getpid() } }
    }
    pub fn sample(&self, top_n: usize)
        -> std::io::Result<Vec<AppMemoryUsage>>;
}
```

Internals:

1. `proc_listallpids` into a `Vec<libc::pid_t>` with one retry-on-grow.
   On error or 0 pids returned, return `Err`.
2. For each pid:
   - **skip if `pid == self.self_pid`** (rami should not appear in its own list)
   - `proc_pid_rusage(pid, RUSAGE_INFO_V4, &mut info)` — on `EPERM`/`ESRCH`,
     bump local `skipped` and continue.
   - skip if `info.ri_phys_footprint == 0`
   - `proc_pidpath(pid, buf, PROC_PIDPATHINFO_MAXSIZE)` — on failure, empty
     path
   - `proc_name(pid, buf, len)` — on failure, empty name
   - skip rows where both path and name are empty
3. Map → `ProcessMemoryRecord`, hand to `aggregate(records, top_n)`.
4. **If aggregated rows is empty, return `Err`** (covers the "all skipped"
   case). The caller turns `Err` into `AppMemorySnapshot::Unavailable`.
5. Otherwise return `Ok(rows)`. The skipped count is observed in tests via
   a `#[cfg(test)]` accessor; not exposed in release.

### 2.4 libc binding hazard

`libc::proc_pid_rusage` exists on macOS. `libc::rusage_info_v4` exists.
`RUSAGE_INFO_V4` the integer flavor constant has been missing in some libc
versions. Strategy:

```rust
#[cfg(target_os = "macos")]
const RUSAGE_INFO_V4: libc::c_int = 4;
```

Define locally even if libc exposes it — costs nothing, avoids a future
breakage. Verify the `rusage_info_v4` struct layout has `ri_phys_footprint`
as `u64`; if not, fall back to a hand-rolled `#[repr(C)]` struct.

### 2.5 Tests (`tests/process_memory.rs`)

Pure-logic only — the sampler itself isn't easy to assert against in CI.

- `groups_helper_under_outer_app`: input
  `/Applications/Google Chrome.app/Contents/Frameworks/.../Helper`,
  expect group key ending in `Google Chrome.app`, name `Google Chrome`
- `falls_back_to_proc_name_for_non_app`: `/usr/sbin/cfprefsd` + name
  `cfprefsd` → group `cfprefsd`, name `cfprefsd`
- `aggregate_sums_helpers`: 3 records same group, footprints 100/200/300 →
  one row, footprint 600
- `aggregate_sorts_desc_then_name_asc`: stable order across equal sizes
- `aggregate_truncates_to_top_n`: 7 groups, top_n=5 → 5 rows
- `empty_path_falls_back_to_proc_name`: no `.app/`, empty path, name set →
  group by name
- `empty_name_falls_back_to_path_basename`: path set, empty name → use
  basename
- **`aggregate_empty_input_returns_empty`**: `aggregate(vec![], 5) == vec![]`.
  Pins the contract that the sampler relies on for the "all skipped → Err"
  decision. (Sampler-level "empty → Err" gets a separate integration check
  in `tests/process_memory.rs` via a fake records vec.)
- **`group_key_first_app_segment_wins`**: input
  `/Applications/Outer.app/Contents/MacOS/Inner.app/Contents/MacOS/Bar`,
  expect group key ending in `Outer.app`, name `Outer`. Catches Squirrel-
  style nested `.app` updaters. The rule is "first `.app/Contents/`
  boundary from the left," not "outermost" (the spec wording was
  ambiguous; this test pins it).
- **`self_pid_is_filtered`**: construct sampler with `self_pid = 42`, feed
  records with pids `[1, 42, 99]`, assert the row for pid 42 is excluded
  before aggregation. (Refactor sampler to take an injectable record
  source for this test, or split the filter into a pure helper
  `filter_self(records, self_pid) -> Vec<_>` and test that directly.)

**Acceptance:** `cargo test --test process_memory` green. No UI yet.

## 3. Phase 2 — Dropdown Model

**Files:** `src/format.rs`.

### 3.1 New types

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSectionDisplay {
    Hidden,
    Loading,
    Rows(Vec<StatRow>),     // reuse StatRow
    Unavailable,
}
```

### 3.2 Extended `DropdownModel`

```rust
pub enum DropdownModel {
    Loading,
    Loaded {
        memory: StatRow,
        apps: AppSectionDisplay,
        pressure: PressureDisplay,
        swap: StatRow,
    },
}
```

### 3.3 Builders

```rust
pub fn dropdown_model(snapshot: MemorySnapshot) -> DropdownModel;
// keeps existing signature, returns apps: AppSectionDisplay::Hidden

pub fn dropdown_model_with_apps(
    snapshot: MemorySnapshot,
    apps: &AppMemorySnapshot,
) -> DropdownModel;
// maps AppMemorySnapshot → AppSectionDisplay using app_row()

fn app_row(app: &AppMemoryUsage, total_bytes: u64) -> StatRow {
    StatRow {
        primary: truncate_name(&app.name, 28),
        tail: Some(format!("{}  {}",
            gb_text(app.footprint_bytes),
            percent_label(app.footprint_bytes, total_bytes))),
    }
}
```

### 3.4 Pure helpers

```rust
fn percent_label(part: u64, total: u64) -> String {
    if total == 0 { return "—".into(); }
    let raw = part as f64 / total as f64 * 100.0;
    if raw < 1.0 { "<1%".into() }
    else { format!("{}%", raw.round() as u32) }
}

fn truncate_name(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars { return name.to_string(); }
    let mut out: String = name.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}
```

Use `…` (single char) not `...` so the count stays accurate.

### 3.5 Tests (`tests/format_app_section.rs`)

- `percent_label_below_one_renders_lt`: `5_000_000` of `1_000_000_000_000` →
  `<1%`
- `percent_label_at_one_renders_whole`: `10` of `1000` → `1%`
- `percent_label_rounds_to_nearest`: `127` of `1000` → `13%`
- `percent_label_handles_zero_total`: returns `—`, no panic
- `truncate_name_short_passthrough`
- `truncate_name_long_uses_ellipsis`
- `dropdown_model_default_apps_hidden`
- `dropdown_model_with_apps_loading`
- `dropdown_model_with_apps_unavailable`
- `dropdown_model_with_apps_rows_format`: builds tail
  `"2.0 GB  12%"` with two spaces

Update existing `loaded_layout_renders_three_sections_with_stat_rows` test
to assert `apps == AppSectionDisplay::Hidden` for the basic snapshot path.

**Acceptance:** `cargo test` green. No UI yet.

## 4. Phase 3 — Tray UI

**Files:** `src/tray.rs`.

### 4.1 New cached items

Add to `TrayController`:

```rust
apps_section: Retained<NSMenuItem>,           // "Apps" header
app_loading_item: Retained<NSMenuItem>,       // "Loading…" placeholder
app_unavailable_item: Retained<NSMenuItem>,   // "Unavailable"
app_items: Vec<Retained<NSMenuItem>>,         // pool of 5 reusable rows
show_app_usage_item: Retained<NSMenuItem>,    // action row w/ checkmark
last_app_section: RefCell<Option<AppSectionDisplay>>,
```

Pre-allocate `app_items` with 5 stat items at construction. Reuse them when
row count fits; the menu rebuild only re-attaches the visible prefix.

### 4.2 `MenuShape` extension

```rust
enum MenuShape {
    Uninitialized,
    Loading,
    LoadedNoApps,
    LoadedWithApps(AppShape),
}

enum AppShape { Loading, Unavailable, Rows(usize) }
```

`rebuild_menu` switches on shape, attaches the correct prefix of
`app_items`, and inserts the `apps_section` between memory and pressure.
The Show App Usage row is always present in the action block (between
Auto-Refresh and Launch at Login), regardless of shape.

### 4.3 `apply_model` extension

After updating the memory row:

```rust
match (&model.apps, last_app_section.borrow().as_ref()) {
    (new, Some(old)) if new == old => { /* skip */ }
    (AppSectionDisplay::Rows(rows), _) => {
        for (item, row) in self.app_items.iter().zip(rows) {
            item.setAttributedTitle(Some(&stat_row_attributed(
                row, NSColor::labelColor())));
        }
        // shape change handled by rebuild path
    }
    _ => { /* loading/hidden/unavailable handled by shape change */ }
}
*last_app_section.borrow_mut() = Some(model.apps.clone());
```

The shape transitions trigger rebuild; in-place title updates handle the
common case where row count is stable.

### 4.4 Show App Usage row

```rust
let show_app_usage_item = NSMenuItem::initWithTitle_action_keyEquivalent(
    NSMenuItem::alloc(mtm),
    &NSString::from_str("Show App Usage"),
    Some(sel!(toggleShowAppUsage:)),
    &empty,
);
show_app_usage_item.setTarget(Some(&refresh_target));
show_app_usage_item.setState(NSControlStateValueOff);
```

Ordered in `rebuild_menu`: Refresh → Auto-Refresh → Show App Usage →
Launch at Login.

### 4.5 Test helper updates

Extend `MenuEntry` with:

```rust
AppSectionHeader,
AppLoading,
AppUnavailable,
AppRow { primary: &'a str, tail: Option<&'a str> },
ShowAppUsage { enabled: bool },
```

Add tests:

- `loaded_with_apps_hidden_omits_apps_section`
- `loaded_with_apps_loading_renders_loading_row`
- `loaded_with_apps_unavailable_renders_one_row`
- `loaded_with_apps_rows_inserts_between_memory_and_pressure`
- `show_app_usage_present_in_action_block`
- `show_app_usage_state_reflects_toggle`

**Acceptance:** `cargo test` green. App still builds; no behavior change yet
because nothing wires the new types.

## 5. Phase 4 — App State

**Files:** `src/app.rs`.

### 5.1 `AppState` extensions

```rust
process_sampler: ProcessMemorySampler,
show_app_usage: Cell<bool>,                       // default false
app_memory: RefCell<AppMemorySnapshot>,           // default Hidden
ticks_until_app_refresh: Cell<u8>,                // 0 → refresh next tick
```

### 5.2 New selector

```rust
#[unsafe(method(toggleShowAppUsage:))]
fn toggle_show_app_usage(&self, _sender: &AnyObject) {
    let state = APP_STATE.with(|slot|
        slot.borrow().as_ref().and_then(Weak::upgrade));
    if let Some(state) = state { state.toggle_show_app_usage(); }
}
```

```rust
fn toggle_show_app_usage(&self) {
    let on = !self.show_app_usage.get();
    self.show_app_usage.set(on);
    if on {
        *self.app_memory.borrow_mut() = AppMemorySnapshot::Loading;
        self.ticks_until_app_refresh.set(0);
    } else {
        *self.app_memory.borrow_mut() = AppMemorySnapshot::Hidden;
    }
    self.refresh(true);
}
```

### 5.3 Refresh logic

```rust
fn refresh(&self, manual: bool) {
    if !manual && !self.auto_refresh_enabled.get() { return; }
    let mtm = MainThreadMarker::new().expect(...);

    if self.show_app_usage.get() {
        let should_scan = manual
            || self.ticks_until_app_refresh.get() == 0;
        if should_scan {
            self.sample_apps();
            self.ticks_until_app_refresh.set(6);  // 6 * 5s = 30s
        } else {
            self.ticks_until_app_refresh
                .set(self.ticks_until_app_refresh.get() - 1);
        }
    }

    if let Ok(snapshot) = self.sampler.sample() {
        let apps = self.app_memory.borrow().clone();
        self.tray.set_snapshot_with_apps(
            snapshot, &apps, ..., mtm);
    }
}

fn sample_apps(&self) {
    match self.process_sampler.sample(5) {
        Ok(rows) => *self.app_memory.borrow_mut() =
            AppMemorySnapshot::Loaded(rows),
        Err(_)  => *self.app_memory.borrow_mut() =
            AppMemorySnapshot::Unavailable,
    }
}
```

Per Departure #8, the sampler returns `Err` on both whole-scan failure AND
empty aggregation. `sample_apps` does not need to distinguish — both
collapse to `Unavailable`. One code path.

`TrayController::set_snapshot` is **modified in place** to take an
additional `apps: &AppMemorySnapshot` parameter and route through
`dropdown_model_with_apps`. `set_placeholder` does not call `set_snapshot`
(it goes straight through `set_gauge` + `apply_model`, `tray.rs:196-208`),
so the change has exactly one caller to update. No sibling method.

### 5.4 Sample-time instrumentation (debug only)

```rust
#[cfg(debug_assertions)]
{
    let t0 = std::time::Instant::now();
    self.process_sampler.sample(5).ok();
    eprintln!("[rami] proc scan: {:?}", t0.elapsed());
}
```

Per the napkin: drop these `eprintln!`s before merging. Use them only
during the perf check below.

**Acceptance:** App launches; toggling Show App Usage adds the section,
populates rows within one tick, refreshes ~every 30s.

## 6. Phase 5 — Verify and Document

### 6.1 Toolchain pre-check

Per the napkin: do not assume `cargo` is on PATH.

```sh
cargo --version || { echo "cargo missing — fix shims first"; exit 1; }
```

If shims are missing, restore from
`$(rustup which cargo)`/`rustc`/`rustdoc` into `~/.cargo/bin/`. Only then
run tests.

### 6.2 Probe SF Symbol availability (only if adding any)

This feature does not need new symbols, but if we change icons:

```sh
swift -e 'import AppKit; print(NSImage(systemSymbolName: "X", accessibilityDescription: nil) == nil ? "MISSING" : "ok")'
```

### 6.3 Required commands

```sh
cargo test
./scripts/build-app.sh
open rami.app
```

### 6.4 Manual QA checklist

- menu bar gauge unchanged
- dropdown with Show App Usage off matches previous shape + new toggle row
- toggle on → "Apps" appears between Memory and Pressure with at most 5 rows
- helper-heavy apps (Chrome, Cursor) collapse under the parent
- root-owned daemons do not appear (expected)
- `Refresh` updates app rows immediately; pause auto-refresh and verify
  manual still works
- leave running 60s with auto-refresh on → rows visibly update once
- toggle off → Apps disappears, no scans (verify via `sudo fs_usage -w
  -f filesys rami` or Activity Monitor's CPU% staying flat)
- no system permission prompts
- summed app percents may exceed Memory percent — that's expected

### 6.5 Performance check

In `sample_apps` debug build, log scan time. Target < 50ms on the user's
Mac. Typical scan of ~500 PIDs is 10–25ms. If > 50ms after warm-up:

- move `process_sampler.sample()` into `std::thread::spawn` with a
  `mpsc::channel`, drain results next tick
- defer to a follow-up; do not ship synchronous if it stutters

### 6.6 Docs

Update `README.md`: new "Show App Usage" toggle paragraph noting opt-in,
30s cadence, helper grouping, and the same-user limitation. No screenshot
refresh needed (menu bar unchanged).

## 7. Risk Register

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `RUSAGE_INFO_V4` constant missing in libc | Med | Define `const RUSAGE_INFO_V4: c_int = 4;` locally |
| `rusage_info_v4` struct ABI drift | Low | Hand-roll `#[repr(C)]` if libc layout shifts |
| Synchronous scan stutters menu open | Low | Debug instrumentation in §5.4; thread offload only if measured > 50ms |
| User confused by app% > memory% | Med | Out-of-scope per spec §15; covered by acknowledgment in §0 |
| `proc_pidpath` returns non-UTF8 | Very low | `String::from_utf8_lossy` and continue |
| Process exits mid-scan | High | Skip on `ESRCH`, no error surfaced |

## 8. Out of Scope (mirrors spec §15)

Custom popover, graphs, per-tab browser memory, CPU/network/disk/energy
metrics, killing apps, app icons in rows, persistent preferences,
reconciling app rows with the global Memory row.

## 9. Phase Ordering and Commit Plan

One commit per phase, each independently green:

1. `Add process memory sampler and grouping helpers` (Phase 1)
2. `Extend dropdown model with optional apps section` (Phase 2)
3. `Render apps section and Show App Usage row in tray` (Phase 3)
4. `Wire process sampler into app state with 30s cadence` (Phase 4)
5. `Document Show App Usage toggle in README` (Phase 5 docs)

Each commit must pass `cargo test` and `cargo clippy --all-targets
-- -D warnings`. Phases 1–3 are no-op behaviorally; only Phase 4 turns
the feature on.
