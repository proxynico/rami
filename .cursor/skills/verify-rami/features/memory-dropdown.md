# Memory dropdown

Clicking the gauge opens one native menu: Memory % and Pressure rings, a two-minute history row, the App Memory / Wired / Compressed / Free breakdown, optional Swap, and (when enabled) the top apps.

## Sub-features

- `memory-open` opens the dropdown from the gauge.
- `memory-rings` exposes an AXGroup named `Memory` whose value includes both Memory % and Pressure.
- `memory-history` exposes an AXGroup named `Memory history`.
- `memory-breakdown` lists App Memory, Wired, Compressed, and Free.
- `memory-swap` shows a Swap row only when swap is non-zero.
- `memory-apps` lists top apps when Show Apps is on, or omits them when it is off.

## How to get to it (user POV)

- Click the menu-bar gauge.
- Click `Refresh` in the open menu to resample without quitting.

## Driving it with control-rami

Preconditions:

- `control-rami doctor` prints `ok=yes`.
- Show Apps is on (default unless the snapshotted defaults disabled it). If `defaults read com.nicomontero.rami showAppUsage` is `0`, this recipe cannot prove `memory-apps` until Settings modules turns it back on.

- **Read the closed gauge.** Run `control-rami status-label`. Save it to `.cursor/skills/verify-rami/artifacts/memory-dropdown/before-label.txt`.
- **Open and capture.** Run `control-rami capture-open-menu --dump .cursor/skills/verify-rami/artifacts/memory-dropdown/menu.ax.txt --screenshot .cursor/skills/verify-rami/artifacts/memory-dropdown/menu.png`.
- **Rings.** The dump contains a row whose name or description is `Memory` and whose value matches `[0-9]+ percent, .*, pressure [0-9]+ percent`.
- **History.** The dump contains `Memory history`.
- **Breakdown.** The dump contains `App Memory`, `Wired`, `Compressed`, and `Free`.
- **Commands.** The dump contains `Refresh`, `Settings`, and `Quit`.
- **Resample.** Run `control-rami click --item "Refresh"`. After the menu reopens, run `capture-open-menu` again to `.cursor/skills/verify-rami/artifacts/memory-dropdown/after-refresh.ax.txt` and `after-refresh.png`. Rings still show Memory and Pressure.
- **Proof.** The screenshot shows the open dropdown under the gauge, and the AX dump lists rings, history, breakdown, and the three commands.

## Gotchas

- Opening the menu with raw AppleScript blocks until the menu closes. Use `capture-open-menu`, not a bare `open-menu` plus a later dump in the same shell.
- Swap is absent when swap used is 0. Missing Swap is not a failure; a Swap row when Activity Monitor also shows 0 is.
- App names are truncated to 16 characters. Assert the row exists, not the full process name.
- Custom views (rings, history) often have an empty menu-item title. Read the AXGroup label and value, not `name` alone.
- `RAMI_FORCE_PRESSURE=warning` makes Pressure 88%; `critical` makes it 95%. Memory % stays the real sample.
