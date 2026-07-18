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

running_pids() {
  pgrep -x "$APP_NAME" 2>/dev/null || true
}

process_binary_path() {
  ps -o comm= -p "$1" 2>/dev/null | awk 'NR == 1 { sub(/^[[:space:]]+/, ""); print; exit }' || true
}

binary_mtime() {
  local binary_path="$1"
  local mtime

  if [ ! -f "$binary_path" ]; then
    printf '%s' "unavailable (binary not found)"
    return
  fi

  if ! mtime="$(stat -f '%Sm' -t '%Y-%m-%d %H:%M:%S %z' "$binary_path" 2>/dev/null)"; then
    printf '%s' "unavailable"
    return
  fi

  printf '%s' "$mtime"
}

report_running_binaries() {
  local pids="$1"
  local pid
  local binary_path

  printf '%s\n' "$pids" | while IFS= read -r pid; do
    [ -n "$pid" ] || continue
    binary_path="$(process_binary_path "$pid")"
    if [ -n "$binary_path" ]; then
      printf '  PID %s: %s (mtime: %s)\n' "$pid" "$binary_path" "$(binary_mtime "$binary_path")"
    else
      printf '  PID %s: binary path and mtime unavailable\n' "$pid"
    fi
  done
}

wait_for_processes_to_exit() {
  local timeout_seconds="$1"
  local deadline=$(( $(date +%s) + timeout_seconds ))
  local pids

  while :; do
    pids="$(running_pids)"
    if [ -z "$pids" ]; then
      return 0
    fi
    if [ "$(date +%s)" -ge "$deadline" ]; then
      return 1
    fi
    sleep 0.2
  done
}

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
MACOSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET_VALUE" PATH="$toolchain_bin:$PATH" "$cargo_bin" build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml"
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

# Optionally install the freshly built bundle into /Applications and relaunch
# it, so the running app and the launch-at-login target stay in sync with this
# build. Enable with RAMI_INSTALL=1.
if [ "${RAMI_INSTALL:-0}" = "1" ]; then
  INSTALL_DIR="/Applications/$APP_NAME.app"
  INSTALLED_BINARY_PATH="$INSTALL_DIR/Contents/MacOS/$APP_NAME"
  RUNNING_PIDS="$(running_pids)"

  echo "==> Installing $APP_DIR -> $INSTALL_DIR"
  echo "==> Replacing $INSTALLED_BINARY_PATH (mtime: $(binary_mtime "$INSTALLED_BINARY_PATH"))"
  if [ -n "$RUNNING_PIDS" ]; then
    echo "==> Terminating running $APP_NAME PID(s): $RUNNING_PIDS"
    report_running_binaries "$RUNNING_PIDS"
    printf '%s\n' "$RUNNING_PIDS" | while IFS= read -r pid; do
      [ -n "$pid" ] || continue
      kill -TERM "$pid" 2>/dev/null || true
    done

    if ! wait_for_processes_to_exit 10; then
      echo "warning: $APP_NAME did not exit after SIGTERM; sending SIGKILL" >&2
      pkill -9 -x "$APP_NAME" 2>/dev/null || true
      if ! wait_for_processes_to_exit 2; then
        echo "error: $APP_NAME is still running; refusing to replace $INSTALL_DIR" >&2
        exit 1
      fi
    fi
  fi
  rm -rf "$INSTALL_DIR"
  cp -R "$APP_DIR" "$INSTALL_DIR"
  open "$INSTALL_DIR"
  echo "==> Replaced and launched $INSTALL_DIR"
elif [ -z "${RAMI_SIGNING_IDENTITY:-}" ]; then
  RUNNING_PIDS="$(running_pids)"
  if [ -n "$RUNNING_PIDS" ]; then
    {
      echo "warning: a new $APP_NAME build was produced, but it is NOT live."
      echo "warning: running $APP_NAME PID(s): $RUNNING_PIDS"
      report_running_binaries "$RUNNING_PIDS"
      echo "warning: make this build live with: RAMI_INSTALL=1 ./scripts/build-app.sh"
    } >&2
  fi
fi
