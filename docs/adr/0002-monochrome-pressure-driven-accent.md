# Monochrome visuals with a pressure-driven accent

The dropdown's rings, legends, and sparklines use a single hue instead of the
Activity Monitor / iStat multi-hue palette. Multi-category displays encode
categories as an opacity ramp of that hue (e.g. App 100% / Wired 65% /
Compressed 35% / Free 12% gray), and ring tracks are quaternary gray.

The accent itself is semantic, not a brand color: under normal memory
pressure it is the user's macOS accent color; under Warning it is orange;
under Critical it is red. This extends the status gauge's existing tint
language to the whole dropdown, so the app has one color system, not two.
Rejected: system semantic colors per category (blue/pink/yellow — familiar
from Activity Monitor but visually noisy and off-identity) and a fixed accent
with pressure tint only on the gauge (two competing color systems).

Consequence: any future category or module must fit the opacity ramp — if a
display genuinely cannot be read in one hue, that is a signal to simplify the
display, not to add a color.
