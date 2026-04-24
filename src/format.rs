use crate::model::{DropdownRows, MemoryPressure, MemorySnapshot};

pub fn menu_bar_text(percent: u8) -> String {
    format!("{} {percent}%", ram_meter(percent))
}

pub fn gb_text(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000_f64;
    format!("{gb:.1} GB")
}

pub fn menu_bar_icon(pressure: MemoryPressure) -> &'static str {
    match pressure {
        MemoryPressure::Normal => "",
        MemoryPressure::Elevated => "!",
        MemoryPressure::High => "!!",
    }
}

fn ram_meter(percent: u8) -> &'static str {
    match percent {
        0..=12 => "▁",
        13..=25 => "▂",
        26..=38 => "▃",
        39..=51 => "▄",
        52..=64 => "▅",
        65..=77 => "▆",
        _ => "▇",
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
        ram_summary: format!(
            "RAM: {} of {}",
            gb_text(snapshot.used_bytes),
            gb_text(snapshot.total_bytes)
        ),
        memory_pressure: format!("Memory Pressure: {}", pressure_text(snapshot.pressure)),
        swap_used: format!("Swap Used: {}", gb_text(snapshot.swap_used_bytes)),
        refresh: "Refresh".to_string(),
        quit: "Quit".to_string(),
    }
}

pub fn placeholder_dropdown_rows() -> DropdownRows {
    DropdownRows {
        ram_summary: "RAM: 0.0 GB of 0.0 GB".to_string(),
        memory_pressure: "Memory Pressure: Normal".to_string(),
        swap_used: "Swap Used: 0.0 GB".to_string(),
        refresh: "Refresh".to_string(),
        quit: "Quit".to_string(),
    }
}

pub fn placeholder_text() -> String {
    "--%".to_string()
}

pub fn menu_bar_tooltip(snapshot: MemorySnapshot) -> String {
    format!(
        "RAM {} ({})\nPressure: {}\nSwap: {}",
        menu_bar_text(snapshot.used_percent),
        gb_text(snapshot.used_bytes),
        pressure_text(snapshot.pressure),
        gb_text(snapshot.swap_used_bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::{menu_bar_icon, menu_bar_text, menu_bar_tooltip};
    use crate::model::MemoryPressure;

    #[test]
    fn menu_bar_text_includes_visual_meter() {
        assert_eq!(menu_bar_text(0), "▁ 0%");
        assert_eq!(menu_bar_text(67), "▆ 67%");
        assert_eq!(menu_bar_text(100), "▇ 100%");
    }

    #[test]
    fn pressure_icons_are_subtle_and_escalate() {
        assert_eq!(menu_bar_icon(MemoryPressure::Normal), "");
        assert_eq!(menu_bar_icon(MemoryPressure::Elevated), "!");
        assert_eq!(menu_bar_icon(MemoryPressure::High), "!!");
    }

    #[test]
    fn tooltip_contains_core_memory_details() {
        let tooltip = menu_bar_tooltip(crate::model::MemorySnapshot {
            used_bytes: 12_300_000_000,
            total_bytes: 24_600_000_000,
            used_percent: 50,
            pressure: MemoryPressure::Elevated,
            swap_used_bytes: 1_500_000_000,
        });
        assert!(tooltip.contains("RAM ▄ 50%"));
        assert!(tooltip.contains("Pressure: Elevated"));
        assert!(tooltip.contains("Swap: 1.5 GB"));
    }
}
