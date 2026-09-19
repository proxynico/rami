# Status gauge

The menu bar shows one memory gauge and no other rami icons. Hovering or asking VoiceOver reads the live Memory % and used/total GB without opening the dropdown.

## Sub-features

- `gauge-present` shows exactly one rami status item.
- `gauge-label` speaks `Memory <n> percent, <used> of <total> GB used`.
- `gauge-tooltip` shows `<n>% · <used> / <total> GB`.
- `gauge-pressure-preview` under `RAMI_FORCE_PRESSURE` still reports a Memory % from the real sample; only Pressure (inside the menu) is forced.

## How to get to it (user POV)

- Look at the menu bar for the single gauge icon.
- Hover the gauge to read the tooltip.
- Use VoiceOver / Accessibility Inspector on the gauge.

## Driving it with control-rami

Preconditions:

- `control-rami doctor` prints `ok=yes`.
- No other rami status item is on the bar.

- **Confirm identity.** Run `control-rami doctor`. `binary` ends with `rami.app/Contents/MacOS/rami` under this repo, not `/Applications/rami.app`.
- **Read the spoken label.** Run `control-rami status-label`. Stdout matches `Memory [0-9]+ percent, [0-9.]+ of [0-9.]+ GB used`.
- **Capture the bar.** Run `control-rami screenshot --path .cursor/skills/verify-rami/artifacts/status-gauge/menubar.png`. The image shows one gauge and no second rami icon.
- **Proof.** Write the doctor output and status-label to `.cursor/skills/verify-rami/artifacts/status-gauge/label.txt`. The file contains the same `Memory … percent` line as the live command.

## Gotchas

- The status item is `menu bar 1` of process `rami`. Clicking `menu bar 2` misses it.
- The label updates on the refresh timer. Do not assert a fixed percent across minutes.
- A screenshot of the bar without the label text is not enough; the spoken string is the proof.
- RisingFast adds a badge on the icon. That is trend, not a second status item.
