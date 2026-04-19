#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Normal,
    Elevated,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub used_percent: u8,
    pub pressure: MemoryPressure,
    pub swap_used_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownRows {
    pub ram_used: String,
    pub ram_total: String,
    pub memory_pressure: String,
    pub swap_used: String,
    pub refresh: String,
    pub quit: String,
}
