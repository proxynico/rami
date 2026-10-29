# Building & releasing rami

End-user install lives in [README.md](README.md). This is for contributors
and the maintainer.

rami ships ad-hoc signed and not notarized, because there is no Apple
Developer Program membership. macOS blocks a downloaded copy's first launch
until the user clicks Open Anyway in System Settings → Privacy & Security,
and again after each update. Signing and notarizing switch on when the
Developer ID secrets below are set.

## Prereqs

- macOS 14+ (Apple Silicon)
- Rust stable
- Xcode command-line tools (`xcrun swift` is used to render the icon)

## Run from source

```sh
cargo test
cargo run
```

Before sending changes, run the same narrow checks used for the polish pass:

```sh
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

CI (`.github/workflows/ci.yml`) runs these same checks, plus `cargo audit`,
on every push and pull request. The ignored syscall smoke test
(`cargo test -- --ignored`) is local-only: it needs real `.app` processes,
which headless CI runners don't have.

## Local app bundle

```sh
./scripts/build-app.sh
open rami.app
```

`build-app.sh` produces an ad-hoc-signed `rami.app` next to the repo. Fine
for local use. A local build carries no quarantine flag, so Gatekeeper does not
check it.

For a Developer ID-signed bundle (no Gatekeeper warning), set the identity:

```sh
RAMI_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  ./scripts/build-app.sh
```

## Previewing the pressure accents

The dropdown inherits one accent from the current memory pressure: label-color
text and orange marks under Normal, alert red under Warning (>= 88%) and
Critical (>= 95%). Other and Free marks, labels, and readings stay neutral gray.
The status gauge and its rising-memory badge share full-strength ink: a single
untinted template under Normal and red under Warning/Critical.
Those elevated states are
unreachable in normal use without genuinely exhausting memory, so
`RAMI_FORCE_PRESSURE` overrides the reading:

```sh
RAMI_FORCE_PRESSURE=critical ./target/release/rami
RAMI_FORCE_PRESSURE=warning  ./target/release/rami
RAMI_FORCE_PRESSURE=normal   ./target/release/rami
```

Accepts those three names only, case-insensitively; anything else logs a warning
and is ignored. Read once at startup, so changing it needs a relaunch. It affects
the displayed pressure percent and the accent derived from it, nothing else.

This is a development hook, but it is compiled into release builds deliberately —
`build-app.sh` produces a release bundle, so gating it out would make it useless
for checking the accents in the app people actually run.

## Runtime health check

Measure the installed app instead of inferring cost from the source. With rami
running from `/Applications`, record one sample, leave the menu closed for a
minute, record another, then repeat around one menu open:

```sh
pid="$(pgrep -x rami | head -n 1)"
ps -o pid=,etime=,time=,%cpu=,rss=,command= -p "$pid"
```

`TIME` is cumulative CPU time, so its change between samples captures work that
instantaneous `%CPU` can miss. `RSS` is resident memory in KiB. Record the macOS
version, Mac model, enabled modules, Auto-Refresh setting, interval, and action
between samples with any result; without those conditions, exact figures are
not comparable.

## Release DMG

`scripts/release.sh` builds and packages `dist/rami-<version>.dmg` and its
`.sha256`. Without `RAMI_SIGNING_IDENTITY` the app is ad-hoc signed and the
DMG is not notarized. That is how rami ships:

```sh
./scripts/release.sh
```

### Signing and notarizing

This needs a paid Apple Developer Program membership. One-time setup:

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

Build:

```sh
RAMI_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  ./scripts/release.sh
```

Set `RAMI_SKIP_NOTARIZE=1` to dry-run without contacting Apple's notary
service.

## Release via GitHub Actions

`.github/workflows/release.yml` builds a DMG and uploads it to a GitHub
Release on every `v*` tag push. It signs and notarizes only when
`MACOS_CERTIFICATE_P12_BASE64` is set, and then it needs the other five
`MACOS_*` secrets too:

| Secret | Purpose |
|---|---|
| `MACOS_CERTIFICATE_P12_BASE64` | base64 of the exported `.p12` (Developer ID cert + private key) |
| `MACOS_CERTIFICATE_P12_PASSWORD` | password used during the `.p12` export |
| `MACOS_SIGNING_IDENTITY` | full identity string |
| `MACOS_NOTARY_APPLE_ID` | Apple ID email |
| `MACOS_NOTARY_TEAM_ID` | Apple developer team ID |
| `MACOS_NOTARY_APP_PASSWORD` | app-specific password |

Export the `.p12` from Keychain Access (right-click cert → Export) and
encode with `base64 -i cert.p12 | pbcopy`. Workflow runs on `macos-14`, so
the DMG is `arm64`-only.

To cut a release:

```sh
version="$(awk -F'"' '/^version[[:space:]]*=/ { print $2; exit }' Cargo.toml)"
if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$'; then
  echo "error: Cargo.toml did not yield a valid version" >&2
  exit 1
fi
git tag "v${version}"
git push origin "v${version}"
```

Confirm the derived tag matches the intended `Cargo.toml` version before
pushing it. A `v*` tag publishes a public release immediately.

## Repo notes

- `scripts/build-app.sh` builds the release binary and assembles `rami.app`.
- `scripts/release.sh` builds the release DMG.
- `scripts/install.sh` is the `curl | bash` end-user installer (downloads
  the latest GitHub release DMG, verifies it against its published `.sha256`,
  copies into `/Applications`, ejects the DMG, launches). curl sets no
  quarantine flag, so this copy skips the Open Anyway step.
- `macos/Info.plist` configures accessory-app behavior with `LSUIElement`.
- `macos/rami.entitlements` disables the App Sandbox (needed for the
  cross-process memory scan); the hardened runtime is enabled by
  `codesign --options runtime` in `build-app.sh`, not by this file.
- `scripts/generate-icon.swift` draws the app icon and emits the `.icns`.
- `CFBundleShortVersionString` / `CFBundleVersion` are templated from
  `Cargo.toml` at build time.
