use crate::model::{DropdownRows, MemorySnapshot};

pub fn menu_bar_text(percent: u8) -> String {
    format!("{percent}%")
}

pub fn gb_text(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000_f64;
    format!("{gb:.1} GB")
}

pub fn dropdown_rows(snapshot: MemorySnapshot, temperature_c: Option<i32>) -> DropdownRows {
    DropdownRows {
        ram_used: format!("RAM Used: {}", gb_text(snapshot.used_bytes)),
        ram_total: format!("RAM Total: {}", gb_text(snapshot.total_bytes)),
        temperature: temperature_c.map(|value| format!("CPU Temp: {value} C")),
        refresh: "Refresh".to_string(),
        quit: "Quit".to_string(),
    }
}

pub fn placeholder_text() -> String {
    "--%".to_string()
}
