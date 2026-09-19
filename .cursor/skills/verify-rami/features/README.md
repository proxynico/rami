# rami verification map

This directory is the maintained source for verifying the user-facing behavior of rami. Read the index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- No rami process is running except an instance started by `control-rami launch` in this run.
- The helper has snapshotted `com.nicomontero.rami`.
- `control-rami doctor` prints `ok=yes` for the repo-local `rami.app` binary.
- Accessibility is granted to the process running the helper.
- Never drive `/Applications/rami.app` or any other instance.

## Driving conventions

- Start every recipe from the baseline unless its preconditions say otherwise.
- Use status-item and menu-item names from this map. The gauge lives on **menu bar 1**.
- Treat every command as literal. Keep quoted item titles unchanged.
- Open-and-inspect through `control-rami capture-open-menu`.
- Commands through `control-rami click` or `control-rami click-settings`.
- Restore defaults and pasteboard via `control-rami cleanup`. Do not remove proof artifacts.

## Proof and skip reporting

- Capture the user action and the resulting state, not only the final screen.
- UI proof includes an AX dump of the open menu and a screenshot with the gauge and dropdown visible.
- Mutation proof includes `defaults read com.nicomontero.rami` or `pbpaste` as the feature file specifies.
- Record the feature ID and entry point used with every artifact.
- Report an unreachable path with the attempted command and the unmet precondition.
- Do not report a skipped entry point as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then uses exactly four H2 sections in this order.

1. `Sub-features` lists short IDs with one line for each behavior.
2. `How to get to it (user POV)` lists every user entry point.
3. `Driving it with control-rami` starts with `Preconditions:` and uses labeled bullets that pair each user action with an exact command and observable result.
4. `Gotchas` lists traps that can waste or invalidate a verification run.

Keep implementation details out of the map. Name only user paths, stable handles, required state, commands, and observable proof.

## Features

- [Status gauge](./status-gauge.md) covers the menu-bar icon, its spoken label, and its tooltip.
- [Memory dropdown](./memory-dropdown.md) covers rings, history, breakdown, swap, and top apps.
- [Settings modules](./settings-modules.md) covers Show CPU, Show GPU, Show Apps, and Auto-Refresh.
- [CPU module](./cpu-module.md) covers the CPU heading, User/System/Idle, E-cores/P-cores, and busy processes.
- [Copy diagnostics](./copy-diagnostics.md) covers the pasteboard report from Settings.
