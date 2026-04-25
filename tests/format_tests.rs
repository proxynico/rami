use rami::format::{dropdown_rows, gauge_symbol_name, gb_text, placeholder_dropdown_rows};
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
fn placeholder_dropdown_rows_match_the_v2_menu_shape() {
    let rows = placeholder_dropdown_rows();

    assert_eq!(rows.ram_summary, "RAM: --% — 0.0 GB of 0.0 GB");
    assert_eq!(rows.memory_pressure, "Memory Pressure: Normal");
    assert_eq!(rows.swap_used, "Swap Used: 0.0 GB");
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
fn dropdown_rows_include_pressure_and_swap_usage() {
    let snapshot = MemorySnapshot {
        used_bytes: 9_019_437_056,
        total_bytes: 17_179_869_184,
        used_percent: 53,
        pressure: MemoryPressure::Elevated,
        swap_used_bytes: 4_414_120_000,
    };

    let rows = dropdown_rows(snapshot);

    assert_eq!(rows.ram_summary, "RAM: 53% — 9.0 GB of 17.2 GB");
    assert_eq!(rows.memory_pressure, "Memory Pressure: Elevated");
    assert_eq!(rows.swap_used, "Swap Used: 4.4 GB");
    assert_eq!(rows.refresh, "Refresh");
    assert_eq!(rows.quit, "Quit");
}
