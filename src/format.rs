use crate::engine::REFRESH_TICK_SECONDS;
use crate::model::{
    classify_pressure, CpuModuleState, GpuModuleState, MemoryPressure, MemorySnapshot,
    SystemSnapshot,
};
use crate::share::SharedMac;

/// Binary gibibytes (1024³), matching Activity Monitor and marketed RAM sizes.
const ONE_GIB_BYTES: u64 = 1_073_741_824;
const ONE_MIB_BYTES: u64 = 1_048_576;

pub fn gb_text(bytes: u64) -> String {
    let gb = bytes as f64 / ONE_GIB_BYTES as f64;
    format!("{gb:.1} GB")
}

pub fn gb_pair(used_bytes: u64, total_bytes: u64) -> String {
    let used = used_bytes as f64 / ONE_GIB_BYTES as f64;
    let total = total_bytes as f64 / ONE_GIB_BYTES as f64;
    format!("{used:.1} / {total:.1} GB")
}

/// Hover tooltip for the menu-bar gauge, e.g. "47% · 7.2 / 16.0 GB".
pub fn gauge_tooltip(used_percent: u8, used_bytes: u64, total_bytes: u64) -> String {
    format!("{used_percent}% · {}", gb_pair(used_bytes, total_bytes))
}

/// VoiceOver label for the menu-bar gauge, e.g. "Memory 47 percent, 7.2 of 16.0 GB used".
pub fn gauge_accessibility_label(used_percent: u8, used_bytes: u64, total_bytes: u64) -> String {
    let used = used_bytes as f64 / ONE_GIB_BYTES as f64;
    let total = total_bytes as f64 / ONE_GIB_BYTES as f64;
    format!("Memory {used_percent} percent, {used:.1} of {total:.1} GB used")
}

pub fn mem_text(bytes: u64) -> String {
    if bytes >= ONE_GIB_BYTES {
        gb_text(bytes)
    } else {
        let mb = (bytes as f64 / ONE_MIB_BYTES as f64).round() as u64;
        format!("{mb} MB")
    }
}

/// Share of physical RAM, rounded to the nearest percent and capped at 100.
pub fn share_of_total(bytes: u64, total_bytes: u64) -> u8 {
    if total_bytes == 0 {
        return 0;
    }
    let percent = (u128::from(bytes) * 100 + u128::from(total_bytes) / 2) / u128::from(total_bytes);
    percent.min(100) as u8
}

/// Caption for the memory-history row: the span the samples cover and their
/// used-percent range, e.g. ("2 min", "71–75% used"). Samples are timer
/// ticks, so a window still filling after launch or a resume reads in
/// seconds. `None` while fewer than two samples are available.
pub fn history_caption(samples: &[u64], total_bytes: u64) -> Option<(String, String)> {
    if samples.len() < 2 {
        return None;
    }
    let seconds = (samples.len() - 1) as u64 * REFRESH_TICK_SECONDS as u64;
    let window = if seconds < 120 {
        format!("{seconds} s")
    } else {
        format!("{} min", seconds / 60)
    };
    let low = share_of_total(*samples.iter().min()?, total_bytes);
    let high = share_of_total(*samples.iter().max()?, total_bytes);
    let range = if low == high {
        format!("{high}% used")
    } else {
        format!("{low}–{high}% used")
    };
    Some((window, range))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatRow {
    pub primary: String,
    pub detail: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Accent {
    Neutral,
    Warning,
    Critical,
}

impl From<MemoryPressure> for Accent {
    fn from(pressure: MemoryPressure) -> Self {
        match pressure {
            MemoryPressure::Normal => Self::Neutral,
            MemoryPressure::Warning => Self::Warning,
            MemoryPressure::Critical => Self::Critical,
        }
    }
}

/// How a legend swatch or map cell is filled. Every category fits one hue:
/// accent steps, a neutral gray, an accent hatch, or the empty track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Swatch {
    Accent(u8),
    Neutral,
    Hatched,
    Empty,
}

/// One slice of physical RAM, in map and legend order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCategory {
    App,
    Wired,
    Compressed,
    Other,
    Cached,
    Free,
}

impl MemoryCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::App => "App Memory",
            Self::Wired => "Wired",
            Self::Compressed => "Compressed",
            Self::Other => "Other",
            Self::Cached => "Cached",
            Self::Free => "Free",
        }
    }

    pub fn bytes(self, memory: &MemorySnapshot) -> u64 {
        match self {
            Self::App => memory.app_memory_bytes,
            Self::Wired => memory.wired_bytes,
            Self::Compressed => memory.compressed_bytes,
            Self::Other => memory.other_bytes(),
            Self::Cached => memory.cached_bytes,
            Self::Free => memory.free_bytes,
        }
    }

    pub fn swatch(self) -> Swatch {
        match self {
            Self::App => Swatch::Accent(100),
            Self::Wired => Swatch::Accent(62),
            Self::Compressed => Swatch::Accent(36),
            Self::Other => Swatch::Neutral,
            Self::Cached => Swatch::Hatched,
            Self::Free => Swatch::Empty,
        }
    }
}

pub const MAP_CELLS: usize = 64;

/// The memory map: RAM drawn as `MAP_CELLS` equal cells in category order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMapDisplay {
    /// e.g. "12.7 / 16.0 GB"
    pub used_of_total: String,
    pub used_percent: u8,
    pub cells: Vec<MemoryCategory>,
    /// e.g. "1 cell = 256 MB"
    pub cell_caption: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendRow {
    pub label: String,
    pub value: String,
    pub swatch: Swatch,
    /// Row hierarchy (#23): totals render at full label strength, derived
    /// breakdowns are demoted. Brightness tracks actionability, not position.
    pub primary: bool,
}

/// Everything the loaded dropdown shows: the Memory section, then one row
/// each for CPU and GPU when they are shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownModel {
    pub accent: Accent,
    pub map: MemoryMapDisplay,
    pub pressure_percent: u8,
    pub total_bytes: u64,
    /// Trend-window samples for the one memory-history row, oldest first.
    pub history: Vec<u64>,
    /// One row per `LEGEND_CATEGORIES` entry. Other and Free are read from
    /// the map's gray and empty cells.
    pub breakdown: [LegendRow; LEGEND_CATEGORIES.len()],
    pub swap: Option<StatRow>,
    pub cpu: Option<StatRow>,
    pub gpu: Option<StatRow>,
    pub shared: SharedSection,
}

/// The shared Mac's section, after CPU and GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedSection {
    Hidden,
    /// One line: the title and "Loading…" or "Offline".
    Status {
        title: String,
        status: &'static str,
    },
    Reading(SharedMacDisplay),
}

/// A shared reading as the section draws it: header, a strip of the 64 map
/// cells, then Pressure and Swap, CPU and GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedMacDisplay {
    pub title: String,
    /// e.g. "10.8 / 16.0 GB · 67%"
    pub figures: String,
    pub cells: Vec<MemoryCategory>,
    /// The shared Mac's own pressure accent, independent of this Mac's.
    pub accent: Accent,
    pub pressure: String,
    pub swap: String,
    pub cpu: String,
    pub gpu: String,
}

pub(crate) fn dropdown_model(
    snapshot: SystemSnapshot,
    history: &[u64],
    shared: &SharedMac,
) -> DropdownModel {
    let memory = snapshot.memory;
    DropdownModel {
        accent: Accent::from(classify_pressure(memory.pressure_percent)),
        map: memory_map(&memory),
        pressure_percent: memory.pressure_percent,
        total_bytes: memory.total_bytes,
        history: history.to_vec(),
        breakdown: memory_breakdown(&memory),
        swap: (memory.swap_used_bytes > 0).then(|| StatRow {
            primary: "Swap".to_string(),
            detail: mem_text(memory.swap_used_bytes),
            value: None,
        }),
        cpu: cpu_row(snapshot.cpu),
        gpu: gpu_row(snapshot.gpu),
        shared: shared_section(shared),
    }
}

const SHARED_TITLE_FALLBACK: &str = "Shared Mac";

fn shared_section(shared: &SharedMac) -> SharedSection {
    let status = |title: &Option<String>, status| SharedSection::Status {
        title: title
            .as_deref()
            .unwrap_or(SHARED_TITLE_FALLBACK)
            .to_string(),
        status,
    };
    match shared {
        SharedMac::Hidden => SharedSection::Hidden,
        SharedMac::Loading { title } => status(title, "Loading…"),
        SharedMac::Offline { title } => status(title, "Offline"),
        SharedMac::Reading(reading) => {
            let memory = &reading.memory;
            let missing = || "–".to_string();
            SharedSection::Reading(SharedMacDisplay {
                title: reading.model.clone(),
                figures: format!(
                    "{} · {}%",
                    gb_pair(memory.used_bytes, memory.total_bytes),
                    memory.used_percent
                ),
                cells: allocate_cells(&memory_parts(memory), MAP_CELLS),
                accent: Accent::from(classify_pressure(memory.pressure_percent)),
                pressure: format!("{}%", memory.pressure_percent),
                swap: mem_text(memory.swap_used_bytes),
                cpu: reading.cpu.map_or_else(missing, |cpu| {
                    format!(
                        "{}%",
                        cpu.user_percent.saturating_add(cpu.system_percent).min(100)
                    )
                }),
                gpu: reading.gpu.map_or_else(missing, |gpu| {
                    format!("{}%", gpu.utilization_percent.min(100))
                }),
            })
        }
    }
}

/// RAM split into categories in map order. Other appears only when its share
/// rounds to at least 1%.
fn memory_parts(memory: &MemorySnapshot) -> Vec<(MemoryCategory, u64)> {
    let mut parts = vec![
        (MemoryCategory::App, memory.app_memory_bytes),
        (MemoryCategory::Wired, memory.wired_bytes),
        (MemoryCategory::Compressed, memory.compressed_bytes),
    ];
    let other = memory.other_bytes();
    if share_of_total(other, memory.total_bytes) >= 1 {
        parts.push((MemoryCategory::Other, other));
    }
    parts.push((MemoryCategory::Cached, memory.cached_bytes));
    parts.push((MemoryCategory::Free, memory.free_bytes));
    parts
}

/// The categories the breakdown legend names, in map order.
pub const LEGEND_CATEGORIES: [MemoryCategory; 4] = [
    MemoryCategory::App,
    MemoryCategory::Wired,
    MemoryCategory::Compressed,
    MemoryCategory::Cached,
];

fn memory_breakdown(memory: &MemorySnapshot) -> [LegendRow; LEGEND_CATEGORIES.len()] {
    LEGEND_CATEGORIES.map(|category| LegendRow {
        label: category.label().to_string(),
        value: mem_text(category.bytes(memory)),
        swatch: category.swatch(),
        primary: category == MemoryCategory::App,
    })
}

fn memory_map(memory: &MemorySnapshot) -> MemoryMapDisplay {
    MemoryMapDisplay {
        used_of_total: gb_pair(memory.used_bytes, memory.total_bytes),
        used_percent: memory.used_percent,
        cells: allocate_cells(&memory_parts(memory), MAP_CELLS),
        cell_caption: format!(
            "1 cell = {}",
            mem_text(memory.total_bytes / MAP_CELLS as u64)
        ),
    }
}

/// Largest-remainder apportionment, so the cells always number exactly
/// `count` and stay in category order.
fn allocate_cells(parts: &[(MemoryCategory, u64)], count: usize) -> Vec<MemoryCategory> {
    let sum: u128 = parts.iter().map(|(_, bytes)| u128::from(*bytes)).sum();
    if sum == 0 {
        return vec![MemoryCategory::Free; count];
    }
    let quotas: Vec<(usize, u128)> = parts
        .iter()
        .map(|(_, bytes)| {
            let scaled = u128::from(*bytes) * count as u128;
            ((scaled / sum) as usize, scaled % sum)
        })
        .collect();
    let mut shares: Vec<usize> = quotas.iter().map(|(whole, _)| *whole).collect();
    let mut by_remainder: Vec<usize> = (0..parts.len()).collect();
    by_remainder.sort_by(|a, b| quotas[*b].1.cmp(&quotas[*a].1));
    let missing = count - shares.iter().sum::<usize>();
    for index in by_remainder.into_iter().take(missing) {
        shares[index] += 1;
    }
    parts
        .iter()
        .zip(shares)
        .flat_map(|((category, _), share)| std::iter::repeat_n(*category, share))
        .collect()
}

/// One CPU row: User and System as detail, their busy sum as the value.
fn cpu_row(state: CpuModuleState) -> Option<StatRow> {
    let (detail, value) = match state {
        CpuModuleState::Disabled => return None,
        CpuModuleState::Loading => ("Loading…".to_string(), None),
        CpuModuleState::Unavailable => ("Unavailable".to_string(), None),
        CpuModuleState::Available(cpu) => (
            format!("{} usr · {} sys", cpu.user_percent, cpu.system_percent),
            Some(format!(
                "{}%",
                cpu.user_percent.saturating_add(cpu.system_percent).min(100)
            )),
        ),
    };
    Some(StatRow {
        primary: "CPU".to_string(),
        detail,
        value,
    })
}

/// One GPU row: Renderer and Tiler as detail when the driver reports them,
/// Device Utilization as the value.
fn gpu_row(state: GpuModuleState) -> Option<StatRow> {
    let GpuModuleState::Available(gpu) = state else {
        return None;
    };
    let detail: Vec<String> = [
        ("render", gpu.renderer_percent),
        ("tiler", gpu.tiler_percent),
    ]
    .into_iter()
    .filter_map(|(name, percent)| percent.map(|percent| format!("{name} {percent}")))
    .collect();
    let utilization = format!("{}%", gpu.utilization_percent.min(100));
    let (detail, value) = if detail.is_empty() {
        (utilization, None)
    } else {
        (detail.join(" · "), Some(utilization))
    };
    Some(StatRow {
        primary: "GPU".to_string(),
        detail,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CpuModuleState, CpuSnapshot, GpuModuleState, GpuSnapshot, MemorySnapshot, PressureSource,
    };

    fn snapshot(total_bytes: u64) -> SystemSnapshot {
        SystemSnapshot {
            memory: MemorySnapshot {
                used_bytes: total_bytes / 2,
                total_bytes,
                used_percent: 50,
                pressure_percent: 50,
                pressure_source: PressureSource::Kernel,
                app_memory_bytes: total_bytes / 4,
                wired_bytes: total_bytes / 8,
                compressed_bytes: total_bytes / 8,
                free_bytes: total_bytes / 4,
                cached_bytes: 0,
                swap_used_bytes: 0,
                available_bytes: total_bytes / 2,
            },
            cpu: CpuModuleState::Disabled,
            gpu: GpuModuleState::Disabled,
        }
    }

    fn model(snapshot: SystemSnapshot) -> DropdownModel {
        dropdown_model(snapshot, &[], &SharedMac::Hidden)
    }

    const SIXTEEN_GIB: u64 = 17_179_869_184;

    #[test]
    fn gauge_tooltip_pairs_percent_with_used_over_total() {
        assert_eq!(
            gauge_tooltip(47, 7_729_084_723, 17_179_869_184),
            "47% · 7.2 / 16.0 GB"
        );
    }

    #[test]
    fn gauge_accessibility_label_is_spoken_friendly() {
        assert_eq!(
            gauge_accessibility_label(47, 7_729_084_723, 17_179_869_184),
            "Memory 47 percent, 7.2 of 16.0 GB used"
        );
    }

    #[test]
    fn mem_text_at_gib_boundary_uses_mb_then_gb() {
        assert_eq!(mem_text(ONE_GIB_BYTES - 1), "1024 MB");
        assert_eq!(mem_text(ONE_GIB_BYTES), "1.0 GB");
    }

    #[test]
    fn share_of_total_rounds_to_nearest_percent() {
        assert_eq!(share_of_total(0, SIXTEEN_GIB), 0);
        assert_eq!(share_of_total(SIXTEEN_GIB / 4, SIXTEEN_GIB), 25);
        assert_eq!(share_of_total(SIXTEEN_GIB / 2, SIXTEEN_GIB), 50);
        // 6/16 = 37.5 → 38
        assert_eq!(share_of_total(6 * (SIXTEEN_GIB / 16), SIXTEEN_GIB), 38);
        assert_eq!(share_of_total(1, 0), 0);
    }

    #[test]
    fn history_caption_reports_the_span_and_used_range() {
        assert_eq!(history_caption(&[], SIXTEEN_GIB), None);
        assert_eq!(history_caption(&[ONE_GIB_BYTES], SIXTEEN_GIB), None);
        assert_eq!(
            history_caption(&[12 * ONE_GIB_BYTES, 8 * ONE_GIB_BYTES], SIXTEEN_GIB),
            Some(("5 s".into(), "50–75% used".into()))
        );
        assert_eq!(
            history_caption(&[8 * ONE_GIB_BYTES; 24], SIXTEEN_GIB),
            Some(("115 s".into(), "50% used".into()))
        );
        assert_eq!(
            history_caption(&[8 * ONE_GIB_BYTES; 25], SIXTEEN_GIB),
            Some(("2 min".into(), "50% used".into()))
        );
    }

    #[test]
    fn map_cells_fill_exactly_and_keep_category_order() {
        let parts = [
            (MemoryCategory::App, 4_300),
            (MemoryCategory::Wired, 3_200),
            (MemoryCategory::Compressed, 4_200),
            (MemoryCategory::Cached, 4_200),
            (MemoryCategory::Free, 100),
        ];
        let cells = allocate_cells(&parts, MAP_CELLS);
        let count = |category| cells.iter().filter(|cell| **cell == category).count();

        assert_eq!(cells.len(), MAP_CELLS);
        assert_eq!(
            [
                count(MemoryCategory::App),
                count(MemoryCategory::Wired),
                count(MemoryCategory::Compressed),
                count(MemoryCategory::Cached),
                count(MemoryCategory::Free),
            ],
            [17, 13, 17, 17, 0]
        );
        assert!(cells.windows(2).all(|pair| {
            let rank = |category| parts.iter().position(|(c, _)| *c == category);
            rank(pair[0]) <= rank(pair[1])
        }));
    }

    #[test]
    fn row_hierarchy_promotes_totals_and_demotes_derived_breakdowns() {
        // #23: brightness tracks actionability. App Memory is primary; the
        // derived breakdown rows are demoted.
        let breakdown = model(snapshot(SIXTEEN_GIB)).breakdown;
        assert!(breakdown[0].primary, "App Memory is the total");
        assert!(
            breakdown[1..].iter().all(|row| !row.primary),
            "Wired/Compressed/Cached are derived"
        );
    }

    #[test]
    fn cpu_row_shows_the_user_system_split_and_their_busy_sum() {
        let row = |state| cpu_row(state).map(|row| (row.detail, row.value));
        assert_eq!(
            row(CpuModuleState::Available(CpuSnapshot {
                user_percent: 41,
                system_percent: 13,
            })),
            Some(("41 usr · 13 sys".to_string(), Some("54%".to_string())))
        );
        assert_eq!(
            row(CpuModuleState::Loading),
            Some(("Loading…".to_string(), None))
        );
        assert_eq!(
            row(CpuModuleState::Unavailable),
            Some(("Unavailable".to_string(), None))
        );
        assert_eq!(row(CpuModuleState::Disabled), None);
    }

    #[test]
    fn gpu_row_lists_reported_detail_before_utilization() {
        let row = |renderer_percent, tiler_percent| {
            gpu_row(GpuModuleState::Available(GpuSnapshot {
                utilization_percent: 76,
                renderer_percent,
                tiler_percent,
            }))
            .map(|row| (row.detail, row.value))
        };
        assert_eq!(
            row(Some(54), Some(12)),
            Some(("render 54 · tiler 12".to_string(), Some("76%".to_string())))
        );
        assert_eq!(row(None, None), Some(("76%".to_string(), None)));
        assert_eq!(gpu_row(GpuModuleState::Unavailable), None);
    }

    #[test]
    fn dropdown_model_splits_memory_and_swap_rows() {
        let snapshot = MemorySnapshot {
            used_bytes: 9 * ONE_GIB_BYTES,
            total_bytes: SIXTEEN_GIB,
            used_percent: 53,
            pressure_percent: 96,
            pressure_source: PressureSource::Kernel,
            app_memory_bytes: 6 * ONE_GIB_BYTES,
            wired_bytes: 2 * ONE_GIB_BYTES,
            compressed_bytes: ONE_GIB_BYTES,
            free_bytes: 3 * ONE_GIB_BYTES,
            cached_bytes: 4 * ONE_GIB_BYTES,
            swap_used_bytes: 4_724_461_226,
            available_bytes: 8_160_449_024,
        };

        let memory = model(SystemSnapshot {
            memory: snapshot,
            cpu: CpuModuleState::Disabled,
            gpu: GpuModuleState::Disabled,
        });

        assert_eq!(memory.accent, Accent::Critical);
        assert_eq!(memory.map.used_percent, 53);
        assert_eq!(memory.map.used_of_total, "9.0 / 16.0 GB");
        assert_eq!(memory.map.cell_caption, "1 cell = 256 MB");
        assert_eq!(memory.pressure_percent, 96);
        let legend: Vec<(&str, &str, Swatch)> = memory
            .breakdown
            .iter()
            .map(|row| (row.label.as_str(), row.value.as_str(), row.swatch))
            .collect();
        assert_eq!(
            legend,
            [
                ("App Memory", "6.0 GB", Swatch::Accent(100)),
                ("Wired", "2.0 GB", Swatch::Accent(62)),
                ("Compressed", "1.0 GB", Swatch::Accent(36)),
                ("Cached", "4.0 GB", Swatch::Hatched),
            ],
            "Other and Free stay on the map only"
        );
        let swap = memory.swap.as_ref().expect("swap row present when nonzero");
        assert_eq!(swap.primary, "Swap");
        assert_eq!(swap.detail, "4.4 GB");
        assert_eq!(swap.value, None);
    }

    #[test]
    fn shared_section_names_the_mac_and_fills_two_rows() {
        let reading = crate::share::SharedReading {
            name: "nicomini".to_string(),
            model: "Mac mini".to_string(),
            memory: snapshot(SIXTEEN_GIB).memory,
            cpu: Some(CpuSnapshot {
                user_percent: 14,
                system_percent: 7,
            }),
            gpu: None,
        };
        let SharedSection::Reading(display) = shared_section(&SharedMac::Reading(reading)) else {
            panic!("expected a reading");
        };
        assert_eq!(display.title, "Mac mini");
        assert_eq!(display.cells.len(), MAP_CELLS);
        assert_eq!(display.cpu, "21%");
        assert_eq!(display.gpu, "–");

        assert_eq!(
            shared_section(&SharedMac::Offline { title: None }),
            SharedSection::Status {
                title: "Shared Mac".to_string(),
                status: "Offline"
            }
        );
        assert_eq!(shared_section(&SharedMac::Hidden), SharedSection::Hidden);
    }
}
