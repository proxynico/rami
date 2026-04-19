# rami

`rami` is a tiny macOS menu bar app that shows current RAM usage as a single
percentage in the menu bar.

The goal is not to be a full system monitor. It is meant to stay lightweight,
stay out of the Dock, and answer one question quickly:

How much memory is this Mac using right now?

## What it does

- shows RAM percentage in the menu bar
- keeps the label minimal: `53% ▣`
- opens a plain native dropdown with:
  - `RAM Used`
  - `RAM Total`
  - `Refresh`
  - `Quit`
- refreshes automatically every 5 seconds
- supports manual refresh from the menu
- runs as a single-instance accessory app

## Current scope

This is intentionally a RAM-only v1.

- no CPU temperature yet
- no graphs
- no custom popover UI
- no always-visible extra stats

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
menu bar utility without a Dock icon.

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
- the app bundle target is aligned to macOS 14.0
