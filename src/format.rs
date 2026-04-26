use crate::model::{MemoryPressure, MemorySnapshot};

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

pub fn gb_pair(used_bytes: u64, total_bytes: u64) -> String {
    let used = used_bytes as f64 / 1_000_000_000_f64;
    let total = total_bytes as f64 / 1_000_000_000_f64;
    format!("{used:.1} / {total:.1} GB")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatRow {
    pub primary: String,
    pub tail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureDisplay {
    pub text: String,
    pub is_high: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownModel {
    Loading,
    Loaded {
        memory: StatRow,
        pressure: PressureDisplay,
        swap: StatRow,
    },
}

fn pressure_text(p: MemoryPressure) -> &'static str {
    match p {
        MemoryPressure::Normal => "Normal",
        MemoryPressure::Elevated => "Elevated",
        MemoryPressure::High => "High",
    }
}

pub fn dropdown_model(snapshot: MemorySnapshot) -> DropdownModel {
    DropdownModel::Loaded {
        memory: StatRow {
            primary: format!("{}%", snapshot.used_percent),
            tail: Some(gb_pair(snapshot.used_bytes, snapshot.total_bytes)),
        },
        pressure: PressureDisplay {
            text: pressure_text(snapshot.pressure).to_string(),
            is_high: matches!(snapshot.pressure, MemoryPressure::High),
        },
        swap: StatRow {
            primary: gb_text(snapshot.swap_used_bytes),
            tail: None,
        },
    }
}

pub fn placeholder_dropdown_model() -> DropdownModel {
    DropdownModel::Loading
}

#[cfg(test)]
mod tests {
    use super::*;

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
