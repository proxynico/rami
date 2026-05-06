use crate::model::{MemoryPressure, MemorySnapshot};
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};
use crate::trend::{likely_culprit, rank_app_rows, MemoryTrend};

const APP_NAME_MAX_CHARS: usize = 28;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSectionDisplay {
    Hidden,
    Loading,
    Rows {
        culprit: Option<StatRow>,
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
        pressure: PressureDisplay,
        swap: StatRow,
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
    dropdown_model_with_apps_and_trend(snapshot, MemoryTrend::Stable, &AppMemorySnapshot::Hidden)
}

pub fn dropdown_model_with_apps(
    snapshot: MemorySnapshot,
    apps: &AppMemorySnapshot,
) -> DropdownModel {
    dropdown_model_with_apps_and_trend(snapshot, MemoryTrend::Stable, apps)
}

pub fn dropdown_model_with_apps_and_trend(
    snapshot: MemorySnapshot,
    trend: MemoryTrend,
    apps: &AppMemorySnapshot,
) -> DropdownModel {
    DropdownModel::Loaded {
        memory: StatRow {
            primary: format!("{}%", snapshot.used_percent),
            tail: memory_tail(snapshot.used_bytes, snapshot.total_bytes, trend),
            action_tag: None,
            bundle_path: None,
        },
        apps: app_section_display(apps, snapshot.total_bytes),
        pressure: PressureDisplay {
            text: pressure_text(snapshot.pressure).to_string(),
            is_high: matches!(snapshot.pressure, MemoryPressure::High),
        },
        swap: StatRow {
            primary: gb_text(snapshot.swap_used_bytes),
            tail: None,
            action_tag: None,
            bundle_path: None,
        },
    }
}

pub fn placeholder_dropdown_model() -> DropdownModel {
    DropdownModel::Loading
}

fn app_section_display(apps: &AppMemorySnapshot, total_bytes: u64) -> AppSectionDisplay {
    match apps {
        AppMemorySnapshot::Hidden => AppSectionDisplay::Hidden,
        AppMemorySnapshot::Loading => AppSectionDisplay::Loading,
        AppMemorySnapshot::Unavailable => AppSectionDisplay::Unavailable,
        AppMemorySnapshot::Loaded(rows) => {
            let mut rows = rows.clone();
            rank_app_rows(&mut rows);
            let culprit = likely_culprit(&rows).map(|culprit| StatRow {
                primary: "Likely culprit:".to_string(),
                tail: Some(format!(
                    "{} {}",
                    culprit.name,
                    delta_text(culprit.delta_bytes)
                )),
                action_tag: None,
                bundle_path: None,
            });
            rows.truncate(APP_USAGE_ROW_LIMIT);
            AppSectionDisplay::Rows {
                culprit,
                rows: rows
                    .iter()
                    .enumerate()
                    .map(|(idx, r)| app_row(idx, r, total_bytes))
                    .collect(),
            }
        }
    }
}

fn memory_tail(used_bytes: u64, total_bytes: u64, trend: MemoryTrend) -> Option<String> {
    let base = gb_pair(used_bytes, total_bytes);
    match trend {
        MemoryTrend::Stable => Some(base),
        MemoryTrend::Rising => Some(format!("{base}  Rising")),
        MemoryTrend::RisingFast => Some(format!("{base}  Rising fast")),
    }
}

fn app_row(index: usize, app: &AppMemoryUsage, total_bytes: u64) -> StatRow {
    let tail = if let Some(delta) = app.delta_bytes.filter(|delta| *delta >= 50_000_000) {
        format!(
            "{}  {}",
            gb_text(app.footprint_bytes),
            delta_text(delta as u64)
        )
    } else {
        format!(
            "{}  {}",
            gb_text(app.footprint_bytes),
            percent_label(app.footprint_bytes, total_bytes)
        )
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

fn delta_text(bytes: u64) -> String {
    format!("+{} MB", bytes / 1_000_000)
}

fn percent_label(part: u64, total: u64) -> String {
    if total == 0 {
        return "—".to_string();
    }
    let raw = part as f64 / total as f64 * 100.0;
    if raw < 1.0 {
        "<1%".to_string()
    } else {
        format!("{}%", raw.round() as u32)
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
    fn percent_label_below_one_renders_lt() {
        assert_eq!(percent_label(5_000_000, 1_000_000_000_000), "<1%");
    }

    #[test]
    fn percent_label_at_one_renders_whole() {
        assert_eq!(percent_label(10, 1000), "1%");
    }

    #[test]
    fn percent_label_rounds_to_nearest() {
        assert_eq!(percent_label(127, 1000), "13%");
    }

    #[test]
    fn percent_label_handles_zero_total() {
        assert_eq!(percent_label(100, 0), "—");
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
                AppSectionDisplay::Rows { culprit, rows } => {
                    assert!(culprit.is_none());
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].primary, "Cursor");
                    assert_eq!(rows[0].tail.as_deref(), Some("2.0 GB  13%"));
                    assert_eq!(rows[0].action_tag, Some(0));
                }
                _ => panic!("expected Rows"),
            },
            _ => panic!("expected Loaded"),
        }
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
    fn dropdown_model_with_apps_prefers_positive_deltas_and_culprit() {
        let usage = vec![
            usage("Chrome", 4_000_000_000, None),
            usage("Zen", 700_000_000, Some(300_000_000)),
            usage("Codex", 500_000_000, Some(80_000_000)),
        ];
        let model =
            dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded {
                apps: AppSectionDisplay::Rows { culprit, rows },
                ..
            } => {
                let culprit = culprit.expect("culprit");
                assert_eq!(culprit.primary, "Likely culprit:");
                assert_eq!(culprit.tail.as_deref(), Some("Zen +300 MB"));
                assert_eq!(rows[0].primary, "Zen");
                assert_eq!(rows[0].tail.as_deref(), Some("0.7 GB  +300 MB"));
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
