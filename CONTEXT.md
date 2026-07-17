# rami

A macOS menu bar system monitor (memory, CPU, GPU) with a single status item:
a memory gauge in the menu bar, everything else in the dropdown.

## Language

### Memory

**Memory %**:
Used memory as a share of total physical RAM. One of the two memory rings.
_Avoid_: usage, load

**Pressure**:
The kernel's view of memory scarcity: 100 − `kern.memorystatus_level`
(the jetsam "percent available" stat). The second memory ring. Distinct from
Memory % — pressure can spike while Memory % is flat, and vice versa.
_Avoid_: computing pressure from available/total (that proxy is only a fallback)

**App Memory**:
Anonymous (application-allocated) memory, per Activity Monitor's vocabulary.
One of the four breakdown categories.
_Avoid_: used, active

**Wired**:
Kernel-pinned memory that can never be paged out. Breakdown category.

**Compressed**:
Memory held by the compressor. Breakdown category.

**Free**:
Truly free page count, as Activity Monitor reports it. Breakdown category
shown in the UI.
_Avoid_: conflating with Available

**Available**:
The reclaimable pool (free + inactive + speculative + purgeable). Internal
concept used for the pressure fallback; not shown in the breakdown legend.

**Swap**:
Bytes of swap in use. Shown conditionally, only when non-zero.

### CPU

**User / System**:
The two-way CPU split (per `host_processor_info` ticks). Rendered with the
same legend component as the memory breakdown.

**E-cores / P-cores**:
Aggregate utilization per Apple Silicon core cluster (efficiency /
performance), shown as two rows — never per-core rings.

### GPU

**GPU Utilization**:
`Device Utilization %` from the IORegistry `IOAccelerator` node. The GPU
section hides itself entirely if this read fails.
_Avoid_: GPU memory (meaningless on unified memory)

### Presentation

**Module**:
One monitored subsystem (Memory, CPU, GPU). Each renders as one dropdown
section; CPU and GPU can be hidden via Settings toggles.

**Accent**:
The single hue the whole dropdown inherits, driven by pressure state:
calm = the user's macOS accent color, Warning = orange, Critical = red.
There is no separate fixed brand color.

**Opacity ramp**:
How multi-category displays encode categories in monochrome: one hue at
stepped opacities (e.g. App 100% / Wired 65% / Compressed 35% / Free 12%
gray; User 100% / System 50%).
_Avoid_: multi-hue palettes (Activity Monitor colors)

**Ring**:
A circular gauge menu-item view showing one percentage (Memory %, Pressure).

**Breakdown legend**:
The four-way colored list (App Memory / Wired / Compressed / Free) that
accounts for where RAM is going.

**Status gauge**:
The single menu-bar icon (SF Symbol gauge, pressure-tinted). There is exactly
one status item regardless of how many modules the dropdown shows.
