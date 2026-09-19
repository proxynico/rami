# Copy diagnostics

Settings → Copy Diagnostics puts a local text report on the pasteboard so the user can paste rami's version, bundle path, memory snapshot, and top apps.

## Sub-features

- `diag-copy` replaces the pasteboard with a report that starts with `rami diagnostics`.
- `diag-identity` includes `Version:`, `Bundle ID: com.nicomontero.rami`, and `Process path:` pointing at the running binary.
- `diag-memory` includes Memory, Pressure, App Memory, Wired, Compressed, Free, and Swap lines.

## How to get to it (user POV)

- Click the gauge, open Settings, choose Copy Diagnostics, then paste into any text field.

## Driving it with control-rami

Preconditions:

- `control-rami doctor` prints `ok=yes`.
- The pasteboard may hold user secrets. Snapshot it first.

- **Save the current pasteboard.** Run `control-rami snapshot-pasteboard`.
- **Copy.** Run `control-rami click-settings --item "Copy Diagnostics"`.
- **Read.** Run `pbpaste > .cursor/skills/verify-rami/artifacts/copy-diagnostics/report.txt`.
- **Identity.** The file starts with `rami diagnostics` and contains `Version:` equal to `cargo_version` from doctor, `Bundle ID: com.nicomontero.rami`, and a `Process path:` that is the repo-local `rami.app/Contents/MacOS/rami`.
- **Memory.** The file contains `Memory:`, `Pressure:`, `App Memory:`, `Wired:`, `Compressed:`, `Free:`, and `Swap:`.
- **Proof.** Keep `report.txt`. Then `control-rami restore-pasteboard` (cleanup also restores).

## Gotchas

- This overwrites the system pasteboard. Always snapshot/restore. The report file is the evidence, not the live pasteboard after cleanup.
- `Available` appears in the report and is not shown in the dropdown breakdown. That is expected.
- Launch-at-login status in the report is a label (`Disabled` / `Enabled` / …), not a reason to click the menu item.
- Do not use `Check for Updates` as a substitute path.
