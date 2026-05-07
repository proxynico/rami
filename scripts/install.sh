#!/usr/bin/env bash
# rami one-liner installer.
#
#   curl -fsSL https://raw.githubusercontent.com/proxynico/rami/main/scripts/install.sh | bash
#
# Downloads the latest notarized DMG from GitHub Releases, copies rami.app
# into /Applications, ejects the DMG, and launches the app.

set -euo pipefail

REPO="proxynico/rami"
APP_NAME="rami"
INSTALL_DIR="/Applications"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "rami is macOS-only (you are on $(uname -s))." >&2
  exit 1
fi

if [ "$(uname -m)" != "arm64" ]; then
  echo "rami currently ships arm64-only builds (you are on $(uname -m))." >&2
  echo "Build from source instead: https://github.com/$REPO/blob/main/BUILDING.md" >&2
  exit 1
fi

tmpdir="$(mktemp -d -t rami-install)"
mount_point=""
cleanup() {
  if [ -n "$mount_point" ] && [ -d "$mount_point" ]; then
    hdiutil detach "$mount_point" -quiet >/dev/null 2>&1 || true
  fi
  rm -rf "$tmpdir"
}
trap cleanup EXIT

echo "==> Resolving latest release..."
api_url="https://api.github.com/repos/$REPO/releases/latest"
dmg_url="$(curl -fsSL "$api_url" \
  | grep -o '"browser_download_url": *"[^"]*\.dmg"' \
  | head -n1 \
  | sed 's/.*: *"\(.*\)"/\1/')"

if [ -z "$dmg_url" ]; then
  echo "Could not find a .dmg asset on the latest release." >&2
  echo "Visit https://github.com/$REPO/releases/latest to grab it manually." >&2
  exit 1
fi

dmg_path="$tmpdir/rami.dmg"
echo "==> Downloading $dmg_url"
curl -fL --progress-bar -o "$dmg_path" "$dmg_url"

echo "==> Mounting DMG"
mount_output="$(hdiutil attach "$dmg_path" -nobrowse -quiet -plist)"
mount_point="$(echo "$mount_output" \
  | /usr/libexec/PlistBuddy -c 'Print :system-entities' /dev/stdin 2>/dev/null \
  | grep -o '/Volumes/[^"]*' \
  | head -n1)"

if [ -z "$mount_point" ] || [ ! -d "$mount_point" ]; then
  # Fallback: parse without plist
  mount_point="$(hdiutil attach "$dmg_path" -nobrowse -quiet \
    | awk '/\/Volumes\// { for (i=1; i<=NF; i++) if ($i ~ /^\/Volumes\//) print $i }' \
    | head -n1)"
fi

if [ -z "$mount_point" ] || [ ! -d "$mount_point/$APP_NAME.app" ]; then
  echo "Could not locate $APP_NAME.app inside the mounted DMG." >&2
  exit 1
fi

if [ -d "$INSTALL_DIR/$APP_NAME.app" ]; then
  echo "==> Removing existing $INSTALL_DIR/$APP_NAME.app"
  rm -rf "$INSTALL_DIR/$APP_NAME.app"
fi

echo "==> Copying to $INSTALL_DIR"
cp -R "$mount_point/$APP_NAME.app" "$INSTALL_DIR/"

echo "==> Ejecting DMG"
hdiutil detach "$mount_point" -quiet
mount_point=""

echo "==> Launching $APP_NAME"
open "$INSTALL_DIR/$APP_NAME.app"

echo
echo "Done. rami lives in your menu bar."
