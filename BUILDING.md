# Building & releasing rami

End-user install lives in [README.md](README.md). This is for contributors
and the maintainer.

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

CI (`.github/workflows/ci.yml`) runs these same checks, plus the ignored
syscall smoke test and `cargo audit`, on every push and pull request.

## Local app bundle

```sh
./scripts/build-app.sh
open rami.app
```

`build-app.sh` produces an ad-hoc-signed `rami.app` next to the repo. Fine
for local use; Gatekeeper will warn first launch (right-click → Open).

For a Developer ID-signed bundle (no Gatekeeper warning), set the identity:

```sh
RAMI_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  ./scripts/build-app.sh
```

## Notarized DMG

`scripts/release.sh` builds, signs, packages, notarizes, and staples a
distributable DMG to `dist/rami-<version>.dmg`.

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

Build:

```sh
RAMI_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  ./scripts/release.sh
```

Set `RAMI_SKIP_NOTARIZE=1` to dry-run without contacting Apple's notary
service.

## Release via GitHub Actions

`.github/workflows/release.yml` builds, signs, notarizes, and uploads a
DMG to a GitHub Release on every `v*` tag push. Required repo secrets:

| Secret | Purpose |
|---|---|
| `MACOS_CERTIFICATE_P12_BASE64` | base64 of the exported `.p12` (Developer ID cert + private key) |
| `MACOS_CERTIFICATE_P12_PASSWORD` | password used during the `.p12` export |
| `MACOS_SIGNING_IDENTITY` | full identity string |
| `MACOS_NOTARY_APPLE_ID` | Apple ID email |
| `MACOS_NOTARY_TEAM_ID` | Apple developer team ID |
| `MACOS_NOTARY_APP_PASSWORD` | app-specific password |
| `HOMEBREW_TAP_TOKEN` | (optional) PAT with write access to `proxynico/homebrew-tap`; enables the automatic Cask bump |

Export the `.p12` from Keychain Access (right-click cert → Export) and
encode with `base64 -i cert.p12 | pbcopy`. Workflow runs on `macos-14`, so
the DMG is `arm64`-only.

To cut a release:

```sh
git tag v0.1.1
git push origin v0.1.1
```

## Publishing the Homebrew Cask

`Casks/rami.rb` is the source-of-truth template. On every `v*` tag,
`release.yml`'s `update-tap` job renders it — filling in `version` and the
published DMG's `sha256` — and pushes it to `proxynico/homebrew-tap`, **if**
the `HOMEBREW_TAP_TOKEN` secret is set. One-time setup:

1. Create a `proxynico/homebrew-tap` repo on GitHub.
2. Add a `HOMEBREW_TAP_TOKEN` repo secret (fine-grained PAT with contents
   read/write on `homebrew-tap`).

Users then run:

```sh
brew install --cask proxynico/tap/rami
```

If `HOMEBREW_TAP_TOKEN` is unset the job no-ops. To render the Cask by hand,
set `version` to the tag and `sha256` from the release's `.sha256` asset
(adjust the `zap` paths only if `CFBundleIdentifier` ever changes from
`com.nicomontero.rami`), then commit it to the tap.

## Repo notes

- `scripts/build-app.sh` builds the release binary and assembles `rami.app`.
- `scripts/release.sh` builds the notarized DMG.
- `scripts/install.sh` is the `curl | bash` end-user installer (downloads
  the latest GitHub release DMG, verifies it against its published `.sha256`,
  copies into `/Applications`, ejects the DMG, leaves Gatekeeper quarantine
  handling intact, launches).
- `macos/Info.plist` configures accessory-app behavior with `LSUIElement`.
- `macos/rami.entitlements` disables the App Sandbox (needed for the
  cross-process memory scan); the hardened runtime is enabled by
  `codesign --options runtime` in `build-app.sh`, not by this file.
- `scripts/generate-icon.swift` draws the app icon and emits the `.icns`.
- `CFBundleShortVersionString` / `CFBundleVersion` are templated from
  `Cargo.toml` at build time.
