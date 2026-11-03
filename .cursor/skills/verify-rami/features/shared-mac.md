# Shared Mac

With `sharedMacHost` set, the dropdown ends its data with a section for that Mac: its model, used / total and Memory %, a two-row strip of its 64 map cells, then Pressure and Swap, CPU and GPU.

## Sub-features

- `shared-reading` shows the section as an AXGroup labeled with the model, e.g. `Mac mini`, valued `<used> / <total> GB · <n>%, pressure <p>%, swap <s>, CPU <c>, GPU <g>`.
- `shared-offline` shows one row, the title with `Offline`, when the ask fails.
- `shared-hide` removes the section through Settings → Show Shared Mac.
- `shared-accent` turns the section red when the shared Mac's pressure is at Warning or Critical.

## How to get to it (user POV)

- Set `defaults write com.nicomontero.rami sharedMacHost <host>` and relaunch rami. The host runs rami with Share over Tailscale on and `tailscale serve` set.
- Click the gauge.

## Driving it with control-rami

Preconditions:

- `sharedMacHost` and the sharing Mac must be set before `launch`: rami reads the host once at startup. With one Mac, point it at itself: `defaults write com.nicomontero.rami shareOverTailscale -bool true` and `defaults write com.nicomontero.rami sharedMacHost <this Mac's Tailscale name>`, with `tailscale serve --tcp 47960` set.
- `curl -s -m 3 http://<host>:47960/` returns `rami-share 1`.

- **Read.** Run `control-rami capture-open-menu` into `.cursor/skills/verify-rami/artifacts/shared-mac/menu.ax.txt` and `menu.png`. The dump has the AXGroup above, whose figures match `curl` within a few seconds' drift. The screenshot shows the strip and the two rows between CPU/GPU and Settings.
- **Offline.** Turn Share over Tailscale off on the sharing Mac, wait one tick (5 s), and capture again. The dump has an AXStaticText `Mac mini` valued `Offline`.
- **Accent.** On the sharing Mac launch with `--force-pressure critical`. The AXGroup value reads `pressure 95%`, and the section draws red.
- **Hide.** Run `control-rami click-settings --item "Show Shared Mac"`. `showSharedMac` is `0` and the section is gone.

## Gotchas

- Cleanup restores the defaults snapshot from launch, which includes any `sharedMacHost` written before launch. Delete it afterwards if it was only for the run.
- `--force-pressure` also forces the sharing Mac's answer when both are the same app, so the whole dropdown turns red in a one-Mac run.
