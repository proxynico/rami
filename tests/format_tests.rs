use rami::format::{dropdown_rows, gib_text, menu_bar_text, placeholder_text};
use rami::model::MemorySnapshot;

#[test]
fn menu_bar_text_uses_integer_percent() {
    assert_eq!(menu_bar_text(53), "53%");
}

#[test]
fn placeholder_is_double_dash_percent() {
    assert_eq!(placeholder_text(), "--%");
}

#[test]
fn gib_text_rounds_to_one_decimal_place() {
    assert_eq!(gib_text(9_019_437_056), "9.0 GB");
}

#[test]
fn dropdown_rows_include_ram_values_and_actions() {
    let snapshot = MemorySnapshot {
        used_bytes: 9_019_437_056,
        total_bytes: 17_179_869_184,
        used_percent: 53,
    };

    let rows = dropdown_rows(snapshot, None);

    assert_eq!(rows.ram_used, "RAM Used: 9.0 GB");
    assert_eq!(rows.ram_total, "RAM Total: 17.2 GB");
    assert_eq!(rows.temperature, None);
    assert_eq!(rows.refresh, "Refresh");
    assert_eq!(rows.quit, "Quit");
}

#[test]
fn dropdown_rows_include_temperature_only_when_present() {
    let snapshot = MemorySnapshot {
        used_bytes: 9_019_437_056,
        total_bytes: 17_179_869_184,
        used_percent: 53,
    };

    let rows = dropdown_rows(snapshot, Some(58));

    assert_eq!(rows.temperature.as_deref(), Some("CPU Temp: 58 C"));
}
