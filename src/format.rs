use crate::model::MemorySnapshot;
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};

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

pub fn gb_text(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000_f64;
    format!("{gb:.1} GB")
}

pub fn gb_pair(used_bytes: u64, total_bytes: u64) -> String {
    let used = used_bytes as f64 / 1_000_000_000_f64;
    let total = total_bytes as f64 / 1_000_000_000_f64;
    format!("{used:.1} / {total:.1} GB")
}

/// Hover tooltip for the menu-bar gauge, e.g. "47% · 7.2 / 16.0 GB".
pub fn gauge_tooltip(used_percent: u8, used_bytes: u64, total_bytes: u64) -> String {
    format!("{used_percent}% · {}", gb_pair(used_bytes, total_bytes))
}

/// VoiceOver label for the menu-bar gauge, e.g. "Memory 47 percent, 7.2 of 16.0 GB used".
pub fn gauge_accessibility_label(used_percent: u8, used_bytes: u64, total_bytes: u64) -> String {
    let used = used_bytes as f64 / 1_000_000_000_f64;
    let total = total_bytes as f64 / 1_000_000_000_f64;
    format!("Memory {used_percent} percent, {used:.1} of {total:.1} GB used")
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

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownModel {
    Loading,
    Loaded {
        memory: StatRow,
        available: StatRow,
        apps: AppSectionDisplay,
        swap: Option<StatRow>,
    },
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
            quit_key: None,
            bundle_path: None,
        },
        available: StatRow {
            primary: "Available".to_string(),
            tail: Some(mem_text(snapshot.available_bytes)),
            quit_key: None,
            bundle_path: None,
        },
        apps: app_section_display(apps),
        swap: (snapshot.swap_used_bytes > 0).then(|| StatRow {
            primary: "Swap".to_string(),
            tail: Some(mem_text(snapshot.swap_used_bytes)),
            quit_key: None,
            bundle_path: None,
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
        // Rows arrive already ranked (and delta-tagged) from `trend::app_rows_with_deltas`,
        // which is the single source of ranking. Here we only project the top N to display rows.
        AppMemorySnapshot::Loaded(rows) => AppSectionDisplay::Rows {
            rows: rows.iter().take(APP_USAGE_ROW_LIMIT).map(app_row).collect(),
        },
    }
}

fn app_row(app: &AppMemoryUsage) -> StatRow {
    let tail = if let Some(delta) = app.delta_bytes.filter(|delta| *delta >= 50_000_000) {
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
    use crate::trend::rank_app_rows;

    fn snapshot(total_bytes: u64) -> MemorySnapshot {
        MemorySnapshot {
            used_bytes: total_bytes / 2,
            total_bytes,
            used_percent: 50,
            swap_used_bytes: 0,
            available_bytes: total_bytes / 2,
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
    fn gauge_tooltip_pairs_percent_with_used_over_total() {
        assert_eq!(
            gauge_tooltip(47, 7_200_000_000, 16_000_000_000),
            "47% · 7.2 / 16.0 GB"
        );
    }

    #[test]
    fn gauge_accessibility_label_is_spoken_friendly() {
        assert_eq!(
            gauge_accessibility_label(47, 7_200_000_000, 16_000_000_000),
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
                    assert_eq!(
                        rows[0].quit_key.as_deref(),
                        Some("/Applications/Cursor.app")
                    );
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
    fn dropdown_model_app_row_truncates_names_before_tail_column() {
        let usage = vec![AppMemoryUsage {
            name: "Codex Computer Use".to_string(),
            group_key: "/Applications/Codex Computer Use.app".to_string(),
            footprint_bytes: 81_000_000,
            pids: vec![1],
            can_quit: true,
            delta_bytes: None,
        }];
        let model =
            dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded { apps, .. } => match apps {
                AppSectionDisplay::Rows { rows, .. } => {
                    assert_eq!(rows[0].primary, "Codex Computer …");
                    assert_eq!(rows[0].tail.as_deref(), Some("81 MB"));
                }
                _ => panic!("expected Rows"),
            },
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn dropdown_model_memory_row_shows_percent_tail() {
        let snapshot = MemorySnapshot {
            used_bytes: 9_000_000_000,
            total_bytes: 16_000_000_000,
            used_percent: 56,
            swap_used_bytes: 0,
            available_bytes: 7_000_000_000,
        };
        let DropdownModel::Loaded { memory, .. } = dropdown_model(snapshot) else {
            panic!("expected Loaded model");
        };
        assert_eq!(memory.tail.as_deref(), Some("56%"));
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
        let mut usage = vec![
            usage("Chrome", 4_000_000_000, None),
            usage("Zen", 700_000_000, Some(300_000_000)),
            usage("Codex", 500_000_000, Some(80_000_000)),
        ];
        rank_app_rows(&mut usage);
        let model =
            dropdown_model_with_apps(snapshot(16_000_000_000), &AppMemorySnapshot::Loaded(usage));
        match model {
            DropdownModel::Loaded {
                apps: AppSectionDisplay::Rows { rows },
                ..
            } => {
                assert_eq!(rows[0].primary, "Zen");
                assert_eq!(rows[0].tail.as_deref(), Some("700 MB\t+300 MB"));
                assert_eq!(rows[0].quit_key.as_deref(), Some("/Applications/Zen.app"));
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
