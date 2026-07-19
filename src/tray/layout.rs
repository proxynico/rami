use super::style::APP_ROW_POOL;
use crate::format::{AppSectionDisplay, CpuDisplayState, DropdownModel, ModuleDisplay};
use crate::login_item::LaunchAtLoginStatus;
use crate::process_cpu::PROCESS_CPU_ROW_LIMIT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppShape {
    Hidden,
    Loading,
    Unavailable,
    Rows { rows: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CpuShape {
    Hidden,
    Loading,
    Unavailable,
    Available { cores: usize, processes: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GpuShape {
    Hidden,
    Available,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuShape {
    Uninitialized,
    Loading,
    Loaded {
        apps: AppShape,
        show_swap: bool,
        cpu: CpuShape,
        gpu: GpuShape,
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
                apps: app_shape,
                show_swap: memory.swap.is_some(),
                cpu: modules
                    .iter()
                    .find_map(|module| match module {
                        ModuleDisplay::Cpu(cpu) => Some(match &cpu.state {
                            CpuDisplayState::Loading => CpuShape::Loading,
                            CpuDisplayState::Unavailable => CpuShape::Unavailable,
                            CpuDisplayState::Available {
                                cores, processes, ..
                            } => CpuShape::Available {
                                cores: cores.len().min(2),
                                processes: processes.len().min(PROCESS_CPU_ROW_LIMIT),
                            },
                        }),
                        ModuleDisplay::Memory(_) | ModuleDisplay::Gpu(_) => None,
                    })
                    .unwrap_or(CpuShape::Hidden),
                gpu: if modules
                    .iter()
                    .any(|module| matches!(module, ModuleDisplay::Gpu(_)))
                {
                    GpuShape::Available
                } else {
                    GpuShape::Hidden
                },
            }
        }
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum MenuEntry<'a> {
    ModuleTitle(&'a str),
    Rings {
        memory_percent: u8,
        pressure_percent: u8,
    },
    Legend {
        label: &'a str,
        value: &'a str,
        opacity_percent: u8,
    },
    Stat {
        primary: &'a str,
        tail: Option<&'a str>,
    },
    Loading,
    AppLoading,
    AppUnavailable,
    CpuLoading,
    CpuUnavailable,
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
            entries.push(MenuEntry::Rings {
                memory_percent: memory.rings[0].percent,
                pressure_percent: memory.rings[1].percent,
            });
            for row in &memory.breakdown {
                entries.push(MenuEntry::Legend {
                    label: &row.label,
                    value: &row.value,
                    opacity_percent: row.opacity_percent,
                });
            }
            if let Some(swap) = &memory.swap {
                entries.push(MenuEntry::Stat {
                    primary: &swap.primary,
                    tail: swap.tail.as_deref(),
                });
            }
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
            for module in modules.iter().skip(1) {
                match module {
                    ModuleDisplay::Memory(_) => {}
                    ModuleDisplay::Cpu(cpu) => {
                        entries.push(MenuEntry::Separator);
                        entries.push(MenuEntry::ModuleTitle("CPU"));
                        match &cpu.state {
                            CpuDisplayState::Loading => entries.push(MenuEntry::CpuLoading),
                            CpuDisplayState::Unavailable => entries.push(MenuEntry::CpuUnavailable),
                            CpuDisplayState::Available {
                                utilization,
                                cores,
                                processes,
                            } => {
                                for row in utilization {
                                    entries.push(MenuEntry::Legend {
                                        label: &row.label,
                                        value: &row.value,
                                        opacity_percent: row.opacity_percent,
                                    });
                                }
                                for row in cores {
                                    entries.push(MenuEntry::Stat {
                                        primary: &row.primary,
                                        tail: row.tail.as_deref(),
                                    });
                                }
                                for row in processes {
                                    entries.push(MenuEntry::Stat {
                                        primary: &row.primary,
                                        tail: row.tail.as_deref(),
                                    });
                                }
                            }
                        }
                    }
                    ModuleDisplay::Gpu(gpu) => {
                        entries.push(MenuEntry::Separator);
                        entries.push(MenuEntry::ModuleTitle("GPU"));
                        entries.push(MenuEntry::Stat {
                            primary: &gpu.utilization.primary,
                            tail: gpu.utilization.tail.as_deref(),
                        });
                    }
                }
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
        dropdown_model, dropdown_model_with_apps, dropdown_model_with_sections,
        placeholder_dropdown_model,
    };
    use crate::login_item::LaunchAtLoginStatus;
    use crate::model::{
        CpuModuleState, CpuSnapshot, GpuModuleState, GpuSnapshot, MemorySnapshot, PressureSource,
        SystemSnapshot,
    };
    use crate::process_cpu::{ProcessCpuSnapshot, ProcessCpuUsage};
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
                MenuEntry::Rings {
                    memory_percent: 47,
                    pressure_percent: 34,
                },
                MenuEntry::Legend {
                    label: "App Memory",
                    value: "4.0 GB",
                    opacity_percent: 100,
                },
                MenuEntry::Legend {
                    label: "Wired",
                    value: "1.0 GB",
                    opacity_percent: 65,
                },
                MenuEntry::Legend {
                    label: "Compressed",
                    value: "717 MB",
                    opacity_percent: 35,
                },
                MenuEntry::Legend {
                    label: "Free",
                    value: "2.0 GB",
                    opacity_percent: 12,
                },
                MenuEntry::Stat {
                    primary: "Swap",
                    tail: Some("1.2 GB"),
                },
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
    fn compact_menu_omits_decorative_history() {
        // ADR-0001 keeps the native dropdown bounded: memory is current state,
        // not a decorative history dashboard.
        let model = dropdown_model(snapshot());
        let entries = loaded_menu_entries(&model);
        assert!(matches!(
            &entries[..6],
            [
                MenuEntry::Rings { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Legend { .. },
                MenuEntry::Stat {
                    primary: "Swap",
                    ..
                },
            ]
        ));
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
        assert_eq!(entries[6], MenuEntry::Separator);
        assert_eq!(entries[7], MenuEntry::AppLoading);
    }

    #[test]
    fn loaded_with_apps_unavailable_renders_one_row() {
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Unavailable);
        let entries = loaded_menu_entries(&model);
        assert_eq!(entries[6], MenuEntry::Separator);
        assert_eq!(entries[7], MenuEntry::AppUnavailable);
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
                pids: vec![1],
                delta_bytes: None,
            },
            AppMemoryUsage {
                name: "Chrome".to_string(),
                group_key: "/Applications/Chrome.app".to_string(),
                footprint_bytes: 1_288_490_189,
                pids: vec![2],
                delta_bytes: None,
            },
        ];
        let model = dropdown_model_with_apps(snapshot(), &AppMemorySnapshot::Loaded(usage));
        let entries = loaded_menu_entries(&model);

        assert!(matches!(entries[0], MenuEntry::Rings { .. }));
        assert!(matches!(
            entries[1],
            MenuEntry::Legend {
                label: "App Memory",
                ..
            }
        ));
        assert!(matches!(
            entries[5],
            MenuEntry::Stat {
                primary: "Swap",
                ..
            }
        ));
        assert_eq!(entries[6], MenuEntry::Separator);
        assert_eq!(
            entries[7],
            MenuEntry::AppRow {
                primary: "Cursor",
                tail: Some("2.0 GB"),
            }
        );
        assert_eq!(
            entries[8],
            MenuEntry::AppRow {
                primary: "Chrome",
                tail: Some("1.2 GB"),
            }
        );
        assert_eq!(entries[9], MenuEntry::Separator);
    }

    #[test]
    fn loaded_cpu_module_follows_memory_with_shared_legend_and_core_rows() {
        let mut snapshot = snapshot();
        snapshot.cpu = CpuModuleState::Available(CpuSnapshot {
            user_percent: 42,
            system_percent: 9,
            efficiency_percent: Some(18),
            performance_percent: Some(74),
        });
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model);

        assert_eq!(entries[6], MenuEntry::Separator);
        assert_eq!(entries[7], MenuEntry::ModuleTitle("CPU"));
        assert_eq!(
            entries[8],
            MenuEntry::Legend {
                label: "User",
                value: "42%",
                opacity_percent: 100,
            }
        );
        assert_eq!(
            entries[9],
            MenuEntry::Legend {
                label: "System",
                value: "9%",
                opacity_percent: 50,
            }
        );
        assert_eq!(
            entries[10],
            MenuEntry::Stat {
                primary: "E-cores",
                tail: Some("18%"),
            }
        );
        assert_eq!(
            entries[11],
            MenuEntry::Stat {
                primary: "P-cores",
                tail: Some("74%"),
            }
        );
    }

    #[test]
    fn loaded_cpu_process_rows_follow_the_cpu_overview_without_app_actions() {
        let mut snapshot = snapshot();
        snapshot.cpu = CpuModuleState::Available(CpuSnapshot {
            user_percent: 42,
            system_percent: 9,
            efficiency_percent: None,
            performance_percent: None,
        });
        let processes = ProcessCpuSnapshot::Loaded(vec![ProcessCpuUsage {
            name: "Video Encoder".to_string(),
            utilization_percent: 240,
        }]);
        let model = dropdown_model_with_sections(snapshot, &AppMemorySnapshot::Hidden, &processes);
        let entries = loaded_menu_entries(&model);

        assert_eq!(
            entries[10],
            MenuEntry::Stat {
                primary: "Video Encoder",
                tail: Some("240%"),
            }
        );
        assert!(!entries.iter().any(|entry| matches!(
            entry,
            MenuEntry::AppRow {
                primary: "Video Encoder",
                ..
            }
        )));
    }

    #[test]
    fn loaded_gpu_module_follows_existing_modules_with_one_utilization_row() {
        let mut snapshot = snapshot();
        snapshot.gpu = GpuModuleState::Available(GpuSnapshot {
            utilization_percent: 76,
        });
        let model = dropdown_model(snapshot);
        let entries = loaded_menu_entries(&model);

        assert_eq!(entries[6], MenuEntry::Separator);
        assert_eq!(entries[7], MenuEntry::ModuleTitle("GPU"));
        assert_eq!(
            entries[8],
            MenuEntry::Stat {
                primary: "Utilization",
                tail: Some("76%"),
            }
        );
    }
}
