use rami::format::{
    dropdown_model, gauge_symbol_name, gb_pair, gb_text, placeholder_dropdown_model, DropdownModel,
};
use rami::model::{MemoryPressure, MemorySnapshot};

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
    assert_eq!(gb_text(9_019_437_056), "9.0 GB");
}

#[test]
fn gb_text_rounds_decimal_boundary_to_one_gb() {
    assert_eq!(gb_text(999_999_999), "1.0 GB");
}

#[test]
fn gb_pair_renders_used_over_total() {
    assert_eq!(gb_pair(5_700_000_000, 16_000_000_000), "5.7 / 16.0 GB");
}

#[test]
fn placeholder_model_is_loading() {
    assert_eq!(placeholder_dropdown_model(), DropdownModel::Loading);
}

#[test]
fn dropdown_model_splits_memory_into_primary_and_tail() {
    let snapshot = MemorySnapshot {
        used_bytes: 9_019_437_056,
        total_bytes: 17_179_869_184,
        used_percent: 53,
        pressure: MemoryPressure::Elevated,
        swap_used_bytes: 4_414_120_000,
    };

    let DropdownModel::Loaded {
        memory,
        pressure,
        swap,
        ..
    } = dropdown_model(snapshot)
    else {
        panic!("expected Loaded model");
    };

    assert_eq!(memory.primary, "53%");
    assert_eq!(memory.tail.as_deref(), Some("9.0 / 17.2 GB"));
    assert_eq!(pressure.text, "Elevated");
    assert!(!pressure.is_high);
    assert!(pressure.is_elevated);
    let swap = swap.expect("swap row present when nonzero");
    assert_eq!(swap.primary, "Swap");
    assert_eq!(swap.tail.as_deref(), Some("4.4 GB"));
}

#[test]
fn dropdown_model_hides_swap_when_zero() {
    let snapshot = MemorySnapshot {
        used_bytes: 5_000_000_000,
        total_bytes: 16_000_000_000,
        used_percent: 31,
        pressure: MemoryPressure::Normal,
        swap_used_bytes: 0,
    };

    let DropdownModel::Loaded { swap, .. } = dropdown_model(snapshot) else {
        panic!("expected Loaded model");
    };

    assert!(swap.is_none());
}

#[test]
fn dropdown_model_marks_high_pressure_for_red_rendering() {
    let snapshot = MemorySnapshot {
        used_bytes: 14_000_000_000,
        total_bytes: 16_000_000_000,
        used_percent: 88,
        pressure: MemoryPressure::High,
        swap_used_bytes: 6_000_000_000,
    };

    let DropdownModel::Loaded { pressure, .. } = dropdown_model(snapshot) else {
        panic!("expected Loaded model");
    };

    assert_eq!(pressure.text, "High");
    assert!(pressure.is_high);
}
