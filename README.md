# rami

`rami` is a tiny macOS menu bar app that shows current RAM usage as a single
SF Symbol gauge in the menu bar.

The goal is not to be a full system monitor. It is meant to stay lightweight,
stay out of the Dock, and answer one question quickly:

How much memory is this Mac using right now?

## What it does

- shows a template-tinted gauge glyph in the menu bar, bucketed across
  `0% / 33% / 50% / 67% / 100%`
- tints the glyph red when kernel memory pressure is `High`
- opens a plain native dropdown with:
  - `RAM: NN% — used of total`
  - `Memory Pressure: Normal | Elevated | High`
  - `Swap Used: N.N GB`
  - `Refresh`
  - `Pause Auto Refresh`
  - `Show App Usage` (optional, see below)
  - `Launch at Login`
  - `Quit`
- refreshes automatically every 5 seconds (toggle with `Pause Auto Refresh`)

## Show App Usage (optional)

Toggle `Show App Usage` to add an `Apps` section between Memory and Pressure
that lists the top five processes by physical memory footprint, grouped under
their parent `.app` bundle. Refreshes every 30 seconds while visible (and
immediately on `Refresh`).

Caveats worth knowing:

- macOS only lets a process inspect other processes owned by the same user,
  so root-owned daemons (`WindowServer`, `kernel_task`, `mds`, `launchd`)
  never appear. This is expected, not a bug.
- Per-process footprint includes compressed and swapped pages, so the app
  rows can sum to more than the global Memory percentage. Activity Monitor
  reports the same way.
- `rami` filters its own pid out of the list.

## Current scope

This is intentionally still a tiny menu bar utility.

- no CPU temperature
- no graphs
- no custom popover UI
- no settings panel

## Platform

- macOS 14+
- Apple Silicon tested
- written in Rust with native AppKit bindings

## Run in development

```sh
cargo test
cargo run
```

## Build the app bundle

```sh
./scripts/build-app.sh
open rami.app
```

The build script creates a signed local `rami.app` bundle that launches as a
menu bar utility without a Dock icon. The built app bundle also carries the
generated app icon and enables the `Launch at Login` menu item.

## How it works

`rami` reads macOS VM statistics and computes used RAM from:

- active memory
- wired memory
- compressed memory

It then rounds that to an integer percentage for the menu bar and shows the
used/total values in the dropdown.

To keep the app simple and cheap to run:

- sampling happens on a 5-second timer
- the UI stays native and small
- second launches exit quietly instead of opening duplicate menu bar items

## Repo notes

- `scripts/build-app.sh` builds the release binary and assembles `rami.app`
- `macos/Info.plist` configures accessory-app behavior with `LSUIElement`
- `scripts/generate-icon.swift` draws the app icon and emits the `.icns` file used by the bundle build
- the app bundle target is aligned to macOS 14.0
