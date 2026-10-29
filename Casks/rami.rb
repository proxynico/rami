# Cask formula template for rami.
#
# Source of truth for the Homebrew Cask published at proxynico/homebrew-tap.
# After every v* tag the tap's copy gets this file with `version` and the
# published DMG's `sha256` filled in: by release.yml's update-tap job when the
# HOMEBREW_TAP_TOKEN secret is configured, by hand otherwise. See BUILDING.md.

cask "rami" do
  version "0.1.2"
  sha256 "REPLACE_WITH_SHA256_OF_DMG"

  url "https://github.com/proxynico/rami/releases/download/v#{version}/rami-#{version}.dmg"
  name "rami"
  desc "Restrained menu-bar system monitor for memory, CPU, and GPU"
  homepage "https://github.com/proxynico/rami"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "rami.app"

  zap trash: [
    "~/Library/Preferences/com.nicomontero.rami.plist",
    "~/Library/Saved Application State/com.nicomontero.rami.savedState",
  ]

  caveats <<~EOS
    rami is not notarized by Apple, so macOS blocks its first launch.
    Open rami and close the warning. Then open System Settings → Privacy &
    Security, click Open Anyway, confirm, and enter your Mac's password.
    Do this again after each update.
  EOS
end
