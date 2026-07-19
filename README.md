# rami

[![Release](https://img.shields.io/github/v/release/proxynico/rami?display_name=tag&sort=semver)](https://github.com/proxynico/rami/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%2014%2B-lightgrey)](#install)

A restrained system monitor that lives in your Mac menu bar.

One memory gauge stays in the menu bar. Click it to see Memory, CPU, and GPU
in one compact native menu: memory pressure and breakdown, a bounded history,
the apps using the most memory, CPU utilization and busy processes, and GPU
utilization when macOS exposes it. A small trend marker appears when memory is
climbing fast.

CPU, GPU, and app rows can each be hidden. There is still one status item, no
dock icon, and no window.

Settings also includes auto-refresh, launch-at-login status, and a local
diagnostics copy action.

## Install

Public install is not live yet. The Homebrew Cask and notarized DMG flow are
prepared, but they still need the Apple Developer signing/notarization setup
before release.

For now, build the local app bundle:

```sh
./scripts/build-app.sh
open rami.app
```

Release notes for the future DMG and Homebrew Cask live in
[BUILDING.md](BUILDING.md).

## License

[MIT](LICENSE).
