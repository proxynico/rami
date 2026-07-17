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

Memory remains the visual anchor. CPU and GPU section labels use fixed-width
centered views, while metric rows keep a consistent label/value grid. Settings
retains the only disclosure chevron because it is the only real submenu.
