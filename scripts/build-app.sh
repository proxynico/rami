#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="rami"
APP_DIR="$ROOT_DIR/$APP_NAME.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"
BINARY_PATH="$ROOT_DIR/target/release/$APP_NAME"
MACOSX_DEPLOYMENT_TARGET_VALUE="14.0"

if command -v cargo >/dev/null 2>&1; then
  cargo_bin="$(command -v cargo)"
elif command -v rustup >/dev/null 2>&1; then
  cargo_bin="$(rustup which cargo)"
else
  echo "error: cargo is not available on PATH" >&2
  exit 1
fi

toolchain_bin="$(dirname "$cargo_bin")"
MACOSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET_VALUE" PATH="$toolchain_bin:$PATH" "$cargo_bin" build --release --manifest-path "$ROOT_DIR/Cargo.toml"

rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$ROOT_DIR/macos/Info.plist" "$CONTENTS_DIR/Info.plist"
cp "$BINARY_PATH" "$MACOS_DIR/$APP_NAME"
chmod +x "$MACOS_DIR/$APP_NAME"

codesign --force --deep --sign - "$APP_DIR"

printf '%s\n' "$APP_DIR"
