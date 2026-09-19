# Settings modules

Settings in the dropdown hides or shows CPU, GPU, and the app list, and pauses Auto-Refresh. Each toggle persists across quit and relaunch.

## Sub-features

- `settings-open` reveals Auto-Refresh, Show Apps, Show CPU, Show GPU, Copy Diagnostics, and the disabled `rami <version>` row.
- `settings-cpu` shows or hides the CPU module.
- `settings-gpu` shows or hides the GPU module. GPU stays hidden if the IORegistry read fails even when the toggle is on.
- `settings-apps` shows or hides the top-apps rows under Memory.
- `settings-auto-refresh` checks or unchecks Auto-Refresh (pause/play icon follows the state).

## How to get to it (user POV)

- Click the gauge, then `Settings`.
- Choose `Show CPU`, `Show GPU`, `Show Apps`, or `Auto-Refresh`.

## Driving it with control-rami

Preconditions:

- `control-rami doctor` prints `ok=yes`.
- Launch already snapshotted defaults. Do not skip cleanup after this recipe.
- Do not click Launch at Login or Check for Updates.

- **Open Settings.** Run `control-rami capture-open-menu --dump .cursor/skills/verify-rami/artifacts/settings-modules/before.ax.txt --screenshot .cursor/skills/verify-rami/artifacts/settings-modules/before.png` so the closed-settings baseline is recorded, then `control-rami click --item "Settings"` is not required for the toggles — `click-settings` opens the submenu itself.
- **Toggle CPU on.** Run `control-rami click-settings --item "Show CPU"`. Run `defaults read com.nicomontero.rami showCpu`. For this step the value must be `1`.
- **See CPU.** Run `control-rami capture-open-menu --dump .cursor/skills/verify-rami/artifacts/settings-modules/cpu-on.ax.txt --screenshot .cursor/skills/verify-rami/artifacts/settings-modules/cpu-on.png`. The dump contains an AXHeading named `CPU`.
- **Toggle GPU on.** Run `control-rami click-settings --item "Show GPU"`. `defaults read com.nicomontero.rami showGpu` is `1`. Capture to `gpu-on.ax.txt` / `gpu-on.png`. If the dump contains `GPU`, the module loaded; if it does not, record that GPU data was unavailable — do not treat a missing heading as a toggle failure when `showGpu` is `1`.
- **Hide apps.** Run `control-rami click-settings --item "Show Apps"`. `defaults read com.nicomontero.rami showAppUsage` is `0`. Capture to `apps-off.ax.txt`. The dump still has Memory rings and has no app-name rows under the breakdown (loading/unavailable placeholders also count as the apps section — they must be gone).
- **Pause auto-refresh.** Run `control-rami click-settings --item "Auto-Refresh"`. `defaults read com.nicomontero.rami autoRefreshEnabled` is `0`.
- **Proof.** Keep the AX dumps and the `defaults read` transcripts in `.cursor/skills/verify-rami/artifacts/settings-modules/defaults.txt`. Cleanup must restore the pre-launch domain (this machine's daily driver currently stores `showCpu=0` and `showGpu=0` when those keys exist).

## Gotchas

- Defaults keys are `showCpu`, `showGpu`, `showAppUsage`, `autoRefreshEnabled`. Unset means the in-app default (CPU on, GPU off, Apps on, Auto-Refresh on), not "false".
- Toggling writes immediately. A crash before cleanup leaves the user's Settings dirty — always cleanup.
- GPU heading absent after a successful `showGpu=1` means the sampler returned nothing, not that the toggle failed.
- The menu closes and reopens after each toggle. Capture after that reopen, not during the click.
- Launch at Login is visible here and is off-limits.
