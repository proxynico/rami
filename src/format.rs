use crate::model::{MemoryPressure, MemorySnapshot};
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};

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
    Rows(Vec<StatRow>),
    Unavailable,
}

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
    dropdown_model_with_apps(snapshot, &AppMemorySnapshot::Hidden)
}

pub fn dropdown_model_with_apps(
    snapshot: MemorySnapshot,
    apps: &AppMemorySnapshot,
) -> DropdownModel {
    DropdownModel::Loaded {
        memory: StatRow {
            primary: format!("{}%", snapshot.used_percent),
            tail: Some(gb_pair(snapshot.used_bytes, snapshot.total_bytes)),
        },
        apps: app_section_display(apps, snapshot.total_bytes),
        pressure: PressureDisplay {
            text: pressure_text(snapshot.pressure).to_string(),
            is_high: matches!(snapshot.pressure, MemoryPressure::High),
        },
        swap: StatRow {
            primary: gb_text(snapshot.swap_used_bytes),
            tail: None,
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
            rows.sort_by(|a, b| {
                b.footprint_bytes
                    .cmp(&a.footprint_bytes)
                    .then_with(|| a.name.cmp(&b.name))
            });
            rows.truncate(APP_USAGE_ROW_LIMIT);
            AppSectionDisplay::Rows(rows.iter().map(|r| app_row(r, total_bytes)).collect())
        }
    }
}

fn app_row(app: &AppMemoryUsage, total_bytes: u64) -> StatRow {
    StatRow {
        primary: truncate_name(&app.name, APP_NAME_MAX_CHARS),
        tail: Some(format!(
            "{}  {}",
            gb_text(app.footprint_bytes),
            percent_label(app.footprint_bytes, total_bytes)
        )),
    }
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
            footprint_bytes: 2_000_000_000,
        }];
        let model =
            dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded { apps, .. } => match apps {
                AppSectionDisplay::Rows(rows) => {
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].primary, "Cursor");
                    assert_eq!(rows[0].tail.as_deref(), Some("2.0 GB  13%"));
                }
                _ => panic!("expected Rows"),
            },
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn dropdown_model_with_apps_keeps_top_five_sorted() {
        let usage = vec![
            AppMemoryUsage {
                name: "Six".to_string(),
                footprint_bytes: 6,
            },
            AppMemoryUsage {
                name: "One".to_string(),
                footprint_bytes: 1,
            },
            AppMemoryUsage {
                name: "Five".to_string(),
                footprint_bytes: 5,
            },
            AppMemoryUsage {
                name: "Two".to_string(),
                footprint_bytes: 2,
            },
            AppMemoryUsage {
                name: "Four".to_string(),
                footprint_bytes: 4,
            },
            AppMemoryUsage {
                name: "Three".to_string(),
                footprint_bytes: 3,
            },
        ];
        let model = dropdown_model_with_apps(snapshot(100), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded {
                apps: AppSectionDisplay::Rows(rows),
                ..
            } => {
                let names: Vec<_> = rows.iter().map(|row| row.primary.as_str()).collect();
                assert_eq!(names, vec!["Six", "Five", "Four", "Three", "Two"]);
            }
            _ => panic!("expected app rows"),
        }
    }
}
