# Expand to Memory + CPU + GPU behind a single status item

rami began as a memory-only menu bar monitor. We decided to add CPU and GPU
modules, but concentrated entirely in the dropdown: there is exactly one
NSStatusItem, its icon remains the memory gauge, and the dropdown stays an
NSMenu with custom menu-item views (extending the sparkline pattern) rather
than a popover. Rejected: iStat-style multiple status items (triples the
status-item/settings surface and grows menu-bar footprint) and an
NSPopover panel (reimplements dismissal, positioning, and native menu feel
for a look-alike payoff). CPU and GPU sections are individually hideable in
Settings, so the original memory-only rami remains two toggles away.

Deliberate scope no-s that will look like omissions later: no CPU frequency
and no temperatures (both need private APIs or privileged helpers — against
the app's simplicity premise), no per-core rings (E/P cluster aggregates
only), no per-process GPU (no public API), and no quit action on the
CPU process list (only the memory list keeps it; killing a momentarily-hot
process invites data loss).
