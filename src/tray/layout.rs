use super::style::{APP_ROW_POOL, BREAKDOWN_ROW_POOL};
#[cfg(test)]
use crate::format::Swatch;
use crate::format::{AppSectionDisplay, DropdownModel, ModuleDisplay};
use crate::login_item::LaunchAtLoginStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppShape {
    Hidden,
    Loading,
    Unavailable,
    Rows { rows: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuShape {
    Uninitialized,
    Loading,
    Loaded {
        breakdown_rows: usize,
        apps: AppShape,
        show_swap: bool,
        show_cpu: bool,
        show_gpu: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SettingsMenuProjection {
    pub(super) launch_at_login: LaunchAtLoginStatus,
    pub(super) auto_refresh_enabled: bool,
    pub(super) show_app_usage: bool,
    pub(super) show_cpu: bool,
    pub(super) show_gpu: bool,
}

pub(super) fn settings_menu_projection(
    launch_at_login: LaunchAtLoginStatus,
    auto_refresh_enabled: bool,
    show_app_usage: bool,
    show_cpu: bool,
    show_gpu: bool,
) -> SettingsMenuProjection {
    SettingsMenuProjection {
        launch_at_login,
        auto_refresh_enabled,
        show_app_usage,
        show_cpu,
        show_gpu,
    }
}

pub(super) fn menu_shape_for(model: &DropdownModel) -> MenuShape {
    match model {
        DropdownModel::Loading => MenuShape::Loading,
        DropdownModel::Loaded { modules, .. } => {
            let Some(ModuleDisplay::Memory(memory)) = modules.first() else {
                return MenuShape::Uninitialized;
            };
            let app_shape = match &memory.apps {
                AppSectionDisplay::Hidden => AppShape::Hidden,
                AppSectionDisplay::Loading => AppShape::Loading,
                AppSectionDisplay::Unavailable => AppShape::Unavailable,
                AppSectionDisplay::Rows { rows } => AppShape::Rows {
                    rows: rows.len().min(APP_ROW_POOL),
                },
            };
            MenuShape::Loaded {
                breakdown_rows: memory.breakdown.len().min(BREAKDOWN_ROW_POOL),
                apps: app_shape,
                show_swap: memory.swap.is_some(),
                show_cpu: modules
                    .iter()
                    .any(|module| matches!(module, ModuleDisplay::Cpu(_))),
                show_gpu: modules
                    .iter()
                    .any(|module| matches!(module, ModuleDisplay::Gpu(_))),
            }
        }
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum MenuEntry<'a> {
    Map {
        used_percent: u8,
        cells: usize,
    },
    Pressure {
        percent: u8,
    },
    History {
        samples: usize,
    },
    Legend {
        label: &'a str,
        value: &'a str,
        swatch: Swatch,
    },
    Stat {
        primary: &'a str,
        tail: Option<&'a str>,
    },
    Loading,
    AppLoading,
    AppUnavailable,
    AppRow {
        primary: &'a str,
        tail: Option<&'a str>,
    },
    Separator,
    Refresh {
        key_equivalent: Option<&'a str>,
    },
    SettingsCommand,
    Quit {
        key_equivalent: Option<&'a str>,
    },
}

#[cfg(test)]
fn loaded_menu_entries(model: &DropdownModel) -> Vec<MenuEntry<'_>> {
    let mut entries = Vec::new();
    match model {
        DropdownModel::Loading => {
            entries.push(MenuEntry::Loading);
        }
        DropdownModel::Loaded { modules, .. } => {
            let Some(ModuleDisplay::Memory(memory)) = modules.first() else {
                return entries;
            };
            entries.push(MenuEntry::Map {
                used_percent: memory.map.used_percent,
                cells: memory.map.cells.len(),
            });
            for row in &memory.breakdown {
                entries.push(MenuEntry::Legend {
                    label: &row.label,
                    value: &row.value,
                    swatch: row.swatch,
                });
            }
            entries.push(MenuEntry::Pressure {
                percent: memory.pressure_percent,
            });
            if let Some(swap) = &memory.swap {
                entries.push(MenuEntry::Stat {
                    primary: &swap.primary,
                    tail: swap.tail.as_deref(),
                });
            }
            entries.push(MenuEntry::History {
                samples: memory.history.len(),
            });
            match &memory.apps {
                AppSectionDisplay::Hidden => {}
                AppSectionDisplay::Loading => {
                    entries.push(MenuEntry::Separator);
                    entries.push(MenuEntry::AppLoading);
                }
                AppSectionDisplay::Unavailable => {
                    entries.push(MenuEntry::Separator);
                    entries.push(MenuEntry::AppUnavailable);
                }
                AppSectionDisplay::Rows { rows } => {
                    entries.push(MenuEntry::Separator);
                    for row in rows.iter().take(APP_ROW_POOL) {
                        entries.push(MenuEntry::AppRow {
                            primary: &row.primary,
                            tail: row.tail.as_deref(),
                        });
                    }
                }
            }
            let compact: Vec<_> = modules
                .iter()
                .filter_map(|module| match module {
                    ModuleDisplay::Memory(_) => None,
                    ModuleDisplay::Cpu(row) | ModuleDisplay::Gpu(row) => Some(row),
                })
                .collect();
            if !compact.is_empty() {
                entries.push(MenuEntry::Separator);
            }
            for row in compact {
                entries.push(MenuEntry::Stat {
                    primary: &row.primary,
                    tail: row.tail.as_deref(),
                });
            }
        }
    }
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Refresh {
        key_equivalent: None,
    });
    entries.push(MenuEntry::SettingsCommand);
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Quit {
        key_equivalent: None,
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::{loaded_menu_entries, settings_menu_projection, MenuEntry, SettingsMenuProjection};
    use crate::format::{
        dropdown_model, dropdown_model_with_apps, placeholder_dropdown_model, Swatch,
    };
    use crate::login_item::LaunchAtLoginStatus;
    use crate::model::{
        CpuModuleState, CpuSnapshot, GpuModuleState, GpuSnapshot, MemorySnapshot, PressureSource,
        SystemSnapshot,
    };
    use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};

    fn snapshot() -> SystemSnapshot {
        SystemSnapshot {
            memory: MemorySnapshot {
                used_bytes: 6_120_328_397,
                total_bytes: 17_179_869_184,
                used_percent: 47,
                pressure_percent: 34,
                pressure_source: PressureSource::Kernel,
                app_memory_bytes: 4_294_967_296,
                wired_bytes: 1_073_741_824,
                compressed_bytes: 751_619_276,
                free_bytes: 2_147_483_648,
                cached_bytes: 8_912_057_139,
                swap_used_bytes: 1_288_490_189,
                available_bytes: 11_055_540_777,
            },
            cpu: CpuModuleState::Disabled,
            gpu: crate::model::GpuModuleState::Disabled,
        }
    }

    #[test]
    fn loading_layout_omits_memory_detail_sections() {
        let model = placeholder_dropdown_model();
        let entries = loaded_menu_entries(&model);
        assert_eq!(
            entries,
            vec![
                MenuEntry::Loading,
                MenuEntry::Separator,
                MenuEntry::Refresh {
                    key_equivalent: None,
                },
                MenuEntry::SettingsCommand,
                MenuEntry::Separator,
                MenuEntry::Quit {
                    key_equivalent: None,
                },
            ]
        );
    }

    #[test]
    fn loaded_layout_renders_memory_and_swap_rows() {
        let model = dropdown_model(snapshot());
        let entries = loaded_menu_entries(&model);
        assert_eq!(
            entries,
            vec![
                MenuEntry::Map {
                    used_percent: 47,
                    cells: 64,
                },
                MenuEntry::Legend {
                    label: "App Memory",
                    value: "4.0 GB · 25%",
                    swatch: Swatch::Accent(100),
                },
                MenuEntry::Legend {
                    label: "Wired",
                    value: "1.0 GB · 6%",
                    swatch: Swatch::Accent(62),
                },
                MenuEntry::Legend {
                    label: "Compressed",
                    value: "717 MB · 4%",
                    swatch: Swatch::Accent(36),
                },
                MenuEntry::Legend {
                    label: "Cached",
                    value: "8.3 GB · 52%",
                    swatch: Swatch::Hatched,
                },
                MenuEntry::Legend {
                    label: "Free",
                    value: "2.0 GB · 13%",
                    swatch: Swatch::Empty,
                },
                MenuEntry::Pressure { percent: 34 },
                MenuEntry::Stat {
                    primary: "Swap",
                    tail: Some("1.2 GB"),
                },
                MenuEntry::History { samples: 0 },
                MenuEntry::Separator,
                MenuEntry::Refresh {
                    key_equivalent: None,
                },
                MenuEntry::SettingsCommand,
                MenuEntry::Separator,
                MenuEntry::Quit {
                    key_equivalent: None,
                },
            ]
        );
    }

    #[test]
    fn memory_history_closes_the_memory_section_and_is_the_only_history() {
        // One bounded memory-history row lives in the Memory module, after
        // the map, breakdown, pressure, and swap. No per-module histories, no
        // second graph.
        let model = dropdown_model(snapshot());
        let entries = loaded_menu_entries(&model);
        assert!(matches!(
            &entries[..9],
            [
                MenuEntry::Map { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Pressure { .. },
                MenuEntry::Stat {
                    primary: "Swap",
                    ..
                },
                MenuEntry::History { .. },
            ]
        ));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| matches!(entry, MenuEntry::History { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn loaded_layout_hides_swap_row_when_zero() {
        let mut snapshot = snapshot();
        snapshot.memory.swap_used_bytes = 0;
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model);
        assert!(!entries.iter().any(|e| matches!(
            e,
            MenuEntry::Stat {
                primary: "Swap",
                ..
            }
        )));
    }

    #[test]
    fn loaded_with_apps_hidden_omits_apps_section() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Hidden);
        let entries = loaded_menu_entries(&model);
        assert!(!entries.iter().any(|e| matches!(
            e,
            MenuEntry::AppRow { .. } | MenuEntry::AppLoading | MenuEntry::AppUnavailable
        )));
    }

    #[test]
    fn loaded_with_apps_loading_renders_loading_row() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Loading);
        let entries = loaded_menu_entries(&model);
        assert_eq!(entries[9], MenuEntry::Separator);
        assert_eq!(entries[10], MenuEntry::AppLoading);
    }

    #[test]
    fn loaded_with_apps_unavailable_renders_one_row() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Unavailable);
        let entries = loaded_menu_entries(&model);
        assert_eq!(entries[9], MenuEntry::Separator);
        assert_eq!(entries[10], MenuEntry::AppUnavailable);
    }

    #[test]
    fn show_app_usage_state_reflects_toggle() {
        let on = settings_menu_projection(LaunchAtLoginStatus::Disabled, true, true, false, false);
        assert!(on.show_app_usage);

        let off =
            settings_menu_projection(LaunchAtLoginStatus::Disabled, true, false, false, false);
        assert!(!off.show_app_usage);
    }

    #[test]
    fn module_visibility_states_are_independent_in_settings() {
        assert_eq!(
            settings_menu_projection(LaunchAtLoginStatus::Disabled, false, true, true, false),
            SettingsMenuProjection {
                auto_refresh_enabled: false,
                show_app_usage: true,
                show_cpu: true,
                show_gpu: false,
                launch_at_login: LaunchAtLoginStatus::Disabled,
            }
        );
    }

    #[test]
    fn loaded_with_apps_rows_follow_memory_section() {
        let usage = vec![
            AppMemoryUsage {
                name: "Cursor".to_string(),
                group_key: "/Applications/Cursor.app".to_string(),
                footprint_bytes: 2_147_483_648,
                delta_bytes: None,
            },
            AppMemoryUsage {
                name: "Chrome".to_string(),
                group_key: "/Applications/Chrome.app".to_string(),
                footprint_bytes: 1_288_490_189,
                delta_bytes: None,
            },
        ];
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Loaded(usage));
        let entries = loaded_menu_entries(&model);

        assert!(matches!(entries[0], MenuEntry::Map { .. }));
        assert!(matches!(
            entries[1],
            MenuEntry::Legend {
                label: "App Memory",
                ..
            }
        ));
        assert!(matches!(
            entries[7],
            MenuEntry::Stat {
                primary: "Swap",
                ..
            }
        ));
        assert!(matches!(entries[8], MenuEntry::History { .. }));
        assert_eq!(entries[9], MenuEntry::Separator);
        assert_eq!(
            entries[10],
            MenuEntry::AppRow {
                primary: "Cursor",
                tail: Some("2.0 GB"),
            }
        );
        assert_eq!(
            entries[11],
            MenuEntry::AppRow {
                primary: "Chrome",
                tail: Some("1.2 GB"),
            }
        );
        assert_eq!(entries[12], MenuEntry::Separator);
    }

    #[test]
    fn cpu_and_gpu_rows_share_one_separator_after_the_memory_section() {
        let mut snapshot = snapshot();
        snapshot.cpu = CpuModuleState::Available(CpuSnapshot {
            user_percent: 12,
            system_percent: 6,
        });
        snapshot.gpu = GpuModuleState::Available(GpuSnapshot {
            utilization_percent: 4,
            renderer_percent: Some(3),
            tiler_percent: None,
        });
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model);

        assert!(matches!(entries[8], MenuEntry::History { .. }));
        assert_eq!(entries[9], MenuEntry::Separator);
        assert_eq!(
            entries[10],
            MenuEntry::Stat {
                primary: "CPU",
                tail: Some("12 usr · 6 sys\t18%"),
            }
        );
        assert_eq!(
            entries[11],
            MenuEntry::Stat {
                primary: "GPU",
                tail: Some("render 3\t4%"),
            }
        );
        assert_eq!(entries[12], MenuEntry::Separator);
    }
}
