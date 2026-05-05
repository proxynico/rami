#!/usr/bin/env bash
#
# Build, sign, package, notarize, and staple a distributable rami DMG.
#
# Required environment:
#   RAMI_SIGNING_IDENTITY   The full string of a "Developer ID Application"
#                           identity in the login keychain. Find with:
#                             security find-identity -v -p codesigning
#                           Example: "Developer ID Application: Nico Montero (PT37C8HWC3)"
#
# Optional environment (notary credentials — pick ONE of two modes):
#
#   Mode A — keychain profile (recommended for local use):
#     RAMI_NOTARY_PROFILE   notarytool keychain profile name. Default: rami-notary.
#                           One-time setup:
#                             xcrun notarytool store-credentials rami-notary \
#                               --apple-id <apple-id> \
#                               --team-id PT37C8HWC3 \
#                               --password <app-specific-password>
#
#   Mode B — direct credentials (recommended for CI):
#     RAMI_NOTARY_APPLE_ID  Apple ID email.
#     RAMI_NOTARY_TEAM_ID   Apple developer team ID.
#     RAMI_NOTARY_PASSWORD  App-specific password.
#                           If all three are set, they take precedence over
#                           RAMI_NOTARY_PROFILE — no keychain setup needed.
#
#   RAMI_SKIP_NOTARIZE      If set to 1, build + sign + DMG only. Useful for
#                           dry-runs before notary credentials are configured.
#
# Output: dist/rami-<version>.dmg

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="rami"
APP_DIR="$ROOT_DIR/$APP_NAME.app"
DIST_DIR="$ROOT_DIR/dist"
NOTARY_PROFILE="${RAMI_NOTARY_PROFILE:-rami-notary}"

if [ -z "${RAMI_SIGNING_IDENTITY:-}" ]; then
  echo "error: RAMI_SIGNING_IDENTITY must be set to a Developer ID Application identity" >&2
  echo "       run: security find-identity -v -p codesigning" >&2
  exit 1
fi

case "$RAMI_SIGNING_IDENTITY" in
  "Developer ID Application:"*) ;;
  *)
    echo "error: RAMI_SIGNING_IDENTITY must be a 'Developer ID Application' identity" >&2
    echo "       got: $RAMI_SIGNING_IDENTITY" >&2
    echo "       Apple Development / Mac Developer certs cannot be notarized." >&2
    exit 1
    ;;
esac

VERSION="$(awk -F'"' '/^version[[:space:]]*=/ { print $2; exit }' "$ROOT_DIR/Cargo.toml")"
if [ -z "$VERSION" ]; then
  echo "error: could not read version from Cargo.toml" >&2
  exit 1
fi

DMG_PATH="$DIST_DIR/$APP_NAME-$VERSION.dmg"

echo "==> Building and signing $APP_NAME $VERSION"
RAMI_SIGNING_IDENTITY="$RAMI_SIGNING_IDENTITY" "$ROOT_DIR/scripts/build-app.sh" >/dev/null

echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$APP_DIR"
spctl --assess --type execute --verbose=2 "$APP_DIR" || {
  echo "warning: spctl assessment failed before notarization (this is normal pre-notarize)"
}

echo "==> Building DMG at $DMG_PATH"
mkdir -p "$DIST_DIR"
rm -f "$DMG_PATH"

STAGE_DIR="$(mktemp -d -t rami-dmg-stage)"
trap 'rm -rf "$STAGE_DIR"' EXIT
cp -R "$APP_DIR" "$STAGE_DIR/"
ln -s /Applications "$STAGE_DIR/Applications"

hdiutil create \
  -volname "$APP_NAME $VERSION" \
  -srcfolder "$STAGE_DIR" \
  -ov \
  -format UDZO \
  "$DMG_PATH" >/dev/null

echo "==> Signing DMG"
codesign --force --sign "$RAMI_SIGNING_IDENTITY" --timestamp "$DMG_PATH"

if [ "${RAMI_SKIP_NOTARIZE:-0}" = "1" ]; then
  echo "==> Skipping notarization (RAMI_SKIP_NOTARIZE=1)"
  echo "$DMG_PATH"
  exit 0
fi

if [ -n "${RAMI_NOTARY_APPLE_ID:-}" ] && [ -n "${RAMI_NOTARY_TEAM_ID:-}" ] && [ -n "${RAMI_NOTARY_PASSWORD:-}" ]; then
  echo "==> Submitting DMG to Apple notary service (direct credentials)"
  xcrun notarytool submit "$DMG_PATH" \
    --apple-id "$RAMI_NOTARY_APPLE_ID" \
    --team-id "$RAMI_NOTARY_TEAM_ID" \
    --password "$RAMI_NOTARY_PASSWORD" \
    --wait
else
  echo "==> Submitting DMG to Apple notary service (profile: $NOTARY_PROFILE)"
  xcrun notarytool submit "$DMG_PATH" \
    --keychain-profile "$NOTARY_PROFILE" \
    --wait
fi

echo "==> Stapling notarization ticket"
xcrun stapler staple "$DMG_PATH"
xcrun stapler validate "$DMG_PATH"

echo "==> Final Gatekeeper assessment"
spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG_PATH"

echo
echo "Done: $DMG_PATH"
