#!/usr/bin/env bash
#
# Build, package, and checksum the rami DMG.
#
# The app is ad-hoc signed and the DMG is not notarized. That is how rami
# ships: macOS blocks a downloaded copy's first launch until the user clicks
# Open Anyway in System Settings → Privacy & Security.
#
# Output: dist/rami-<version>.dmg and dist/rami-<version>.dmg.sha256

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="rami"
APP_DIR="$ROOT_DIR/$APP_NAME.app"
DIST_DIR="$ROOT_DIR/dist"

VERSION="$(awk -F'"' '/^version[[:space:]]*=/ { print $2; exit }' "$ROOT_DIR/Cargo.toml")"
if [ -z "$VERSION" ]; then
  echo "error: could not read version from Cargo.toml" >&2
  exit 1
fi

DMG_PATH="$DIST_DIR/$APP_NAME-$VERSION.dmg"

echo "==> Building $APP_NAME $VERSION"
"$ROOT_DIR/scripts/build-app.sh" >/dev/null

echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$APP_DIR"

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

(cd "$DIST_DIR" && shasum -a 256 "$(basename "$DMG_PATH")" > "$(basename "$DMG_PATH").sha256")
echo "==> Wrote $DMG_PATH.sha256"
echo "$DMG_PATH"
