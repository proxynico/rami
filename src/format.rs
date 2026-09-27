use crate::model::{
    classify_pressure, CpuModuleState, GpuModuleState, MemoryPressure, MemorySnapshot,
    SystemSnapshot,
};
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};
use crate::trend::MEANINGFUL_APP_DELTA_BYTES;

const APP_NAME_MAX_CHARS: usize = 16;
const APP_USAGE_ROW_LIMIT: usize = 3;

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

pub fn delta_bytes_text(delta_bytes: u64) -> String {
    format!("+{}", mem_text(delta_bytes))
}

/// Share of physical RAM, rounded to the nearest percent and capped at 100.
pub fn share_of_total(bytes: u64, total_bytes: u64) -> u8 {
    if total_bytes == 0 {
        return 0;
    }
    let percent = (u128::from(bytes) * 100 + u128::from(total_bytes) / 2) / u128::from(total_bytes);
    percent.min(100) as u8
}

/// Caption for the memory-history row: the window and its used-percent
/// range, e.g. ("2 min", "71–75% used"). `None` while fewer than two samples
/// are available.
pub fn history_caption(samples: &[u64], total_bytes: u64) -> Option<(String, String)> {
    if samples.len() < 2 {
        return None;
    }
    let low = share_of_total(*samples.iter().min()?, total_bytes);
    let high = share_of_total(*samples.iter().max()?, total_bytes);
    let range = if low == high {
        format!("{high}% used")
    } else {
        format!("{low}–{high}% used")
    };
    Some(("2 min".to_string(), range))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatRow {
    pub primary: String,
    pub tail: Option<String>,
    pub bundle_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSectionDisplay {
    Hidden,
    Loading,
    Rows { rows: Vec<StatRow> },
    Unavailable,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryModuleDisplay {
    pub map: MemoryMapDisplay,
    pub pressure_percent: u8,
    pub total_bytes: u64,
    /// Trend-window samples for the one memory-history row, oldest first.
    pub history: Vec<u64>,
    /// Rows partition physical RAM, less any Other too small to show.
    pub breakdown: Vec<LegendRow>,
    pub swap: Option<StatRow>,
    pub apps: AppSectionDisplay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleDisplay {
    Memory(Box<MemoryModuleDisplay>),
    Cpu(StatRow),
    Gpu(StatRow),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownModel {
    Loading,
    Loaded {
        accent: Accent,
        modules: Vec<ModuleDisplay>,
    },
}

pub fn dropdown_model(snapshot: SystemSnapshot) -> DropdownModel {
    dropdown_model_with_apps(snapshot, &AppMemorySnapshot::Hidden)
}

pub fn dropdown_model_with_apps(
    snapshot: SystemSnapshot,
    apps: &AppMemorySnapshot,
) -> DropdownModel {
    dropdown_model_with_sections(snapshot, apps, &[])
}

pub(crate) fn dropdown_model_with_sections(
    snapshot: SystemSnapshot,
    apps: &AppMemorySnapshot,
    history: &[u64],
) -> DropdownModel {
    let memory = snapshot.memory;
    let accent = Accent::from(classify_pressure(memory.pressure_percent));
    let mut modules = vec![ModuleDisplay::Memory(Box::new(MemoryModuleDisplay {
        map: memory_map(&memory),
        pressure_percent: memory.pressure_percent,
        total_bytes: memory.total_bytes,
        history: history.to_vec(),
        breakdown: memory_breakdown(&memory),
        swap: (memory.swap_used_bytes > 0).then(|| StatRow {
            primary: "Swap".to_string(),
            tail: Some(mem_text(memory.swap_used_bytes)),
            bundle_path: None,
        }),
        apps: app_section_display(apps),
    }))];
    if let Some(row) = cpu_row(snapshot.cpu) {
        modules.push(ModuleDisplay::Cpu(row));
    }
    if let Some(row) = gpu_row(snapshot.gpu) {
        modules.push(ModuleDisplay::Gpu(row));
    }
    DropdownModel::Loaded { accent, modules }
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

fn memory_breakdown(memory: &MemorySnapshot) -> Vec<LegendRow> {
    memory_parts(memory)
        .into_iter()
        .map(|(category, bytes)| LegendRow {
            label: category.label().to_string(),
            value: format!(
                "{} · {}%",
                mem_text(bytes),
                share_of_total(bytes, memory.total_bytes)
            ),
            swatch: category.swatch(),
            primary: category == MemoryCategory::App,
        })
        .collect()
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
    let tail = match state {
        CpuModuleState::Disabled => return None,
        CpuModuleState::Loading => "Loading…".to_string(),
        CpuModuleState::Unavailable => "Unavailable".to_string(),
        CpuModuleState::Available(cpu) => format!(
            "{} usr · {} sys\t{}%",
            cpu.user_percent,
            cpu.system_percent,
            cpu.user_percent.saturating_add(cpu.system_percent).min(100)
        ),
    };
    Some(StatRow {
        primary: "CPU".to_string(),
        tail: Some(tail),
        bundle_path: None,
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
    let value = format!("{}%", gpu.utilization_percent.min(100));
    let tail = if detail.is_empty() {
        value
    } else {
        format!("{}\t{value}", detail.join(" · "))
    };
    Some(StatRow {
        primary: "GPU".to_string(),
        tail: Some(tail),
        bundle_path: None,
    })
}

pub fn placeholder_dropdown_model() -> DropdownModel {
    DropdownModel::Loading
}

fn app_section_display(apps: &AppMemorySnapshot) -> AppSectionDisplay {
    match apps {
        AppMemorySnapshot::Hidden => AppSectionDisplay::Hidden,
        AppMemorySnapshot::Loading => AppSectionDisplay::Loading,
        AppMemorySnapshot::Unavailable => AppSectionDisplay::Unavailable,
        // Rows arrive already ranked (and delta-tagged) from `trend::app_rows_with_deltas`,
        // which is the single source of ranking. Here we only project the top N to display rows.
        AppMemorySnapshot::Loaded(rows) => AppSectionDisplay::Rows {
            rows: rows.iter().take(APP_USAGE_ROW_LIMIT).map(app_row).collect(),
        },
    }
}

fn app_row(app: &AppMemoryUsage) -> StatRow {
    let tail = if let Some(delta) = app
        .delta_bytes
        .filter(|delta| *delta >= MEANINGFUL_APP_DELTA_BYTES)
    {
        format!(
            "{}\t{}",
            mem_text(app.footprint_bytes),
            delta_bytes_text(delta as u64)
        )
    } else {
        mem_text(app.footprint_bytes)
    };
    StatRow {
        primary: truncate_name(&app.name, APP_NAME_MAX_CHARS),
        tail: Some(tail),
        bundle_path: app
            .group_key
            .ends_with(".app")
            .then(|| app.group_key.clone()),
    }
}

fn truncate_name(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars {
        return name.to_string();
    }
    let mut out: String = name.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CpuModuleState, CpuSnapshot, GpuModuleState, GpuSnapshot, MemorySnapshot, PressureSource,
    };
    use crate::trend::rank_app_rows;

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

    fn memory_module(model: &DropdownModel) -> &MemoryModuleDisplay {
        let DropdownModel::Loaded { modules, .. } = model else {
            panic!("expected loaded model");
        };
        let Some(ModuleDisplay::Memory(memory)) = modules.first() else {
            panic!("expected memory module");
        };
        memory
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
    fn share_of_total_rounds_to_nearest_percent() {
        assert_eq!(share_of_total(0, SIXTEEN_GIB), 0);
        assert_eq!(share_of_total(SIXTEEN_GIB / 4, SIXTEEN_GIB), 25);
        assert_eq!(share_of_total(SIXTEEN_GIB / 2, SIXTEEN_GIB), 50);
        // 6/16 = 37.5 → 38
        assert_eq!(share_of_total(6 * (SIXTEEN_GIB / 16), SIXTEEN_GIB), 38);
        assert_eq!(share_of_total(1, 0), 0);
    }

    #[test]
    fn history_caption_reports_the_window_and_used_range() {
        assert_eq!(history_caption(&[], SIXTEEN_GIB), None);
        assert_eq!(history_caption(&[ONE_GIB_BYTES], SIXTEEN_GIB), None);
        assert_eq!(
            history_caption(&[12 * ONE_GIB_BYTES, 8 * ONE_GIB_BYTES], SIXTEEN_GIB),
            Some(("2 min".into(), "50–75% used".into()))
        );
        assert_eq!(
            history_caption(&[8 * ONE_GIB_BYTES, 8 * ONE_GIB_BYTES], SIXTEEN_GIB),
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
    fn truncate_name_short_passthrough() {
        assert_eq!(truncate_name("Cursor", 28), "Cursor");
    }

    #[test]
    fn truncate_name_long_uses_ellipsis() {
        let result = truncate_name("ThisIsAVeryLongApplicationNameThatExceedsTheLimit", 28);
        assert_eq!(result.chars().count(), 28);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn dropdown_model_default_apps_hidden() {
        let model = dropdown_model(snapshot(SIXTEEN_GIB));
        assert_eq!(memory_module(&model).apps, AppSectionDisplay::Hidden);
    }

    #[test]
    fn dropdown_model_with_apps_loading() {
        let model = dropdown_model_with_apps(snapshot(SIXTEEN_GIB), &AppMemorySnapshot::Loading);
        assert_eq!(memory_module(&model).apps, AppSectionDisplay::Loading);
    }

    #[test]
    fn dropdown_model_with_apps_unavailable() {
        let model =
            dropdown_model_with_apps(snapshot(SIXTEEN_GIB), &AppMemorySnapshot::Unavailable);
        assert_eq!(memory_module(&model).apps, AppSectionDisplay::Unavailable);
    }

    #[test]
    fn dropdown_model_with_apps_rows_format() {
        let usage = vec![AppMemoryUsage {
            name: "Cursor".to_string(),
            group_key: "/Applications/Cursor.app".to_string(),
            footprint_bytes: 2_147_483_648,
            delta_bytes: None,
        }];
        let model =
            dropdown_model_with_apps(snapshot(SIXTEEN_GIB), &AppMemorySnapshot::Loaded(usage));
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected Rows");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].primary, "Cursor");
        assert_eq!(rows[0].tail.as_deref(), Some("2.0 GB"));
    }

    #[test]
    fn dropdown_model_app_row_under_one_gb_uses_mb() {
        let usage = vec![AppMemoryUsage {
            name: "Tiny".to_string(),
            group_key: "/Applications/Tiny.app".to_string(),
            footprint_bytes: 256_901_120,
            delta_bytes: None,
        }];
        let model =
            dropdown_model_with_apps(snapshot(SIXTEEN_GIB), &AppMemorySnapshot::Loaded(usage));
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected Rows");
        };
        assert_eq!(rows[0].tail.as_deref(), Some("245 MB"));
    }

    #[test]
    fn dropdown_model_app_row_truncates_names_before_tail_column() {
        let usage = vec![AppMemoryUsage {
            name: "Codex Computer Use".to_string(),
            group_key: "/Applications/Codex Computer Use.app".to_string(),
            footprint_bytes: 84_934_656,
            delta_bytes: None,
        }];
        let model =
            dropdown_model_with_apps(snapshot(SIXTEEN_GIB), &AppMemorySnapshot::Loaded(usage));
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected Rows");
        };
        assert_eq!(rows[0].primary, "Codex Computer …");
        assert_eq!(rows[0].tail.as_deref(), Some("81 MB"));
    }

    #[test]
    fn dropdown_model_memory_map_shows_percent() {
        let mut snapshot = snapshot(SIXTEEN_GIB);
        snapshot.memory.used_percent = 56;
        let model = dropdown_model(snapshot);
        assert_eq!(memory_module(&model).map.used_percent, 56);
    }

    #[test]
    fn row_hierarchy_promotes_totals_and_demotes_derived_breakdowns() {
        // #23: brightness tracks actionability. App Memory is primary; the
        // derived breakdown rows are demoted.
        let breakdown = memory_module(&dropdown_model(snapshot(SIXTEEN_GIB)))
            .breakdown
            .clone();
        assert!(breakdown[0].primary, "App Memory is the total");
        assert!(
            breakdown[1..].iter().all(|row| !row.primary),
            "Wired/Compressed/Free are derived"
        );
    }

    #[test]
    fn cpu_row_shows_the_user_system_split_and_their_busy_sum() {
        let row = |state| cpu_row(state).map(|row| row.tail.unwrap_or_default());
        assert_eq!(
            row(CpuModuleState::Available(CpuSnapshot {
                user_percent: 41,
                system_percent: 13,
            })),
            Some("41 usr · 13 sys\t54%".to_string())
        );
        assert_eq!(row(CpuModuleState::Loading), Some("Loading…".to_string()));
        assert_eq!(
            row(CpuModuleState::Unavailable),
            Some("Unavailable".to_string())
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
            .and_then(|row| row.tail)
        };
        assert_eq!(
            row(Some(54), Some(12)),
            Some("render 54 · tiler 12\t76%".to_string())
        );
        assert_eq!(row(None, None), Some("76%".to_string()));
        assert_eq!(gpu_row(GpuModuleState::Unavailable), None);
    }

    #[test]
    fn dropdown_model_with_apps_keeps_top_three_sorted() {
        // Input arrives pre-ranked from trend::rank_app_rows; the compact menu projects three.
        let mut usage = vec![
            usage("Six", 6, None),
            usage("One", 1, None),
            usage("Five", 5, None),
            usage("Two", 2, None),
            usage("Four", 4, None),
            usage("Three", 3, None),
        ];
        rank_app_rows(&mut usage);
        let model = dropdown_model_with_apps(snapshot(100), &AppMemorySnapshot::Loaded(usage));
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected app rows");
        };
        let names: Vec<_> = rows.iter().map(|row| row.primary.as_str()).collect();
        assert_eq!(names, vec!["Six", "Five", "Four"]);
    }

    #[test]
    fn dropdown_model_with_apps_prefers_positive_deltas() {
        let mut usage = vec![
            usage("Chrome", 4_000_000_000, None),
            usage("Zen", 734_003_200, Some(314_572_800)),
            usage("Codex", 500_000_000, Some(80_000_000)),
        ];
        rank_app_rows(&mut usage);
        let model =
            dropdown_model_with_apps(snapshot(SIXTEEN_GIB), &AppMemorySnapshot::Loaded(usage));
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected app rows");
        };
        assert_eq!(rows[0].primary, "Zen");
        assert_eq!(rows[0].tail.as_deref(), Some("700 MB\t+300 MB"));
    }

    fn usage(name: &str, footprint_bytes: u64, delta_bytes: Option<i64>) -> AppMemoryUsage {
        AppMemoryUsage {
            name: name.to_string(),
            group_key: format!("/Applications/{name}.app"),
            footprint_bytes,
            delta_bytes,
        }
    }
}
