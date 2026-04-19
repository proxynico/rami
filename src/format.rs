use crate::model::{DropdownRows, MemorySnapshot};

pub fn menu_bar_text(percent: u8) -> String {
    format!("{percent}%")
}

pub fn gib_text(bytes: u64) -> String {
    let gib = bytes as f64 / 1024_f64.powi(3);
    format!("{gib:.1} GB")
}

pub fn dropdown_rows(snapshot: MemorySnapshot, temperature_c: Option<i32>) -> DropdownRows {
    DropdownRows {
        ram_used: format!("RAM Used: {}", gib_text(snapshot.used_bytes)),
        ram_total: format!("RAM Total: {}", gib_text(snapshot.total_bytes)),
        temperature: temperature_c.map(|value| format!("CPU Temp: {value} C")),
        refresh: "Refresh".to_string(),
        quit: "Quit".to_string(),
    }
}

pub fn placeholder_text() -> String {
    "--%".to_string()
}
