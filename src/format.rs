use crate::model::{DropdownRows, MemoryPressure, MemorySnapshot};

pub fn menu_bar_text(percent: u8) -> String {
    format!("{percent}%")
}

pub fn gb_text(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000_f64;
    format!("{gb:.1} GB")
}

pub fn menu_bar_icon(pressure: MemoryPressure) -> &'static str {
    match pressure {
        MemoryPressure::Normal => "▣",
        MemoryPressure::Elevated | MemoryPressure::High => "!",
    }
}

fn pressure_text(pressure: MemoryPressure) -> &'static str {
    match pressure {
        MemoryPressure::Normal => "Normal",
        MemoryPressure::Elevated => "Elevated",
        MemoryPressure::High => "High",
    }
}

pub fn dropdown_rows(snapshot: MemorySnapshot) -> DropdownRows {
    DropdownRows {
        ram_used: format!("RAM Used: {}", gb_text(snapshot.used_bytes)),
        ram_total: format!("RAM Total: {}", gb_text(snapshot.total_bytes)),
        memory_pressure: format!("Memory Pressure: {}", pressure_text(snapshot.pressure)),
        swap_used: format!("Swap Used: {}", gb_text(snapshot.swap_used_bytes)),
        refresh: "Refresh".to_string(),
        quit: "Quit".to_string(),
    }
}

pub fn placeholder_text() -> String {
    "--%".to_string()
}
