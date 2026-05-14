use crate::model::{MemoryPressure, MemorySnapshot};
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};
use crate::trend::rank_app_rows;

const APP_NAME_MAX_CHARS: usize = 18;
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

pub fn gb_text(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000_f64;
    format!("{gb:.1} GB")
}

pub fn gb_pair(used_bytes: u64, total_bytes: u64) -> String {
    let used = used_bytes as f64 / 1_000_000_000_f64;
    let total = total_bytes as f64 / 1_000_000_000_f64;
    format!("{used:.1} / {total:.1} GB")
}

const ONE_GB_BYTES: u64 = 1_000_000_000;

pub fn mem_text(bytes: u64) -> String {
    if bytes >= ONE_GB_BYTES {
        gb_text(bytes)
    } else {
        let mb = (bytes as f64 / 1_000_000_f64).round() as u64;
        format!("{mb} MB")
    }
}

pub fn signed_delta_pct_text(footprint_bytes: u64, delta_bytes: u64) -> String {
    let previous = footprint_bytes.saturating_sub(delta_bytes);
    if previous == 0 {
        return "+new".to_string();
    }
    let pct = (delta_bytes as f64 * 100.0 / previous as f64).round() as u64;
    format!("+{pct}%")
}

pub fn delta_bytes_text(delta_bytes: u64) -> String {
    format!("+{}", mem_text(delta_bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatRow {
    pub primary: String,
    pub tail: Option<String>,
    pub action_tag: Option<usize>,
    pub bundle_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureDisplay {
    pub text: String,
    pub is_high: bool,
    pub is_elevated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSectionDisplay {
    Hidden,
    Loading,
    Rows {
        rows: Vec<StatRow>,
    },
    Unavailable,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownModel {
    Loading,
    Loaded {
        memory: StatRow,
        apps: AppSectionDisplay,
        pressure: Option<PressureDisplay>,
        swap: Option<StatRow>,
    },
}

fn pressure_text(p: MemoryPressure) -> &'static str {
    match p {
        MemoryPressure::Normal => "Normal",
        MemoryPressure::Elevated => "Elevated",
        MemoryPressure::High => "High",
    }
}

pub fn dropdown_model(snapshot: MemorySnapshot) -> DropdownModel {
    dropdown_model_with_apps(snapshot, &AppMemorySnapshot::Hidden)
}

pub fn dropdown_model_with_apps(
    snapshot: MemorySnapshot,
    apps: &AppMemorySnapshot,
) -> DropdownModel {
    DropdownModel::Loaded {
        memory: StatRow {
            primary: gb_pair(snapshot.used_bytes, snapshot.total_bytes),
            tail: Some(format!("{}%", snapshot.used_percent)),
            action_tag: None,
            bundle_path: None,
        },
        apps: app_section_display(apps),
        pressure: pressure_display(snapshot.pressure),
        swap: (snapshot.swap_used_bytes > 0).then(|| StatRow {
            primary: "Swap".to_string(),
            tail: Some(mem_text(snapshot.swap_used_bytes)),
            action_tag: None,
            bundle_path: None,
        }),
    }
}

fn pressure_display(pressure: MemoryPressure) -> Option<PressureDisplay> {
    match pressure {
        MemoryPressure::Normal => None,
        MemoryPressure::Elevated | MemoryPressure::High => Some(PressureDisplay {
            text: pressure_text(pressure).to_string(),
            is_high: matches!(pressure, MemoryPressure::High),
            is_elevated: matches!(pressure, MemoryPressure::Elevated),
        }),
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
        AppMemorySnapshot::Loaded(rows) => {
            let mut rows = rows.clone();
            rank_app_rows(&mut rows);
            rows.truncate(APP_USAGE_ROW_LIMIT);
            AppSectionDisplay::Rows {
                rows: rows
                    .iter()
                    .enumerate()
                    .map(|(idx, r)| app_row(idx, r))
                    .collect(),
            }
        }
    }
}

fn app_row(index: usize, app: &AppMemoryUsage) -> StatRow {
    let tail = if let Some(delta) = app.delta_bytes.filter(|delta| *delta >= 50_000_000) {
        format!(
            "{}\t{} / {}",
            mem_text(app.footprint_bytes),
            delta_bytes_text(delta as u64),
            signed_delta_pct_text(app.footprint_bytes, delta as u64)
        )
    } else {
        mem_text(app.footprint_bytes)
    };
    StatRow {
        primary: truncate_name(&app.name, APP_NAME_MAX_CHARS),
        tail: Some(tail),
        action_tag: app.can_quit.then_some(index),
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

    fn snapshot(total_bytes: u64) -> MemorySnapshot {
        MemorySnapshot {
            used_bytes: total_bytes / 2,
            total_bytes,
            used_percent: 50,
            pressure: MemoryPressure::Normal,
            swap_used_bytes: 0,
        }
    }

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
        let model = dropdown_model(snapshot(16_000_000_000));
        match model {
            DropdownModel::Loaded { apps, .. } => {
                assert_eq!(apps, AppSectionDisplay::Hidden);
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn dropdown_model_with_apps_loading() {
        let model = dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Loading);
        match model {
            DropdownModel::Loaded { apps, .. } => {
                assert_eq!(apps, AppSectionDisplay::Loading);
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn dropdown_model_with_apps_unavailable() {
        let model =
            dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Unavailable);
        match model {
            DropdownModel::Loaded { apps, .. } => {
                assert_eq!(apps, AppSectionDisplay::Unavailable);
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn dropdown_model_with_apps_rows_format() {
        let usage = vec![AppMemoryUsage {
            name: "Cursor".to_string(),
            group_key: "/Applications/Cursor.app".to_string(),
            footprint_bytes: 2_000_000_000,
            pids: vec![42],
            can_quit: true,
            delta_bytes: None,
        }];
        let model =
            dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded { apps, .. } => match apps {
                AppSectionDisplay::Rows { rows } => {
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].primary, "Cursor");
                    assert_eq!(rows[0].tail.as_deref(), Some("2.0 GB"));
                    assert_eq!(rows[0].action_tag, Some(0));
                }
                _ => panic!("expected Rows"),
            },
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn dropdown_model_app_row_under_one_gb_uses_mb() {
        let usage = vec![AppMemoryUsage {
            name: "Tiny".to_string(),
            group_key: "/Applications/Tiny.app".to_string(),
            footprint_bytes: 245_000_000,
            pids: vec![1],
            can_quit: true,
            delta_bytes: None,
        }];
        let model =
            dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded { apps, .. } => match apps {
                AppSectionDisplay::Rows { rows, .. } => {
                    assert_eq!(rows[0].tail.as_deref(), Some("245 MB"));
                }
                _ => panic!("expected Rows"),
            },
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn dropdown_model_memory_tail_is_just_percent() {
        let snapshot = MemorySnapshot {
            used_bytes: 9_000_000_000,
            total_bytes: 16_000_000_000,
            used_percent: 56,
            pressure: MemoryPressure::Normal,
            swap_used_bytes: 0,
        };
        let DropdownModel::Loaded { memory, .. } = dropdown_model(snapshot) else {
            panic!("expected Loaded model");
        };
        assert_eq!(memory.tail.as_deref(), Some("56%"));
    }

    #[test]
    fn dropdown_model_hides_pressure_when_normal() {
        let snapshot = MemorySnapshot {
            used_bytes: 5_000_000_000,
            total_bytes: 16_000_000_000,
            used_percent: 31,
            pressure: MemoryPressure::Normal,
            swap_used_bytes: 0,
        };
        let DropdownModel::Loaded { pressure, .. } = dropdown_model(snapshot) else {
            panic!("expected Loaded model");
        };
        assert!(pressure.is_none());
    }

    #[test]
    fn dropdown_model_with_apps_keeps_top_five_sorted() {
        let usage = vec![
            usage("Six", 6, None),
            usage("One", 1, None),
            usage("Five", 5, None),
            usage("Two", 2, None),
            usage("Four", 4, None),
            usage("Three", 3, None),
        ];
        let model = dropdown_model_with_apps(snapshot(100), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded {
                apps: AppSectionDisplay::Rows { rows, .. },
                ..
            } => {
                let names: Vec<_> = rows.iter().map(|row| row.primary.as_str()).collect();
                assert_eq!(names, vec!["Six", "Five", "Four", "Three", "Two"]);
            }
            _ => panic!("expected app rows"),
        }
    }

    #[test]
    fn dropdown_model_with_apps_prefers_positive_deltas() {
        let usage = vec![
            usage("Chrome", 4_000_000_000, None),
            usage("Zen", 700_000_000, Some(300_000_000)),
            usage("Codex", 500_000_000, Some(80_000_000)),
        ];
        let model =
            dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded {
                apps: AppSectionDisplay::Rows { rows },
                ..
            } => {
                assert_eq!(rows[0].primary, "Zen");
                assert_eq!(rows[0].tail.as_deref(), Some("700 MB\t+300 MB / +75%"));
                assert_eq!(rows[0].action_tag, Some(0));
            }
            _ => panic!("expected app rows"),
        }
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
