use rami::format::{
    dropdown_rows, gb_text, menu_bar_text, placeholder_dropdown_rows, pressure_tint, PressureTint,
    placeholder_text,
};
use rami::model::{MemoryPressure, MemorySnapshot};

#[test]
fn menu_bar_text_returns_percent_only() {
    for n in [0_u8, 19, 20, 53, 79, 80, 100] {
        assert_eq!(menu_bar_text(n), format!("{n}%"));
    }
}

#[test]
fn placeholder_is_double_dash_percent() {
    assert_eq!(placeholder_text(), "--%");
}

#[test]
fn placeholder_dropdown_rows_match_the_v2_menu_shape() {
    let rows = placeholder_dropdown_rows();

    assert_eq!(rows.ram_summary, "RAM: 0.0 GB of 0.0 GB");
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
fn pressure_tint_uses_template_for_normal_pressure() {
    assert_eq!(pressure_tint(MemoryPressure::Normal), PressureTint::Template);
}

#[test]
fn pressure_tint_warns_with_yellow_and_red() {
    assert_eq!(pressure_tint(MemoryPressure::Elevated), PressureTint::Yellow);
    assert_eq!(pressure_tint(MemoryPressure::High), PressureTint::Red);
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

    assert_eq!(rows.ram_summary, "RAM: 9.0 GB of 17.2 GB");
    assert_eq!(rows.memory_pressure, "Memory Pressure: Elevated");
    assert_eq!(rows.swap_used, "Swap Used: 4.4 GB");
    assert_eq!(rows.refresh, "Refresh");
    assert_eq!(rows.quit, "Quit");
}
