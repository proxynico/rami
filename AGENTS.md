# rami agent guide

rami is a macOS menu bar system monitor for memory, CPU, and GPU. It is
written in Rust and objc2. There is one status item. Everything else
renders in the NSMenu dropdown.

Read `CONCEPTS.md` before you name a domain term. The decisions below
bound product behavior. Read them before you change that behavior.

## Verify

```sh
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
./scripts/build-app.sh
```

See `BUILDING.md` for the bundle, signing, and release path. The ignored
integration tests (`cargo test -- --ignored`) are local-only.

To prove a user-visible change, use the `verify-rami` skill on the
repo-local bundle. It lives at `.cursor/skills/verify-rami/SKILL.md`.
Do not set `RAMI_INSTALL=1` during that drive. That flag kills any
process named rami and replaces `/Applications/rami.app`.

To refresh the installed daily driver after you have checked it:

```sh
RAMI_INSTALL=1 ./scripts/build-app.sh
```

A bare `./scripts/build-app.sh` leaves a running installed app stale.
When you use `RAMI_INSTALL=1`, confirm that the script reports the
terminated PIDs, the new binary mtime, and the relaunch.

## Decisions

### One status item

There is exactly one `NSStatusItem`. Its icon is the memory stick: four
chips that fill left to right with Memory %, drawn in `status_icon.rs`. The
dropdown is an `NSMenu` with custom menu-item views, not an `NSPopover`.

CPU and GPU are one row each after the Memory section. The CPU row
shows User and System with their busy sum. The GPU row shows Renderer
and Tiler when the driver reports them, with Device Utilization. Each
row can be hidden in Settings, so the original memory-only monitor is
two toggles away. Nico cut the E-core and P-core rows, the busy-process
list, and its sampler on 2026-09-27.

These are out of scope: CPU frequency, temperatures, per-core rings,
per-cluster or per-process CPU, per-process GPU, and a quit action on
ranked app-memory rows. rami is a monitor, not a task manager.

The Memory section opens the dropdown with the memory map: a header
with used / total and Memory %, RAM drawn as 64 equal cells on a board
in category order, a contact strip, and the cell size. The breakdown
rows follow, then the Pressure meter with ticks at Warning and
Critical, Swap when nonzero, and the history row.

The app-memory ranking shows three rows. Memory
history is one row, memory-only, about 36 px, at the end of the Memory
section. It reuses the 5 s trend window the engine already records,
including while the menu is closed, so it is warm when the menu opens.
It renders in the mark color at 12% fill, 65% line, and a 100%
now-dot. There is no second graph, no per-module history, and no
submenu or hover reveal.

Memory is the visual anchor. Settings is a standard submenu: hovering
it shows the settings beside the dropdown, which stays open. Nico
replaced the arrowless command on 2026-09-27 because it closed the
dropdown and left only the settings. Refresh and Quit show no key equivalents.

Rejected alternatives were multiple status items, an `NSPopover` panel,
and two equal rings for Memory % and Pressure. Nico replaced the rings
with the map on 2026-09-27: the rings gave both figures equal weight and
did not show where RAM goes.

### Pressure-driven accent

Under Normal pressure, dropdown text stays monochrome in the adaptive
label color. Every mark uses one orange hue: map cells, legend swatches,
the pressure meter, the history sparkline, and the contact strip. App
Memory, Wired, and Compressed step down at 100%, 62%, and 36%. Cached
is an orange hatch, Other is neutral gray, and Free is the empty track.
`mark_color` in `src/presentation.rs` picks light or dark orange while
drawing.

The accent is semantic. Warning and Critical turn the orange marks and the
accent chrome, including row labels, system red. Other and Free stay
neutral gray in every state. The user's macOS accent color is ignored.

The normal status gauge stays an untinted template image so macOS
picks black or white for the menu bar. Warning and Critical draw the
gauge in red; the button's content tint does not recolor a template image
in the menu bar. Severity between those two states is numeric (pressure
meter %, tooltip, VoiceOver), not a second hue. RisingFast is
trend-driven at any pressure. When memory climbs fast, the icon adds
an upward badge in `status_icon.rs`.

Rejected alternatives were multi-hue category palettes, the user's
macOS accent, a fixed accent with pressure tint only on the gauge, and
gray marks with orange rings. Nico chose gold on 2026-09-27 to match the
app icon's contacts, then orange on 2026-09-28. The app icon's contacts
stay gold.

A future category or module must fit the one-hue ramp. If a display
cannot be read in one hue, simplify the display.

When you change Neutral, Warning, or Critical colors, update this
decision, the Accent and Status gauge entries in `CONCEPTS.md`, and
the preview paragraph in `BUILDING.md` in the same change.

### Feature-complete

Decided 2026-07-19, at the close of the audit queue (#17–#26, PRs
#27–#35). The modules the app was asked to grow have shipped. rami
stops here.

Bug fixes, accuracy corrections, dependency updates, and build hygiene
stay in scope.

New features are closed by default. A proposal must argue why it
belongs in a single-glance menu bar monitor, against the one-status-item
bound and the visual restraint above, and Nico must accept it before
any implementation starts.

Do not slim the app down either. The full module set is user-hideable
in Settings. Repeat the runtime health check in `BUILDING.md` before
you use performance as a reason to remove behavior. Exact figures
without the machine, settings, interval, and sampled action are not
comparable.

### Launch at login

`SMAppService.mainAppService().status` is the only launch-at-login
status source. Do not invoke `sfltool` or any other diagnostic
subprocess. Settings reports registrations created through rami. It
does not detect a legacy login item added in System Settings.
`tests/no_diagnostic_subprocess.rs` fails the build if `sfltool`
returns to the sources.

## Process

Issues live on `proxynico/rami`. Use the `gh` CLI.

- `gh issue create --title "..." --body "..."`
- `gh issue view <number> --comments`
- `gh issue list --state open`
- `gh issue comment <number> --body "..."`
- `gh issue edit <number> --add-label "..."`
- `gh issue edit <number> --remove-label "..."`
- `gh issue close <number> --comment "..."`

Pull requests are not a request surface.

The triage labels are `needs-triage`, `needs-info`, `ready-for-agent`,
`ready-for-human`, and `wontfix`. Do not rename them.

One issue gets one branch and one pull request. Split a bundled issue
into several PRs. Do not merge two issues into one PR.

A ticket is workable when GitHub reports no open blockers:

```sh
gh api repos/proxynico/rami/issues/<n> --jq .issue_dependencies_summary.blocked_by
```

Nico audits the tracker, not the PRs. Escalate a PR only when it
changes agreed scope, contradicts a decision above, or the fix is far
larger than the ticket implied. If a small ticket takes hours or
touches many files, say so in the PR.

Issues you author that are already ticket-shaped go to implementation.
Issues from outside reporters take a triage label first.

## Public release

Public install is not live. `Cargo.toml` is `0.1.2`. Do not create or
move tag `v0.1.2` until purpose-specific Apple signing and notary
secrets are configured and a Release dry run has produced a notarized
DMG. Do not reuse the broad local `gh` token as `HOMEBREW_TAP_TOKEN`.

The remaining work is in `BUILDING.md`. Create `proxynico/homebrew-tap`
if it is absent, set the seven secret names, dry-run Release, then tag.

## Notes

The status item is `menu bar 1` in the accessibility tree, not
`menu bar 2`. Its accessibility label carries the live reading.

SF Symbol hierarchical tints and `labelColor.colorWithAlphaComponent`
bake the creation-time appearance. Resolve accent colors inside drawing
handlers (`imageWithSize_flipped_drawingHandler`): a tinted composite
drawn from inside a handler re-resolves per draw, but eager
rasterization or a tint baked at creation time does not, wherever the
image ends up. The dropdown's custom views resolve their colors inside
`drawRect:` for the same reason, through `color_for_accent_alpha` and
`mark_color` (`src/presentation.rs`). `src/status_icon.rs`,
`src/draw.rs`, and `src/presentation.rs` pin this with appearance-flip
regression tests.

Data rows (breakdown, Swap, apps, CPU, GPU) are `RowView`s
(`src/row_view.rs`), not titled menu items. Once any item has a submenu,
NSMenu reserves an arrow column on the right of every titled item, which
left titled rows short of the custom views' right edge. NSMenu on current
macOS also draws no menu-item images. Only Refresh, Settings, and Quit
stay titled items.

Warning and Critical accents are unreachable without exhausting real
memory. Use `RAMI_FORCE_PRESSURE` set to `warning`, `critical`, or `normal`,
as described in `BUILDING.md`.

Screenshots taken during UI runs may capture unrelated windows. Check
a capture before you attach it outside the repo.
