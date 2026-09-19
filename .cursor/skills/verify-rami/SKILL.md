---
name: verify-rami
description: Drive rami, the macOS menu-bar memory/CPU/GPU monitor, through its real NSStatusItem and NSMenu. Use when proving a user-visible change, checking a rebuilt bundle, or capturing dropdown evidence.
---

# Verify rami

rami is an accessory (`LSUIElement`) menu-bar app. The user touches one status-item gauge and the `NSMenu` it opens. There is no window, dock icon, browser, or HTTP port. Unit tests do not prove this surface.

Read `CONTEXT.md` before naming what you see. Memory % and Pressure are different rings. CPU and GPU are hideable modules.

## Isolation (read this first)

One process can hold `~/Library/Application Support/rami/rami.lock`. Settings persist to the real `com.nicomontero.rami` defaults domain. The menu bar shows one gauge.

**Refuse to drive any rami this run did not start.** If `/Applications/rami.app` or another checkout is already running, stop and say so. Do not `pkill rami`. Do not use `RAMI_INSTALL=1` (that script kills by process name and replaces the installed app). Do not override `HOME` to sneak a second instance onto the bar.

## Launch

From the repo root:

```sh
.cursor/skills/verify-rami/bin/control-rami launch
```

That rebuilds the repo-local `rami.app` with `./scripts/build-app.sh` (ad-hoc signed, not installed), snapshots `com.nicomontero.rami`, and `open`s `$REPO/rami.app` so LaunchServices owns the process. Ready when the status item's accessibility description matches `Memory <n> percent, …` (or `rami, memory unavailable` on a failed first sample).

Force an unreachable pressure accent (read once at startup):

```sh
.cursor/skills/verify-rami/bin/control-rami launch --force-pressure warning
.cursor/skills/verify-rami/bin/control-rami launch --force-pressure critical
.cursor/skills/verify-rami/bin/control-rami launch --force-pressure normal
```

`--skip-build` reuses an existing bundle. Only use it when doctor already confirmed `bundle_version` equals `Cargo.toml`.

State lives in `/tmp/rami-verify` (`pid`, defaults snapshot). Evidence lives in `.cursor/skills/verify-rami/artifacts/` and survives cleanup.

Teardown is `control-rami cleanup`. It SIGTERMs only the recorded repo-local PID, restores defaults, and leaves artifacts.

## Doctor

Run this first whenever anything looks off:

```sh
.cursor/skills/verify-rami/bin/control-rami doctor
```

It is read-only. Require `ok=yes`, `binary` equal to the repo-local `rami.app/Contents/MacOS/rami`, `bundle_version` equal to `cargo_version`, and a live `status_label`. If doctor fails because a foreign rami holds the lock, quit that app yourself (user action) and relaunch through this helper. If the status item never appears, grant Accessibility to the terminal or Cursor process running the helper.

## Drive

The harness is `.cursor/skills/verify-rami/bin/control-rami`. Prefer names over coordinates.

| User action | Command |
|---|---|
| Read the gauge without opening it | `control-rami status-label` |
| Open the dropdown and capture it | `control-rami capture-open-menu --dump FILE --screenshot FILE` |
| Choose a top-level command | `control-rami click --item "Refresh"` |
| Choose a Settings row | `control-rami click-settings --item "Show CPU"` |
| Dismiss the open menu | `control-rami dismiss-menu` |

Stable handles (do not invent others):

- Status item: accessibility description `Memory <n> percent, <used> of <total> GB used`; tooltip ` <n>% · <used> / <total> GB`. In System Events this is `menu bar item 1 of menu bar 1` of the recorded PID — **menu bar 1, not menu bar 2**.
- Rings view: AXGroup label `Memory`, value `<mem%> percent, <used> / <total> GB, pressure <p>% percent`.
- History view: AXGroup label `Memory history`.
- Module headings: AXHeading `CPU`, `GPU`.
- Command items by exact title: `Refresh`, `Settings`, `Quit`.
- Settings items by exact title: `Auto-Refresh`, `Show Apps`, `Show CPU`, `Show GPU`, `Copy Diagnostics`. About is `rami <version>` and is disabled.

`capture-open-menu` exists because clicking the status item blocks AppleScript while the menu stays open. It dumps AX, screenshots, then sends Escape.

**Do not click** `Launch at Login` (any suffix), or `Check for Updates`. The helper refuses those.

`click-settings --item "Show CPU"` (and Apps / GPU / Auto-Refresh) writes `NSUserDefaults`. Launch already snapshotted the domain; cleanup restores it. Still snapshot before a settings proof if you did not go through `launch`.

After a settings toggle the menu reopens on its own (`ScheduleMenuReopen`). Wait ~0.2s, then `capture-open-menu` or `dump-menu`.

## Evidence

Proof directory: `.cursor/skills/verify-rami/artifacts/<feature>/`.

Standards:

- Exercise the real status item and menu. Do not treat `cargo test` or engine fixtures as UI proof.
- Capture the action and the resulting state: status-label before open, AX dump + screenshot of the open menu, and a second read after a mutation.
- Side effects: settings keys via `defaults read com.nicomontero.rami`; diagnostics via the pasteboard (`pbpaste` must start with `rami diagnostics`).
- `RAMI_FORCE_PRESSURE` is a compiled-in preview hook, not a mock. It changes displayed pressure percent and the accent derived from it, nothing else. Warning is 88%, Critical is 95%. Confirm the rings AX value, not only the screenshot tint.
- Screenshots can include unrelated windows. Open the dump and the image before treating them as proof.

## Cleanup

```sh
.cursor/skills/verify-rami/bin/control-rami cleanup
```

Kills only the PID recorded at launch, and only if that PID's command is still the repo-local binary. Restores `com.nicomontero.rami` and a snapshotted pasteboard. Does not delete artifacts. Does not relaunch `/Applications/rami.app` — tell the user if they still want their daily driver back.

If a drive fails mid-run, cleanup anyway so the lock and defaults are not left dirty.

## Helpers

`bin/control-rami` is executable. Every recipe uses it as `.cursor/skills/verify-rami/bin/control-rami <command>` from the repo root (or via an absolute path). Commands: `launch`, `doctor`, `status-label`, `open-menu`, `dismiss-menu`, `dump-menu`, `capture-open-menu`, `click`, `click-settings`, `screenshot`, `snapshot-defaults`, `restore-defaults`, `snapshot-pasteboard`, `restore-pasteboard`, `cleanup`.

Read `features/README.md` and drive every listed entry point for the feature you claim to have proved.
