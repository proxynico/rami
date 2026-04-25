use crate::model::{DropdownRows, MemoryPressure, MemorySnapshot};

pub fn gauge_symbol_name(percent: u8) -> &'static str {
    match percent {
        0..=19 => "gauge.with.dots.needle.0percent",
        20..=39 => "gauge.with.dots.needle.33percent",
        40..=59 => "gauge.with.dots.needle.50percent",
        60..=79 => "gauge.with.dots.needle.67percent",
        _ => "gauge.with.dots.needle.100percent",
    }
}

pub fn gb_text(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000_f64;
    format!("{gb:.1} GB")
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
            "RAM: {}% — {} of {}",
            snapshot.used_percent,
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
        ram_summary: "RAM: --% — 0.0 GB of 0.0 GB".to_string(),
        memory_pressure: "Memory Pressure: Normal".to_string(),
        swap_used: "Swap Used: 0.0 GB".to_string(),
        refresh: "Refresh".to_string(),
        quit: "Quit".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::gauge_symbol_name;

    #[test]
    fn gauge_symbol_name_buckets_by_percent() {
        assert_eq!(gauge_symbol_name(0), "gauge.with.dots.needle.0percent");
        assert_eq!(gauge_symbol_name(19), "gauge.with.dots.needle.0percent");
        assert_eq!(gauge_symbol_name(20), "gauge.with.dots.needle.33percent");
        assert_eq!(gauge_symbol_name(39), "gauge.with.dots.needle.33percent");
        assert_eq!(gauge_symbol_name(40), "gauge.with.dots.needle.50percent");
        assert_eq!(gauge_symbol_name(59), "gauge.with.dots.needle.50percent");
        assert_eq!(gauge_symbol_name(60), "gauge.with.dots.needle.67percent");
        assert_eq!(gauge_symbol_name(79), "gauge.with.dots.needle.67percent");
        assert_eq!(gauge_symbol_name(80), "gauge.with.dots.needle.100percent");
        assert_eq!(gauge_symbol_name(100), "gauge.with.dots.needle.100percent");
    }
}
