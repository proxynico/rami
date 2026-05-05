#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="rami"
APP_DIR="$ROOT_DIR/$APP_NAME.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"
BINARY_PATH="$ROOT_DIR/target/release/$APP_NAME"
ICON_PATH="$ROOT_DIR/target/$APP_NAME.icns"
ENTITLEMENTS_PATH="$ROOT_DIR/macos/$APP_NAME.entitlements"
MACOSX_DEPLOYMENT_TARGET_VALUE="14.0"

if command -v cargo >/dev/null 2>&1; then
  cargo_bin="$(command -v cargo)"
elif command -v rustup >/dev/null 2>&1; then
  cargo_bin="$(rustup which cargo)"
else
  echo "error: cargo is not available on PATH" >&2
  exit 1
fi

VERSION="$(awk -F'"' '/^version[[:space:]]*=/ { print $2; exit }' "$ROOT_DIR/Cargo.toml")"
if [ -z "$VERSION" ]; then
  echo "error: could not read version from Cargo.toml" >&2
  exit 1
fi

toolchain_bin="$(dirname "$cargo_bin")"
MACOSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET_VALUE" PATH="$toolchain_bin:$PATH" "$cargo_bin" build --release --manifest-path "$ROOT_DIR/Cargo.toml"
xcrun swift "$ROOT_DIR/scripts/generate-icon.swift" "$ICON_PATH"

rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$ROOT_DIR/macos/Info.plist" "$CONTENTS_DIR/Info.plist"
plutil -replace CFBundleShortVersionString -string "$VERSION" "$CONTENTS_DIR/Info.plist"
plutil -replace CFBundleVersion -string "$VERSION" "$CONTENTS_DIR/Info.plist"
cp "$BINARY_PATH" "$MACOS_DIR/$APP_NAME"
cp "$ICON_PATH" "$RESOURCES_DIR/$APP_NAME.icns"
chmod +x "$MACOS_DIR/$APP_NAME"

# Sign. If RAMI_SIGNING_IDENTITY is set, sign for distribution (hardened runtime,
# entitlements, secure timestamp — required for notarization). Otherwise fall
# back to ad-hoc signing for local development.
if [ -n "${RAMI_SIGNING_IDENTITY:-}" ]; then
  if [ ! -f "$ENTITLEMENTS_PATH" ]; then
    echo "error: entitlements file missing at $ENTITLEMENTS_PATH" >&2
    exit 1
  fi
  codesign --force --options runtime \
    --entitlements "$ENTITLEMENTS_PATH" \
    --sign "$RAMI_SIGNING_IDENTITY" \
    --timestamp \
    "$APP_DIR"
else
  codesign --force --deep --sign - "$APP_DIR"
fi

printf '%s\n' "$APP_DIR"
