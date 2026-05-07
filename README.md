# rami

[![Release](https://img.shields.io/github/v/release/proxynico/rami?display_name=tag&sort=semver)](https://github.com/proxynico/rami/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%2014%2B-lightgrey)](#install)

A tiny macOS menu-bar memory monitor. One glyph in your menu bar; a clean
dropdown when you click.

## What it does

- Live RAM usage as a single SF Symbol gauge in the menu bar.
- Click for the breakdown: used / total GB, percent, kernel pressure, swap.
- Top apps by memory, with risers highlighted in orange (`+39%`).
- Quit any app straight from the dropdown.
- Notifies once when pressure goes High, with the most likely culprit.

## Install

**Homebrew (recommended):**

```sh
brew install --cask proxynico/tap/rami
```

**Or one-liner:**

```sh
curl -fsSL https://raw.githubusercontent.com/proxynico/rami/main/scripts/install.sh | bash
```

The installer downloads the latest notarized DMG, copies `rami.app` to
`/Applications`, ejects the mounted image, and launches the app. It leaves
macOS Gatekeeper quarantine handling intact.

**Or grab the DMG** from the
[latest release](https://github.com/proxynico/rami/releases/latest), drag
`rami.app` into `/Applications`, and launch.

Open `Settings ▸ Launch at Login` if you want it to come back after reboot.

## Build from source

See [BUILDING.md](BUILDING.md).

## Troubleshooting

If rami cannot read memory/process data or toggle Launch at Login, it keeps the
menu bar app running and writes a short diagnostic to stderr. When debugging,
launch it from Terminal with `cargo run` or run the app binary directly from the
bundle to see those messages.

## License

[MIT](LICENSE).
