# CPU module

When Show CPU is on, the dropdown adds a CPU heading, User / System / Idle, E-cores and P-cores when the kernel exposes them, and a short list of busy processes.

## Sub-features

- `cpu-heading` shows an AXHeading named `CPU`.
- `cpu-legend` lists User, System, and Idle as percents that sum near 100.
- `cpu-cores` lists E-cores and/or P-cores when those cluster stats exist.
- `cpu-processes` lists busy process names with a percent tail.
- `cpu-hidden` removes the whole CPU section when Show CPU is off.

## How to get to it (user POV)

- Leave Show CPU checked (default on a never-configured Mac) and click the gauge.
- If CPU is hidden, open Settings and choose Show CPU.

## Driving it with control-rami

Preconditions:

- `control-rami doctor` prints `ok=yes`.
- Show CPU is on. If `defaults read com.nicomontero.rami showCpu` is `0`, run `control-rami click-settings --item "Show CPU"` first and confirm the key becomes `1`.

- **Open the menu.** Run `control-rami capture-open-menu --dump .cursor/skills/verify-rami/artifacts/cpu-module/menu.ax.txt --screenshot .cursor/skills/verify-rami/artifacts/cpu-module/menu.png`.
- **Heading.** The dump contains `CPU` as a heading or labeled group below the Memory block and above Settings.
- **Legend.** The dump contains `User`, `System`, and `Idle` with `%` values.
- **Cores.** If the dump contains `E-cores` or `P-cores`, both values are percents. Missing both is possible only if the sampler omitted cluster stats — record that; do not invent the rows.
- **Processes.** Process rows sit under the legend. Names may be truncated to 16 characters. A `Loading` or `Unavailable` row here is a valid first-open state; Refresh and recapture before failing `cpu-processes`.
- **Hide.** Run `control-rami click-settings --item "Show CPU"` so `showCpu` is `0`. Recapture to `cpu-hidden.ax.txt`. The dump has no `CPU` heading and no User/System/Idle block.
- **Proof.** `menu.ax.txt` plus `menu.png` show the CPU module; `cpu-hidden.ax.txt` shows Memory still present without CPU.

## Gotchas

- This machine's stored default is often `showCpu=0`. Proving `cpu-heading` without turning the module on is a skipped entry point, not a failure of the feature.
- User includes nice ticks. Do not expect User + System + Idle to be a textbook 100 on every sample, but all three rows must exist once CPU is Available.
- There are no per-core rings. Extra ring rows under CPU are a regression.
- Cleanup restores the user's Show CPU value.
