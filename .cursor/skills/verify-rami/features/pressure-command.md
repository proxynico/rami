# Pressure command

rami runs the command stored in the `pressureCommand` default when a timer tick reads Pressure at Warning or Critical. It runs once, and again only after Pressure has read Normal for about a minute. With no command set, nothing runs.

## Sub-features

- `command-runs` runs the command once, within two ticks of launch under forced Warning.
- `command-once` does not run it again while Pressure stays at Warning.
- `command-unset` runs nothing when `pressureCommand` is not set.

## How to get to it (user POV)

- Run `defaults write com.nicomontero.rami pressureCommand '<command>'`, then launch rami. There is no menu row.

## Driving it with control-rami

Preconditions:

- No rami is running, and `defaults read com.nicomontero.rami pressureCommand` fails.
- `/tmp/rami-verify-pressure.log` does not exist.

- **Set.** Run `defaults write com.nicomontero.rami pressureCommand 'date >> /tmp/rami-verify-pressure.log'`.
- **Run.** Run `control-rami launch --force-pressure warning`. Within 15 s `/tmp/rami-verify-pressure.log` has one line.
- **Once.** Wait 30 s more. The file still has one line, and `capture-open-menu` shows Pressure `88 percent`.
- **Unset.** Run `control-rami cleanup`, then `defaults delete com.nicomontero.rami pressureCommand`, and delete the file. Run `control-rami launch --force-pressure warning` again. After 15 s the file does not exist.

## Gotchas

- `launch` snapshots the defaults after you set the command, so `cleanup` restores the command too. Delete the key after every cleanup, or the installed rami runs it at the next real Warning.
- The command is read at launch. Changing it needs a relaunch.
- rami launched by `open` has a bare `PATH`. Use full paths in the command.
- The run happens on the first timer tick, 5 s after launch, not at launch itself.
