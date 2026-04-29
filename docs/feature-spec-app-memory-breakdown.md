# Feature Spec - App Memory Breakdown

## 1. Goal

Add an optional dropdown section that answers:

> Which apps are using the most memory right now, and what percent of total
> installed RAM does each one represent?

The menu bar stays exactly as it is: one quiet RAM gauge glyph. The new feature
only appears inside the native menu dropdown, and only when the user turns it
on.

## 2. Current App Shape

`rami` is a tiny Rust/AppKit accessory app. It currently has:

- one `MemorySampler` in `src/memory.rs`
- one `MemorySnapshot` in `src/model.rs`
- one 5-second `NSTimer` in `src/app.rs`
- one native `NSMenu` controlled by `src/tray.rs`
- formatted dropdown models in `src/format.rs`

The current dropdown shows memory, pressure, swap, refresh controls, launch at
login, and quit. This feature should extend that shape, not replace it with a
custom popover or full Activity Monitor clone.

## 3. Recommendation

Build this as a native `NSMenu` "Apps" section backed by macOS `libproc`
sampling.

Use `proc_listallpids`, `proc_pid_rusage`, `proc_pidpath`, and `proc_name` via
the existing `libc` dependency. Use each process's `ri_phys_footprint` as the
primary memory metric, group helper processes back to their outer `.app`, sort
by grouped footprint, and render the top five groups.

This is the right first version because it is fast, native, dependency-light,
and close to what Activity Monitor means by an app's memory footprint.

## 4. Alternatives Considered

### Option A - Shell out to `ps`

Run `ps`, parse RSS, and display the largest rows.

Reject. It is easy to prototype but wrong for the app. It launches another
process, reports RSS instead of footprint, makes grouping helper apps harder,
and would add parsing brittleness to a tiny native utility.

### Option B - Private Activity Monitor APIs

Try to match Activity Monitor exactly through private frameworks or undocumented
ledger data.

Reject. It is fragile, may break across macOS updates, and is too much surface
area for `rami`.

### Option C - Public `libproc` footprint sampling

Call public Darwin process APIs directly and keep the UI compact.

Choose this. It gives good enough per-app memory attribution without new
permissions, new dependencies, or a custom UI stack.

## 5. User Experience

Add a new action row:

```text
Show App Usage
```

It behaves like `Auto-Refresh`: checkmark on/off, no preferences window, no
separate settings panel.

When off, the dropdown stays close to today's shape:

```text
MEMORY
  53%   9.0 / 17.2 GB

PRESSURE
  Normal

SWAP
  1.2 GB
---------------------
Refresh
Auto-Refresh
Show App Usage
Launch at Login
---------------------
Quit
```

When on, insert an `Apps` section between `Memory` and `Pressure`:

```text
MEMORY
  53%   9.0 / 17.2 GB

APPS
  Cursor          2.0 GB  12%
  Google Chrome   1.2 GB   7%
  Codex           0.9 GB   5%
  Granola         0.4 GB   2%
  WindowServer    0.2 GB   1%

PRESSURE
  Normal

SWAP
  1.2 GB
---------------------
Refresh
Auto-Refresh
Show App Usage
Launch at Login
---------------------
Quit
```

Rows use the existing native menu styling:

- app name in primary label color
- memory and percent tail in secondary label color
- tabular digits for byte and percent values
- no icons in app rows for v1
- maximum five rows
- long app names truncated to 28 visible characters with `...`

The percent is percent of installed physical RAM:

```text
app_percent = app_footprint_bytes / total_memory_bytes * 100
```

The label should be compact. Render percent as whole numbers, except values
below 1% should display `<1%`.

## 6. Data Definition

Add a new module, `src/process_memory.rs`.

Public model:

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

Implementation-only record:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessMemoryRecord {
    pid: libc::pid_t,
    group_key: String,
    display_name: String,
    footprint_bytes: u64,
}
```

`group_key` is the stable grouping identity. For `.app` processes it is the
outermost app bundle path. For non-app processes it is the executable name.

## 7. Sampling Algorithm

1. Get all PIDs with `libc::proc_listallpids`.
2. For each PID:
   - call `libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V4, ...)`
   - read `ri_phys_footprint`
   - skip rows with zero footprint
   - call `libc::proc_pidpath` for the executable path
   - call `libc::proc_name` as a fallback name
3. Derive the app group:
   - if the executable path contains `.app/Contents/`, use the outermost
     `.app` component as the group key
   - display name is the outer `.app` file stem, for example `Cursor.app`
     becomes `Cursor`
   - otherwise group by executable name, for example `WindowServer`
4. Sum footprints by group.
5. Sort by `footprint_bytes` descending, then name ascending for stability.
6. Keep the top five rows.
7. Compute percent of installed RAM in the formatting layer from each row's
   `footprint_bytes` and the current `total_memory_bytes`.

Important: the app rows do not need to sum to the global `Memory` row. macOS
global memory accounting and per-process footprint accounting are different,
especially with shared pages, compression, and kernel memory.

## 8. Refresh Behavior

Keep the existing 5-second RAM refresh.

Process scanning is more expensive, so it gets a separate cadence:

- `Show App Usage` off: never scan processes
- toggled on: scan immediately
- manual `Refresh`: scan immediately when app usage is visible
- auto refresh: refresh app usage every 30 seconds while visible
- toggled off: clear rows and stop scanning

This preserves the "tiny monitor" feel. The user gets live-enough app data
without making `rami` churn every five seconds.

Initial implementation may scan synchronously on the main thread because it is
throttled and small. Acceptance requires measuring it. If a normal scan takes
over 50 ms on the target Mac, move process scanning behind a worker thread
before shipping.

## 9. UI State

Extend `DropdownModel` in `src/format.rs`:

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

pub enum AppSectionDisplay {
    Hidden,
    Loading,
    Rows(Vec<StatRow>),
    Unavailable,
}
```

Formatting rules:

- `footprint_bytes` uses existing `gb_text`
- `total_percent_tenths = 0..=9` renders `<1%`
- `total_percent_tenths = 10..` renders whole rounded percent, for example
  `12%`
- tail format is `"2.0 GB  12%"`

Extend `TrayController` in `src/tray.rs` with cached app section items:

- `apps_section: Retained<NSMenuItem>`
- `app_items: Vec<Retained<NSMenuItem>>`
- `show_app_usage_item: Retained<NSMenuItem>`
- `last_app_rows: RefCell<Option<AppSectionDisplay>>`
- `last_show_app_usage: Cell<bool>`

The menu rebuild only changes shape when app usage changes between hidden,
loading, unavailable, or row-count states. Stable updates should mutate
attributed titles in place like the existing memory, pressure, and swap rows.

## 10. App State Changes

Extend `AppState` in `src/app.rs`:

```rust
process_sampler: ProcessMemorySampler,
show_app_usage: Cell<bool>,
app_memory: RefCell<AppMemorySnapshot>,
ticks_until_app_refresh: Cell<u8>,
```

Add selector on `RefreshTarget`:

```rust
#[unsafe(method(toggleShowAppUsage:))]
fn toggle_show_app_usage(&self, _sender: &AnyObject)
```

Refresh rules:

- `refresh(true)` always refreshes global memory
- if `manual && show_app_usage`, refresh process memory
- if timer refresh and app usage is visible, refresh process memory only when
  `ticks_until_app_refresh == 0`
- after each process refresh, reset the process counter to six timer ticks

## 11. Error Handling

Per-process failures are expected because processes exit while sampling and
some system processes may reject introspection. Skip those processes.

Only show an unavailable state when the whole scan fails, for example
`proc_listallpids` returns an error or the PID list cannot be allocated.

Unavailable row:

```text
APPS
  Unavailable
```

No alerts, no permission prompts, no logs in release builds.

## 12. Tests

Add unit tests around pure logic first.

`src/process_memory.rs` tests:

- groups helper paths under the outer `.app`
- falls back to process name for non-app paths
- sums multiple helper processes into one app row
- sorts by footprint descending
- limits to five rows
- formats percent as `<1%` below one percent
- formats percent as whole rounded values at one percent and above

`tests/format_tests.rs`:

- hidden app section does not add menu entries
- loading app section renders one loading row under `Apps`
- app rows split name and tail correctly
- unavailable app section renders `Unavailable`

`src/tray.rs` test helpers:

- loaded layout without app usage matches today's sections plus
  `Show App Usage`
- loaded layout with app rows inserts `Apps` between `Memory` and `Pressure`
- `Show App Usage` action row reflects checked state

Existing tests must keep passing.

## 13. Verification

Required commands:

```sh
cargo test
./scripts/build-app.sh
open rami.app
```

Manual QA:

- confirm the menu bar icon is unchanged
- open dropdown with app usage off
- toggle `Show App Usage` on
- confirm top rows include expected heavy apps such as browsers, IDEs, or Codex
- confirm helper-heavy apps are grouped under the parent app name
- press `Refresh` and verify rows update without flicker
- pause auto-refresh and confirm manual refresh still updates app rows
- leave auto-refresh on for at least one minute and confirm app rows refresh
  about every 30 seconds
- confirm `rami` does not request Accessibility, Full Disk Access, or admin
  permissions

Performance QA:

- instrument one process scan with `std::time::Instant` in a debug-only block
- target: under 50 ms on the user's Mac
- if over 50 ms, move the scan to a worker thread before shipping

## 14. Acceptance Criteria

- Menu bar glyph remains unchanged.
- `Show App Usage` appears as a native action row.
- When off, no app memory scan runs.
- When on, the dropdown shows at most five top app groups.
- Rows show app name, footprint in GB, and percent of installed RAM.
- Browser/editor helper processes group under the parent `.app`.
- Non-app processes can still appear by executable name if they are top memory
  users.
- App usage refreshes immediately on toggle and manual refresh.
- App usage auto-refreshes no more often than every 30 seconds.
- Per-process sampling failures do not break the dropdown.
- Whole-scan failure renders one compact unavailable row.
- No shelling out to `ps`, `top`, or Activity Monitor.
- No new runtime dependencies unless implementation proves `libc` is missing a
  required binding.
- `cargo test` passes.
- `./scripts/build-app.sh` succeeds.

## 15. Out of Scope

- custom popover UI
- graphs or history
- per-tab browser memory
- CPU, network, disk, or energy metrics
- killing or opening apps from the dropdown
- app icons in the top-app rows
- persistent preferences
- exact reconciliation between app rows and the global RAM row

## 16. Implementation Phases

### Phase 1 - Process Memory Core

Create `src/process_memory.rs` with pure grouping and formatting helpers plus a
thin macOS sampler. Add focused tests before wiring UI.

### Phase 2 - Dropdown Model

Extend `src/format.rs` so `dropdown_model` can include hidden, loading, loaded,
and unavailable app section states.

### Phase 3 - Tray UI

Add the `Apps` section, row caching, and `Show App Usage` action to
`src/tray.rs`. Keep all menu rows native `NSMenuItem`s.

### Phase 4 - App State

Wire `ProcessMemorySampler` into `src/app.rs`, add the toggle selector, and
implement the 30-second app refresh cadence.

### Phase 5 - Docs and QA

Update `README.md` to describe the optional app usage section. Run tests, build
the app bundle, open the app, and manually verify live app grouping.
