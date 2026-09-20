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
repo-local bundle. Do not set `RAMI_INSTALL=1` during that drive. That
flag kills any process named rami and replaces `/Applications/rami.app`.

To refresh the installed daily driver after you have checked it:

```sh
RAMI_INSTALL=1 ./scripts/build-app.sh
```

A bare `./scripts/build-app.sh` leaves a running installed app stale.
When you use `RAMI_INSTALL=1`, confirm that the script reports the
terminated PIDs, the new binary mtime, and the relaunch.

## Decisions

### One status item

There is exactly one `NSStatusItem`. Its icon is the memory gauge. The
dropdown is an `NSMenu` with custom menu-item views, not an `NSPopover`.

CPU and GPU sections can each be hidden in Settings. The original
memory-only monitor is two toggles away.

These are out of scope: CPU frequency, temperatures, per-core rings,
per-process GPU, and a quit action on ranked app-memory or CPU-process
rows. rami is a monitor, not a task manager. E-cores and P-cores stay
as two cluster aggregates.

App-memory and CPU-process rankings show three rows each. Memory
history is one row, memory-only, about 28 px, under the rings. It
reuses the 5 s trend window the engine already records, including
while the menu is closed, so it is warm when the menu opens. It
renders in the pressure-driven accent at 12% fill, 65% line, and a
100% now-dot. There is no second graph, no per-module history, and no
submenu or hover reveal.

Memory is the visual anchor. Settings is an arrowless command that
opens a compact submenu. Refresh and Quit show no key equivalents.

Rejected alternatives were multiple status items and an `NSPopover`
panel.

### Pressure-driven accent

Under Normal pressure, dropdown text, legends, and history stay
monochrome. They use `labelColor` at stepped opacities. Ring tracks
are quaternary gray.

Memory ring strokes are the calm exception. Under Normal they use
system orange. Under Warning and Critical they use system red. Orange
is reserved for those calm rings.

The accent is semantic. Neutral chrome and legend use the adaptive
label color. Warning and Critical use system red, including ring
strokes. The user's macOS accent color is ignored.

The normal status gauge stays an untinted template image so macOS
picks black or white for the menu bar. Warning and Critical tint the
gauge red. Severity between those two states is numeric (pressure
ring %, tooltip, VoiceOver), not a second hue. RisingFast is
trend-driven at any pressure. When memory climbs fast, the icon adds
an upward badge in `status_icon.rs`.

Rejected alternatives were multi-hue category palettes, the user's
macOS accent, and a fixed accent with pressure tint only on the gauge.

A future category or module must fit the opacity ramp. If a display
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

Warning and Critical accents are unreachable without exhausting real
memory. Use `RAMI_FORCE_PRESSURE` set to `warning`, `critical`, or `normal`,
as described in `BUILDING.md`.

Screenshots taken during UI runs may capture unrelated windows. Check
a capture before you attach it outside the repo.
