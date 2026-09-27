# Concepts

rami is a macOS menu bar system monitor for memory, CPU, and GPU.
One memory gauge stays in the menu bar. The rest lives in the dropdown.

## Language

### Memory

**Memory %**:
Used memory as a share of total physical RAM. Used is App Memory + Wired +
Compressed, Activity Monitor's "Memory Used". Shown in the memory map header.
_Avoid_: usage, load

**Pressure**:
The kernel's view of memory scarcity: 100 − `kern.memorystatus_level`
(the jetsam "percent available" stat). Shown as the Pressure meter. Distinct from
Memory %. Pressure can spike while Memory % is flat, and vice versa.
_Avoid_: computing pressure from available/total (that proxy is only a fallback)

**App Memory**:
Anonymous (application-allocated) memory, per Activity Monitor's vocabulary.
A breakdown category.
_Avoid_: used, active

**Wired**:
Kernel-pinned memory that can never be paged out. Breakdown category.

**Compressed**:
Memory held by the compressor. Breakdown category.

**Cached**:
File-backed and purgeable pages, Activity Monitor's "Cached Files". macOS
reclaims them on demand. Breakdown category.

**Other**:
Physical RAM the kernel's page counts do not cover: total − used − Cached −
Free. Breakdown category, shown only when its share rounds to at least 1%.

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

**CPU row**:
One dropdown row. User and System (per `host_processor_info` ticks; nice
ticks fold into User) as detail, their busy sum as the value.
_Avoid_: load average, per-core rings, per-cluster or per-process CPU

### GPU

**GPU Utilization**:
`Device Utilization %` from the IORegistry `IOAccelerator` `PerformanceStatistics`
dictionary. The value of the GPU row, which hides itself if this read fails.

**Renderer / Tiler**:
`Renderer Utilization %` and `Tiler Utilization %` from the same dictionary.
Optional detail on the GPU row; each is omitted when its key is absent.
_Avoid_: GPU memory (`Alloc system memory`, `In use system memory`). Unified
memory makes those figures misleading.

### Presentation

**Module**:
One monitored subsystem (Memory, CPU, GPU). Memory is the dropdown's main
section; CPU and GPU are one row each, sharing a separator after it, and
can be hidden via Settings toggles.

**Accent**:
The single hue the whole dropdown inherits, driven by pressure state.
Under Normal, text uses the adaptive label color and marks use gold. Under
Warning and Critical, gold marks and accent chrome use system red; Other and
Free stay neutral gray. The user's
macOS accent color is deliberately ignored so a bright personal accent does
not dominate routine telemetry.

**Mark**:
Anything drawn in the accent hue rather than written: map cells, legend
swatches, the pressure meter, the history sparkline, the contact strip.

**Opacity ramp**:
How multi-category displays encode categories in one hue: stepped
opacities, plus a hatch and a neutral gray where a category needs one
(App Memory 100% / Wired 62% / Compressed 36% / Cached hatched / Other
gray / Free empty).
_Avoid_: multi-hue palettes (Activity Monitor colors)

**Memory map**:
The dropdown's hero. RAM drawn as 64 equal cells on a board, in breakdown
order, over a contact strip. One cell is total RAM / 64 (256 MB on 16 GB).
Used categories are solid, Cached is hatched, Other is gray, Free is empty.
_Avoid_: rings

**Breakdown legend**:
The monochrome list (App Memory / Wired / Compressed / Other / Cached / Free)
that partitions RAM. Its rows sum to the total, less any Other too small to
show.

**Status gauge**:
The single menu-bar icon: a RAM stick whose four chips fill left to right
with Memory %, over four legs. In normal pressure it remains a template image
so macOS renders it black or white for the current menu bar; Warning and
Critical pressure draw it red; severity between them is carried
numerically (pressure meter %, tooltip, VoiceOver), not by hue. RisingFast is
trend-driven at any pressure. When memory is climbing fast, the icon adds an
upward badge composite. There is exactly one status item regardless of how
many modules the dropdown shows.
