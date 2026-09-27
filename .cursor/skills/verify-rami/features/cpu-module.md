# CPU module

When Show CPU is on, the dropdown adds one CPU row after the Memory section: User and System as detail and their busy sum as the value, e.g. `CPU  12 usr · 6 sys  18%`.

## Sub-features

- `cpu-row` shows one row titled `CPU` whose tail reads `<user> usr · <system> sys` and a percent.
- `cpu-hidden` removes the CPU row when Show CPU is off.

## How to get to it (user POV)

- Leave Show CPU checked (default on a never-configured Mac) and click the gauge.
- If CPU is hidden, open Settings and choose Show CPU.

## Driving it with control-rami

Preconditions:

- `control-rami doctor` prints `ok=yes`.
- Show CPU is on. If `defaults read com.nicomontero.rami showCpu` is `0`, run `control-rami click-settings --item "Show CPU"` first and confirm the key becomes `1`.

- **Open the menu.** Run `control-rami capture-open-menu --dump .cursor/skills/verify-rami/artifacts/cpu-module/menu.ax.txt --screenshot .cursor/skills/verify-rami/artifacts/cpu-module/menu.png`.
- **Row.** The dump contains a row titled `CPU` below the Memory section and above Refresh, with `usr`, `sys`, and a `%` value. `Loading…` is a valid first-open tail; Refresh and recapture before failing `cpu-row`.
- **Hide.** Run `control-rami click-settings --item "Show CPU"` so `showCpu` is `0`. Recapture to `cpu-hidden.ax.txt`. The dump has no `CPU` row.
- **Proof.** `menu.ax.txt` plus `menu.png` show the CPU row; `cpu-hidden.ax.txt` shows Memory still present without it.

## Gotchas

- This machine's stored default is often `showCpu=0`. Proving `cpu-row` without turning the module on is a skipped entry point, not a failure of the feature.
- User includes nice ticks, so the busy sum is User + System, not 100 − Idle to the percent.
- There are no per-core, per-cluster, or per-process CPU rows. Any of them is a regression.
- Cleanup restores the user's Show CPU value.
