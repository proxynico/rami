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
  - a `Memory` section grouping:
    - `NN%   used / total GB`
    - `Pressure   Normal | Elevated | High` (tail tinted orange/red when not Normal)
    - `Swap   N.N GB` (hidden when swap is zero)
  - an `Apps` section (optional, see below) with a likely-culprit sub-line
    and per-app rows
  - `Refresh`, `Auto-Refresh`
  - `Show App Usage`, `Launch at Login`
  - `Quit`
- refreshes automatically every 5 seconds (toggle with `Auto-Refresh`)

## Show App Usage (optional)

Toggle `Show App Usage` to add an `Apps` section between Memory and Pressure
that lists the top five processes by physical memory footprint, grouped under
their parent `.app` bundle. Refreshes every 30 seconds while visible (and
immediately on `Refresh`).

When an app's footprint is rising, `rami` prioritizes the risers over merely
large apps and can show a one-line culprit answer:

```text
Likely culprit: Zen +300 MB
Zen 0.7 GB  +300 MB
```

Rows for normal `.app` bundles expose a native submenu with `Quit <App Name>`.
This sends a graceful quit signal only. There is no force quit fallback.

When memory pressure becomes `High`, `rami` sends a cooldown-protected
notification with the top riser when available. During elevated or high memory
pressure, it prepares app usage in the background even if `Show App Usage` is
off, so the dropdown has an answer when you open it.

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
- no force quit

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

By default the bundle is ad-hoc signed (fine for local use). To build a
distribution-signed bundle with hardened runtime, set `RAMI_SIGNING_IDENTITY`
to a `Developer ID Application` identity from your keychain:

```sh
RAMI_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  ./scripts/build-app.sh
```

## Release a notarized DMG

`scripts/release.sh` builds, signs, packages, notarizes, and staples a
distributable DMG.

One-time setup:

1. Mint a `Developer ID Application` certificate from
   developer.apple.com → Certificates and install it in your login keychain.
2. Create an app-specific password at appleid.apple.com → Sign-In and Security.
3. Store credentials for `notarytool`:
   ```sh
   xcrun notarytool store-credentials rami-notary \
     --apple-id <your-apple-id> \
     --team-id <YOUR_TEAM_ID> \
     --password <app-specific-password>
   ```

Build a release:

```sh
RAMI_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  ./scripts/release.sh
```

Output lands at `dist/rami-<version>.dmg`, fully notarized and stapled. Set
`RAMI_SKIP_NOTARIZE=1` to dry-run the build + DMG flow without contacting
Apple's notary service.

## Release via GitHub Actions

`.github/workflows/release.yml` builds, signs, notarizes, and uploads a DMG to
a GitHub Release whenever a `v*` tag is pushed. Required repository secrets:

| Secret | Purpose |
|---|---|
| `MACOS_CERTIFICATE_P12_BASE64` | base64 of the exported `.p12` containing the Developer ID Application cert + private key |
| `MACOS_CERTIFICATE_P12_PASSWORD` | password used during the `.p12` export |
| `MACOS_SIGNING_IDENTITY` | full identity string, e.g. `Developer ID Application: Your Name (TEAMID)` |
| `MACOS_NOTARY_APPLE_ID` | Apple ID email |
| `MACOS_NOTARY_TEAM_ID` | Apple developer team ID |
| `MACOS_NOTARY_APP_PASSWORD` | app-specific password |

Export the `.p12` from Keychain Access (right-click the cert → Export) and
encode with `base64 -i cert.p12 | pbcopy`. The workflow runs on `macos-14`
(Apple Silicon), so the resulting DMG is `arm64`-only.

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
- `scripts/release.sh` builds a notarized, stapled DMG for distribution
- `macos/Info.plist` configures accessory-app behavior with `LSUIElement`
- `macos/rami.entitlements` carries the hardened-runtime entitlements used by `release.sh`
- `scripts/generate-icon.swift` draws the app icon and emits the `.icns` file used by the bundle build
- the app bundle target is aligned to macOS 14.0
- `CFBundleShortVersionString` and `CFBundleVersion` are templated from `Cargo.toml` at build time
