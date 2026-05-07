# Cask formula template for rami.
#
# This file is the source of truth for the Homebrew Cask published at
# proxynico/homebrew-tap. After cutting a new release, copy this file into
# that tap repo with:
#
#   - `version` updated to match the new git tag
#   - `sha256` set to the SHA-256 of the published DMG asset
#
# See BUILDING.md for the full release flow.

cask "rami" do
  version "0.1.0"
  sha256 "REPLACE_WITH_SHA256_OF_DMG"

  url "https://github.com/proxynico/rami/releases/download/v#{version}/rami-#{version}.dmg"
  name "rami"
  desc "Tiny macOS menu-bar memory monitor"
  homepage "https://github.com/proxynico/rami"

  depends_on macos: ">= :sonoma"
  depends_on arch: :arm64

  app "rami.app"

  zap trash: [
    "~/Library/Preferences/com.proxynico.rami.plist",
    "~/Library/Saved Application State/com.proxynico.rami.savedState",
  ]
end
