//! The dropdown's order, as plain data: which rows a model needs and the
//! sequence `TrayController::rebuild_menu` adds them in. The menu is built
//! from `menu_slots`, so the tests below pin the real order.

use crate::format::{AppSectionDisplay, DropdownModel};

pub(super) const APP_ROW_POOL: usize = 3;
/// App Memory, Wired, Compressed, Other, Cached, Free.
pub(super) const BREAKDOWN_ROW_POOL: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppShape {
    Hidden,
    Loading,
    Unavailable,
    Rows(usize),
}

/// What the menu holds, without the values. The menu is rebuilt only when
/// this changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuShape {
    /// No memory reading yet, or the sample failed.
    Loading,
    Loaded {
        breakdown_rows: usize,
        apps: AppShape,
        show_swap: bool,
        show_cpu: bool,
        show_gpu: bool,
    },
}

/// One dropdown entry, in menu order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    Loading,
    Map,
    Legend(usize),
    Pressure,
    Swap,
    History,
    AppLoading,
    AppUnavailable,
    App(usize),
    Cpu,
    Gpu,
    Separator,
    Refresh,
    Settings,
    Quit,
}

pub(super) fn menu_shape_for(model: Option<&DropdownModel>) -> MenuShape {
    let Some(model) = model else {
        return MenuShape::Loading;
    };
    MenuShape::Loaded {
        breakdown_rows: model.breakdown.len().min(BREAKDOWN_ROW_POOL),
        apps: match &model.apps {
            AppSectionDisplay::Hidden => AppShape::Hidden,
            AppSectionDisplay::Loading => AppShape::Loading,
            AppSectionDisplay::Unavailable => AppShape::Unavailable,
            AppSectionDisplay::Rows { rows } => AppShape::Rows(rows.len().min(APP_ROW_POOL)),
        },
        show_swap: model.swap.is_some(),
        show_cpu: model.cpu.is_some(),
        show_gpu: model.gpu.is_some(),
    }
}

pub(super) fn menu_slots(shape: MenuShape) -> Vec<Slot> {
    let mut slots = Vec::new();
    match shape {
        MenuShape::Loading => slots.push(Slot::Loading),
        MenuShape::Loaded {
            breakdown_rows,
            apps,
            show_swap,
            show_cpu,
            show_gpu,
        } => {
            slots.push(Slot::Map);
            slots.extend((0..breakdown_rows).map(Slot::Legend));
            slots.push(Slot::Pressure);
            if show_swap {
                slots.push(Slot::Swap);
            }
            slots.push(Slot::History);
            match apps {
                AppShape::Hidden => {}
                AppShape::Loading => slots.extend([Slot::Separator, Slot::AppLoading]),
                AppShape::Unavailable => slots.extend([Slot::Separator, Slot::AppUnavailable]),
                AppShape::Rows(rows) => {
                    slots.push(Slot::Separator);
                    slots.extend((0..rows).map(Slot::App));
                }
            }
            if show_cpu || show_gpu {
                slots.push(Slot::Separator);
            }
            if show_cpu {
                slots.push(Slot::Cpu);
            }
            if show_gpu {
                slots.push(Slot::Gpu);
            }
        }
    }
    slots.extend([
        Slot::Separator,
        Slot::Refresh,
        Slot::Settings,
        Slot::Separator,
        Slot::Quit,
    ]);
    slots
}

#[cfg(test)]
mod tests {
    use super::Slot::*;
    use super::*;
    use crate::format::dropdown_model;
    use crate::model::{
        CpuModuleState, CpuSnapshot, GpuModuleState, GpuSnapshot, MemorySnapshot, PressureSource,
        SystemSnapshot,
    };
    use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};

    const COMMANDS: [Slot; 5] = [Separator, Refresh, Settings, Separator, Quit];

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
            gpu: GpuModuleState::Disabled,
        }
    }

    fn slots(snapshot: SystemSnapshot, apps: AppMemorySnapshot) -> Vec<Slot> {
        let model = dropdown_model(snapshot, &apps, &[]);
        menu_slots(menu_shape_for(Some(&model)))
    }

    /// The Memory section, which every loaded menu opens with: the map, five
    /// breakdown rows (Other rounds below 1% here), Pressure, Swap, history.
    const MEMORY: [Slot; 10] = [
        Map,
        Legend(0),
        Legend(1),
        Legend(2),
        Legend(3),
        Legend(4),
        Pressure,
        Swap,
        History,
        Separator,
    ];

    #[test]
    fn loading_menu_holds_only_the_loading_row_and_commands() {
        assert_eq!(
            menu_slots(menu_shape_for(None)),
            [&[Loading][..], &COMMANDS].concat()
        );
    }

    #[test]
    fn memory_section_ends_with_the_one_history_row() {
        assert_eq!(
            slots(snapshot(), AppMemorySnapshot::Hidden),
            [&MEMORY[..9], &COMMANDS].concat()
        );
    }

    #[test]
    fn swap_row_is_omitted_when_swap_is_zero() {
        let mut snapshot = snapshot();
        snapshot.memory.swap_used_bytes = 0;
        assert!(!slots(snapshot, AppMemorySnapshot::Hidden).contains(&Swap));
    }

    #[test]
    fn app_section_follows_memory_behind_a_separator() {
        assert_eq!(
            slots(snapshot(), AppMemorySnapshot::Loading),
            [&MEMORY[..], &[AppLoading], &COMMANDS].concat()
        );
        assert_eq!(
            slots(snapshot(), AppMemorySnapshot::Unavailable),
            [&MEMORY[..], &[AppUnavailable], &COMMANDS].concat()
        );

        let usage = |name: &str| AppMemoryUsage {
            name: name.to_string(),
            group_key: format!("/Applications/{name}.app"),
            footprint_bytes: 1_288_490_189,
            delta_bytes: None,
        };
        let apps = ["Cursor", "Chrome", "Zen", "Safari"].map(usage).to_vec();
        assert_eq!(
            slots(snapshot(), AppMemorySnapshot::Loaded(apps)),
            [&MEMORY[..], &[App(0), App(1), App(2)], &COMMANDS].concat(),
            "at most three app rows"
        );
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
        assert_eq!(
            slots(snapshot, AppMemorySnapshot::Hidden),
            [&MEMORY[..], &[Cpu, Gpu], &COMMANDS].concat()
        );
    }
}
