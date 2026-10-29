<p align="center">
  <img src="assets/rami-icon.png" width="128" height="128" alt="rami icon: a memory stick with three of its four chips lit">
</p>

<h1 align="center">rami</h1>

<p align="center"><b>A restrained system monitor that lives in your Mac menu bar.</b></p>

<p align="center">
  <a href="https://github.com/proxynico/rami/releases/latest"><img src="https://img.shields.io/github/v/release/proxynico/rami?display_name=tag&sort=semver" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <a href="#install"><img src="https://img.shields.io/badge/platform-macOS%2014%2B-lightgrey" alt="Platform: macOS 14+"></a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/readme-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/readme-light.png">
    <img src="assets/readme-light.png" width="350" alt="rami's menu open under its memory-stick icon: a map of 16 GB of RAM with its breakdown, the pressure meter, swap, a two-minute history, the three apps using the most memory, and CPU and GPU rows">
  </picture>
</p>

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

rami needs an Apple Silicon Mac with macOS 14 or later.

1. Download `rami-<version>.dmg` from the
   [latest release](https://github.com/proxynico/rami/releases/latest).
2. Open it and drag rami into Applications.
3. Open rami. It is not notarized by Apple, so macOS blocks the first launch.
   Close the warning, then open System Settings → Privacy & Security, click
   Open Anyway, confirm, and enter your Mac's password.

Repeat step 3 after each update.

To build it yourself, see [BUILDING.md](BUILDING.md).

## License

[MIT](LICENSE).
