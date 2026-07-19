# Expand to Memory + CPU + GPU behind a single status item

rami began as a memory-only menu bar monitor. We decided to add CPU and GPU
modules, but concentrated entirely in the dropdown: there is exactly one
NSStatusItem, its icon remains the memory gauge, and the dropdown stays an
NSMenu with a custom menu-item view for the ring dashboard rather
than a popover. Rejected: iStat-style multiple status items (triples the
status-item/settings surface and grows menu-bar footprint) and an
NSPopover panel (reimplements dismissal, positioning, and native menu feel
for a look-alike payoff). CPU and GPU sections are individually hideable in
Settings, so the original memory-only rami remains two toggles away.

Deliberate scope non-goals that will look like omissions later: no CPU frequency
and no temperatures (both need private APIs or privileged helpers — against
the app's simplicity premise), no per-core rings (E/P cluster aggregates
only), no per-process GPU (no public API), and no quit action on the
ranked app-memory or CPU-process rows. rami is a monitor, not a task manager;
those rows stay informational instead of growing destructive submenus and
chevrons.

The dropdown is intentionally bounded: app-memory and CPU-process rankings
show three rows each, and the decorative history sparkline is omitted. The
full module set should remain usable without turning the native menu into a
screen-height dashboard.

**Amendment (2026-07, #26): a bounded memory-history row is restored.** The
omission above was written to stop the menu becoming a screen-height
dashboard, not to forbid a single 28 px row. One history row returns, inside
the Memory module beneath the rings, because it reuses the trend window the
engine already records every 5 s — including while the menu is closed — so it
adds no sampling work and is warm the moment the menu opens. It renders in
the pressure-driven Accent at an opacity ramp (12% fill, 65% line, 100% "now"
dot), obeying ADR-0002; the old sparkline's fixed blue does not return. The
bound that replaces the blanket omission is narrower and explicit: history is
memory-only and limited to this single row — no per-module histories, no
second graph, no submenu or hover reveal. "Single glance, not a dashboard"
is unchanged.

Memory remains the visual anchor. CPU and GPU section labels use fixed-width
centered views, while metric rows keep a consistent label/value grid. Settings
is an arrowless command that opens the existing controls in a separate compact
menu. Refresh and Quit intentionally have no displayed key equivalents. This
keeps the monitor dropdown free of disclosure and shortcut columns.
