use rami::format::{
    dropdown_rows, gb_text, menu_bar_icon, menu_bar_text, placeholder_dropdown_rows,
    placeholder_text,
};
use rami::model::{MemoryPressure, MemorySnapshot};

#[test]
fn menu_bar_text_uses_integer_percent() {
    assert_eq!(menu_bar_text(53), "53%");
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
fn menu_bar_icon_is_quiet_when_pressure_is_normal() {
    assert_eq!(menu_bar_icon(MemoryPressure::Normal), "▣");
}

#[test]
fn menu_bar_icon_warns_when_pressure_is_elevated_or_high() {
    assert_eq!(menu_bar_icon(MemoryPressure::Elevated), "!");
    assert_eq!(menu_bar_icon(MemoryPressure::High), "!");
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
