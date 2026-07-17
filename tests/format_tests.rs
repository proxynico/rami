use rami::format::{
    dropdown_model, gauge_symbol_name, gb_pair, gb_text, mem_text, placeholder_dropdown_model,
    Accent, DropdownModel, ModuleDisplay,
};
use rami::model::{CpuModuleState, MemorySnapshot, PressureSource, SystemSnapshot};

const ONE_GIB: u64 = 1_073_741_824;
const SIXTEEN_GIB: u64 = 16 * ONE_GIB;

#[test]
fn gauge_symbol_name_returns_expected_variant_for_each_bucket() {
    let cases = [
        (0_u8, "gauge.with.dots.needle.0percent"),
        (19, "gauge.with.dots.needle.0percent"),
        (20, "gauge.with.dots.needle.33percent"),
        (39, "gauge.with.dots.needle.33percent"),
        (40, "gauge.with.dots.needle.50percent"),
        (59, "gauge.with.dots.needle.50percent"),
        (60, "gauge.with.dots.needle.67percent"),
        (79, "gauge.with.dots.needle.67percent"),
        (80, "gauge.with.dots.needle.100percent"),
        (100, "gauge.with.dots.needle.100percent"),
    ];
    for (percent, expected) in cases {
        assert_eq!(gauge_symbol_name(percent), expected, "percent {percent}");
    }
}

#[test]
fn gb_text_rounds_to_one_decimal_place() {
    assert_eq!(gb_text(9 * ONE_GIB), "9.0 GB");
}

#[test]
fn mem_text_at_gib_boundary_uses_mb_then_gb() {
    assert_eq!(mem_text(ONE_GIB - 1), "1024 MB");
    assert_eq!(mem_text(ONE_GIB), "1.0 GB");
}

#[test]
fn gb_pair_renders_used_over_total() {
    assert_eq!(gb_pair(6_120_328_397, SIXTEEN_GIB), "5.7 / 16.0 GB");
}

#[test]
fn placeholder_model_is_loading() {
    assert_eq!(placeholder_dropdown_model(), DropdownModel::Loading);
}

#[test]
fn dropdown_model_splits_memory_and_swap_rows() {
    let snapshot = MemorySnapshot {
        used_bytes: 9 * ONE_GIB,
        total_bytes: SIXTEEN_GIB,
        used_percent: 53,
        pressure_percent: 96,
        pressure_source: PressureSource::Kernel,
        app_memory_bytes: 6 * ONE_GIB,
        wired_bytes: 2 * ONE_GIB,
        compressed_bytes: ONE_GIB,
        free_bytes: 3 * ONE_GIB,
        swap_used_bytes: 4_724_461_226,
        available_bytes: 8_160_449_024,
    };

    let DropdownModel::Loaded { accent, modules } = dropdown_model(SystemSnapshot {
        memory: snapshot,
        cpu: CpuModuleState::Disabled,
    }) else {
        panic!("expected Loaded model");
    };
    let ModuleDisplay::Memory(memory) = &modules[0] else {
        panic!("expected memory module first");
    };

    assert_eq!(accent, Accent::Critical);
    assert_eq!(memory.rings[0].label, "Memory %");
    assert_eq!(memory.rings[0].percent, 53);
    assert_eq!(memory.rings[1].label, "Pressure");
    assert_eq!(memory.rings[1].percent, 96);
    assert_eq!(memory.breakdown[0].label, "App Memory");
    assert_eq!(memory.breakdown[0].value, "6.0 GB");
    assert_eq!(memory.breakdown[0].opacity_percent, 100);
    assert_eq!(memory.breakdown[1].label, "Wired");
    assert_eq!(memory.breakdown[1].opacity_percent, 65);
    assert_eq!(memory.breakdown[2].label, "Compressed");
    assert_eq!(memory.breakdown[2].opacity_percent, 35);
    assert_eq!(memory.breakdown[3].label, "Free");
    assert_eq!(memory.breakdown[3].opacity_percent, 12);
    let swap = memory.swap.as_ref().expect("swap row present when nonzero");
    assert_eq!(swap.primary, "Swap");
    assert_eq!(swap.tail.as_deref(), Some("4.4 GB"));
}

#[test]
fn dropdown_model_hides_swap_when_zero() {
    let snapshot = MemorySnapshot {
        used_bytes: 5 * ONE_GIB,
        total_bytes: SIXTEEN_GIB,
        used_percent: 31,
        pressure_percent: 20,
        pressure_source: PressureSource::Kernel,
        app_memory_bytes: 3 * ONE_GIB,
        wired_bytes: ONE_GIB,
        compressed_bytes: ONE_GIB,
        free_bytes: 4 * ONE_GIB,
        swap_used_bytes: 0,
        available_bytes: 11 * ONE_GIB,
    };

    let DropdownModel::Loaded { modules, .. } = dropdown_model(SystemSnapshot {
        memory: snapshot,
        cpu: CpuModuleState::Disabled,
    }) else {
        panic!("expected Loaded model");
    };
    let ModuleDisplay::Memory(memory) = &modules[0] else {
        panic!("expected memory module first");
    };

    assert!(memory.swap.is_none());
}
