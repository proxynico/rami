# Cask formula template for rami.
#
# Source of truth for the Homebrew Cask published at proxynico/homebrew-tap.
# On every v* tag, release.yml's update-tap job renders this file (filling in
# `version` and the published DMG's `sha256`) and pushes it to the tap when the
# HOMEBREW_TAP_TOKEN secret is configured. See BUILDING.md for setup and the
# manual fallback.

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
    "~/Library/Preferences/com.nicomontero.rami.plist",
    "~/Library/Saved Application State/com.nicomontero.rami.savedState",
  ]
end
