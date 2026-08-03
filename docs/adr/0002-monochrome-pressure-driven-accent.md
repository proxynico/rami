# Monochrome visuals with a pressure-driven accent

The dropdown's text, legends, and history stay monochrome under Normal
pressure: `labelColor` at stepped opacities (e.g. App 100% / Wired 65% /
Compressed 35% / Free 12% gray), and ring tracks are quaternary gray.

Memory **ring strokes** are the calm exception: under Normal pressure they use
system orange; under Warning and Critical they use system red. Orange is
reserved for calm rings — Warning no longer uses orange for accent chrome.

The accent itself is semantic, not a brand color: under normal memory pressure
it is the adaptive neutral label color for chrome and legend; under Warning and
Critical it is system red (including ring strokes). The normal status gauge
stays an untinted template image so macOS chooses black or white for the menu
bar appearance. Warning and Critical tint the gauge red; Critical still uses
the RisingFast badge when trend says so. This keeps routine telemetry quiet
and reserves color for pressure that needs attention.

Rejected: multi-hue category palettes (Activity Monitor colors — familiar but
visually noisy and off-identity), the user's macOS accent color, and a fixed
accent with pressure tint only on the gauge (two competing color systems).

Consequence: the user's macOS accent color does not enter the dashboard. Any
future category or module must fit the opacity ramp — if a display genuinely
cannot be read in one hue, that is a signal to simplify the display, not to add
a color.
