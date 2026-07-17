# rami

[![Release](https://img.shields.io/github/v/release/proxynico/rami?display_name=tag&sort=semver)](https://github.com/proxynico/rami/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%2014%2B-lightgrey)](#install)

A tiny memory monitor that lives in your Mac menu bar.

One little gauge tells you how full your RAM is. Click it and you'll see
where it's going — total used, swap when there is any, and the apps hogging
the most. The top apps tag the orange jump in real bytes, so a runaway is
obvious at a glance without making the menu too wide. An orange arrow joins
the gauge when RAM is climbing fast.

That's it. No dock icon, no window, no fuss.

Settings includes launch-at-login status and a local diagnostics copy action.

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
