use crate::model::{
    classify_pressure, CpuModuleState, GpuModuleState, MemoryPressure, SystemSnapshot,
};
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};
use crate::trend::MEANINGFUL_APP_DELTA_BYTES;

const APP_NAME_MAX_CHARS: usize = 16;
const APP_USAGE_ROW_LIMIT: usize = 5;

pub fn gauge_symbol_name(percent: u8) -> &'static str {
    match percent {
        0..=19 => "gauge.with.dots.needle.0percent",
        20..=39 => "gauge.with.dots.needle.33percent",
        40..=59 => "gauge.with.dots.needle.50percent",
        60..=79 => "gauge.with.dots.needle.67percent",
        _ => "gauge.with.dots.needle.100percent",
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatRow {
    pub primary: String,
    pub tail: Option<String>,
    /// Stable identity (the app's `group_key`) used to quit the app. `Some` only
    /// when the row is quittable. Kept as an identity rather than a positional
    /// index so a reordered or refreshed app list can never quit the wrong app.
    pub quit_key: Option<String>,
    pub bundle_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSectionDisplay {
    Hidden,
    Loading,
    Rows { rows: Vec<StatRow> },
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accent {
    Macos,
    Warning,
    Critical,
}

impl From<MemoryPressure> for Accent {
    fn from(pressure: MemoryPressure) -> Self {
        match pressure {
            MemoryPressure::Normal => Self::Macos,
            MemoryPressure::Warning => Self::Warning,
            MemoryPressure::Critical => Self::Critical,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingDisplay {
    pub label: String,
    pub percent: u8,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendRow {
    pub label: String,
    pub value: String,
    pub opacity_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryModuleDisplay {
    pub rings: [RingDisplay; 2],
    pub breakdown: [LegendRow; 4],
    pub swap: Option<StatRow>,
    pub history: Vec<u64>,
    pub apps: AppSectionDisplay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuModuleDisplay {
    pub state: CpuDisplayState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuModuleDisplay {
    pub utilization: StatRow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuDisplayState {
    Loading,
    Available {
        utilization: [LegendRow; 2],
        cores: Vec<StatRow>,
    },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleDisplay {
    Memory(Box<MemoryModuleDisplay>),
    Cpu(CpuModuleDisplay),
    Gpu(GpuModuleDisplay),
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
    dropdown_model_with_apps(snapshot, &AppMemorySnapshot::Hidden, &[])
}

pub fn dropdown_model_with_apps(
    snapshot: SystemSnapshot,
    apps: &AppMemorySnapshot,
    history: &[u64],
) -> DropdownModel {
    let memory = snapshot.memory;
    let accent = Accent::from(classify_pressure(memory.pressure_percent));
    let mut modules = vec![ModuleDisplay::Memory(Box::new(MemoryModuleDisplay {
        rings: [
            RingDisplay {
                label: "Memory %".to_string(),
                percent: memory.used_percent,
                detail: gb_pair(memory.used_bytes, memory.total_bytes),
            },
            RingDisplay {
                label: "Pressure".to_string(),
                percent: memory.pressure_percent,
                detail: String::new(),
            },
        ],
        breakdown: [
            legend_row("App Memory", memory.app_memory_bytes, 100),
            legend_row("Wired", memory.wired_bytes, 65),
            legend_row("Compressed", memory.compressed_bytes, 35),
            legend_row("Free", memory.free_bytes, 12),
        ],
        swap: (memory.swap_used_bytes > 0).then(|| StatRow {
            primary: "Swap".to_string(),
            tail: Some(mem_text(memory.swap_used_bytes)),
            quit_key: None,
            bundle_path: None,
        }),
        history: history.to_vec(),
        apps: app_section_display(apps),
    }))];
    match snapshot.cpu {
        CpuModuleState::Disabled => {}
        CpuModuleState::Loading => modules.push(ModuleDisplay::Cpu(CpuModuleDisplay {
            state: CpuDisplayState::Loading,
        })),
        CpuModuleState::Available(cpu) => {
            let mut cores = Vec::with_capacity(2);
            if let Some(percent) = cpu.efficiency_percent {
                cores.push(cpu_core_row("E-cores", percent));
            }
            if let Some(percent) = cpu.performance_percent {
                cores.push(cpu_core_row("P-cores", percent));
            }
            modules.push(ModuleDisplay::Cpu(CpuModuleDisplay {
                state: CpuDisplayState::Available {
                    utilization: [
                        cpu_legend_row("User", cpu.user_percent, 100),
                        cpu_legend_row("System", cpu.system_percent, 50),
                    ],
                    cores,
                },
            }));
        }
        CpuModuleState::Unavailable => modules.push(ModuleDisplay::Cpu(CpuModuleDisplay {
            state: CpuDisplayState::Unavailable,
        })),
    }
    if let GpuModuleState::Available(gpu) = snapshot.gpu {
        modules.push(ModuleDisplay::Gpu(GpuModuleDisplay {
            utilization: StatRow {
                primary: "Utilization".to_string(),
                tail: Some(format!("{}%", gpu.utilization_percent.min(100))),
                quit_key: None,
                bundle_path: None,
            },
        }));
    }
    DropdownModel::Loaded { accent, modules }
}

fn cpu_legend_row(label: &str, percent: u8, opacity_percent: u8) -> LegendRow {
    LegendRow {
        label: label.to_string(),
        value: format!("{}%", percent.min(100)),
        opacity_percent,
    }
}

fn cpu_core_row(label: &str, percent: u8) -> StatRow {
    StatRow {
        primary: label.to_string(),
        tail: Some(format!("{}%", percent.min(100))),
        quit_key: None,
        bundle_path: None,
    }
}

fn legend_row(label: &str, bytes: u64, opacity_percent: u8) -> LegendRow {
    LegendRow {
        label: label.to_string(),
        value: mem_text(bytes),
        opacity_percent,
    }
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
        quit_key: app.can_quit.then(|| app.group_key.clone()),
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
    fn gauge_symbol_name_buckets_by_percent() {
        assert_eq!(gauge_symbol_name(0), "gauge.with.dots.needle.0percent");
        assert_eq!(gauge_symbol_name(19), "gauge.with.dots.needle.0percent");
        assert_eq!(gauge_symbol_name(20), "gauge.with.dots.needle.33percent");
        assert_eq!(gauge_symbol_name(39), "gauge.with.dots.needle.33percent");
        assert_eq!(gauge_symbol_name(40), "gauge.with.dots.needle.50percent");
        assert_eq!(gauge_symbol_name(59), "gauge.with.dots.needle.50percent");
        assert_eq!(gauge_symbol_name(60), "gauge.with.dots.needle.67percent");
        assert_eq!(gauge_symbol_name(79), "gauge.with.dots.needle.67percent");
        assert_eq!(gauge_symbol_name(80), "gauge.with.dots.needle.100percent");
        assert_eq!(gauge_symbol_name(100), "gauge.with.dots.needle.100percent");
    }

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
        let model =
            dropdown_model_with_apps(snapshot(SIXTEEN_GIB), &AppMemorySnapshot::Loading, &[]);
        assert_eq!(memory_module(&model).apps, AppSectionDisplay::Loading);
    }

    #[test]
    fn dropdown_model_with_apps_unavailable() {
        let model =
            dropdown_model_with_apps(snapshot(SIXTEEN_GIB), &AppMemorySnapshot::Unavailable, &[]);
        assert_eq!(memory_module(&model).apps, AppSectionDisplay::Unavailable);
    }

    #[test]
    fn dropdown_model_with_apps_rows_format() {
        let usage = vec![AppMemoryUsage {
            name: "Cursor".to_string(),
            group_key: "/Applications/Cursor.app".to_string(),
            footprint_bytes: 2_147_483_648,
            pids: vec![42],
            can_quit: true,
            delta_bytes: None,
        }];
        let model = dropdown_model_with_apps(
            snapshot(SIXTEEN_GIB),
            &AppMemorySnapshot::Loaded(usage),
            &[],
        );
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected Rows");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].primary, "Cursor");
        assert_eq!(rows[0].tail.as_deref(), Some("2.0 GB"));
        assert_eq!(
            rows[0].quit_key.as_deref(),
            Some("/Applications/Cursor.app")
        );
    }

    #[test]
    fn dropdown_model_app_row_under_one_gb_uses_mb() {
        let usage = vec![AppMemoryUsage {
            name: "Tiny".to_string(),
            group_key: "/Applications/Tiny.app".to_string(),
            footprint_bytes: 256_901_120,
            pids: vec![1],
            can_quit: true,
            delta_bytes: None,
        }];
        let model = dropdown_model_with_apps(
            snapshot(SIXTEEN_GIB),
            &AppMemorySnapshot::Loaded(usage),
            &[],
        );
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
            pids: vec![1],
            can_quit: true,
            delta_bytes: None,
        }];
        let model = dropdown_model_with_apps(
            snapshot(SIXTEEN_GIB),
            &AppMemorySnapshot::Loaded(usage),
            &[],
        );
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected Rows");
        };
        assert_eq!(rows[0].primary, "Codex Computer …");
        assert_eq!(rows[0].tail.as_deref(), Some("81 MB"));
    }

    #[test]
    fn dropdown_model_memory_ring_shows_percent() {
        let mut snapshot = snapshot(SIXTEEN_GIB);
        snapshot.memory.used_percent = 56;
        let model = dropdown_model(snapshot);
        assert_eq!(memory_module(&model).rings[0].percent, 56);
    }

    #[test]
    fn available_cpu_module_follows_memory_with_user_system_and_core_rows() {
        let mut snapshot = snapshot(SIXTEEN_GIB);
        snapshot.cpu = CpuModuleState::Available(CpuSnapshot {
            user_percent: 41,
            system_percent: 13,
            efficiency_percent: Some(22),
            performance_percent: Some(71),
        });

        let DropdownModel::Loaded { modules, .. } = dropdown_model(snapshot) else {
            panic!("expected loaded model");
        };
        assert!(matches!(modules.first(), Some(ModuleDisplay::Memory(_))));
        let Some(ModuleDisplay::Cpu(cpu)) = modules.get(1) else {
            panic!("expected CPU module after Memory");
        };
        let CpuDisplayState::Available { utilization, cores } = &cpu.state else {
            panic!("expected available CPU state");
        };
        assert_eq!(
            utilization[0],
            LegendRow {
                label: "User".to_string(),
                value: "41%".to_string(),
                opacity_percent: 100,
            }
        );
        assert_eq!(utilization[1].label, "System");
        assert_eq!(utilization[1].value, "13%");
        assert_eq!(cores[0].primary, "E-cores");
        assert_eq!(cores[0].tail.as_deref(), Some("22%"));
        assert_eq!(cores[1].primary, "P-cores");
        assert_eq!(cores[1].tail.as_deref(), Some("71%"));
    }

    #[test]
    fn cpu_projection_covers_disabled_loading_and_unavailable_without_hiding_memory() {
        let disabled = dropdown_model(snapshot(SIXTEEN_GIB));
        let DropdownModel::Loaded { modules, .. } = disabled else {
            panic!("expected loaded model");
        };
        assert_eq!(modules.len(), 1);

        let mut loading_snapshot = snapshot(SIXTEEN_GIB);
        loading_snapshot.cpu = CpuModuleState::Loading;
        let DropdownModel::Loaded { modules, .. } = dropdown_model(loading_snapshot) else {
            panic!("expected loaded model");
        };
        assert!(matches!(modules.first(), Some(ModuleDisplay::Memory(_))));
        assert!(matches!(
            modules.get(1),
            Some(ModuleDisplay::Cpu(CpuModuleDisplay {
                state: CpuDisplayState::Loading
            }))
        ));

        let mut unavailable_snapshot = snapshot(SIXTEEN_GIB);
        unavailable_snapshot.cpu = CpuModuleState::Unavailable;
        let DropdownModel::Loaded { modules, .. } = dropdown_model(unavailable_snapshot) else {
            panic!("expected loaded model");
        };
        assert!(matches!(modules.first(), Some(ModuleDisplay::Memory(_))));
        assert!(matches!(
            modules.get(1),
            Some(ModuleDisplay::Cpu(CpuModuleDisplay {
                state: CpuDisplayState::Unavailable
            }))
        ));
    }

    #[test]
    fn available_gpu_module_follows_existing_modules_and_unavailable_gpu_stays_hidden() {
        let mut available = snapshot(SIXTEEN_GIB);
        available.gpu = GpuModuleState::Available(GpuSnapshot {
            utilization_percent: 76,
        });

        let DropdownModel::Loaded { modules, .. } = dropdown_model(available) else {
            panic!("expected loaded model");
        };
        let Some(ModuleDisplay::Gpu(gpu)) = modules.get(1) else {
            panic!("expected GPU module after Memory");
        };
        assert_eq!(gpu.utilization.primary, "Utilization");
        assert_eq!(gpu.utilization.tail.as_deref(), Some("76%"));

        let mut unavailable = snapshot(SIXTEEN_GIB);
        unavailable.gpu = GpuModuleState::Unavailable;
        let DropdownModel::Loaded { modules, .. } = dropdown_model(unavailable) else {
            panic!("expected loaded model");
        };
        assert_eq!(modules.len(), 1);
    }

    #[test]
    fn dropdown_model_with_apps_keeps_top_five_sorted() {
        // Input arrives pre-ranked from trend::rank_app_rows; format only projects the top 5.
        let mut usage = vec![
            usage("Six", 6, None),
            usage("One", 1, None),
            usage("Five", 5, None),
            usage("Two", 2, None),
            usage("Four", 4, None),
            usage("Three", 3, None),
        ];
        rank_app_rows(&mut usage);
        let model = dropdown_model_with_apps(snapshot(100), &AppMemorySnapshot::Loaded(usage), &[]);
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected app rows");
        };
        let names: Vec<_> = rows.iter().map(|row| row.primary.as_str()).collect();
        assert_eq!(names, vec!["Six", "Five", "Four", "Three", "Two"]);
    }

    #[test]
    fn dropdown_model_with_apps_prefers_positive_deltas() {
        let mut usage = vec![
            usage("Chrome", 4_000_000_000, None),
            usage("Zen", 734_003_200, Some(314_572_800)),
            usage("Codex", 500_000_000, Some(80_000_000)),
        ];
        rank_app_rows(&mut usage);
        let model = dropdown_model_with_apps(
            snapshot(SIXTEEN_GIB),
            &AppMemorySnapshot::Loaded(usage),
            &[],
        );
        let AppSectionDisplay::Rows { rows } = &memory_module(&model).apps else {
            panic!("expected app rows");
        };
        assert_eq!(rows[0].primary, "Zen");
        assert_eq!(rows[0].tail.as_deref(), Some("700 MB\t+300 MB"));
        assert_eq!(rows[0].quit_key.as_deref(), Some("/Applications/Zen.app"));
    }

    fn usage(name: &str, footprint_bytes: u64, delta_bytes: Option<i64>) -> AppMemoryUsage {
        AppMemoryUsage {
            name: name.to_string(),
            group_key: format!("/Applications/{name}.app"),
            footprint_bytes,
            pids: vec![1],
            can_quit: true,
            delta_bytes,
        }
    }
}
