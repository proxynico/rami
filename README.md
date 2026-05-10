# rami

[![Release](https://img.shields.io/github/v/release/proxynico/rami?display_name=tag&sort=semver)](https://github.com/proxynico/rami/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%2014%2B-lightgrey)](#install)

A tiny memory monitor that lives in your Mac menu bar.

One little gauge tells you how full your RAM is. Click it and you'll see
exactly where it's going — total used, system pressure, swap, and the apps
hogging the most. Spot a runaway app climbing fast? It'll be glowing orange.
Click the menu and rami spells out the cue: `RAM rising` or
`RAM rising fast`, plus app rows show both the real memory jump and the
percentage jump. Quit a runaway app right from the menu.

That's it. No dock icon, no window, no fuss.

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
