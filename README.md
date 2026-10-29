# rami

[![Release](https://img.shields.io/github/v/release/proxynico/rami?display_name=tag&sort=semver)](https://github.com/proxynico/rami/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%2014%2B-lightgrey)](#install)

A restrained system monitor that lives in your Mac menu bar.

One memory gauge stays in the menu bar. Click it to see Memory, CPU, and GPU
in one compact native menu: a memory map with its breakdown and pressure, a
bounded history, the apps using the most memory, and one-line CPU and GPU
readings (GPU when macOS exposes it). A small trend marker appears when memory is
climbing fast.

CPU, GPU, and app rows can each be hidden. There is still one status item, no
dock icon, and no window.

Settings also includes auto-refresh, launch-at-login status, and a local
diagnostics copy action.

## Install

```sh
brew install --cask proxynico/tap/rami
```

rami is not notarized by Apple, so macOS blocks its first launch. Open rami,
click Done, then open System Settings → Privacy & Security and click Open
Anyway. Do this again after each update.

To build it yourself, see [BUILDING.md](BUILDING.md).

## License

[MIT](LICENSE).
