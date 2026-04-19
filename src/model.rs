#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub used_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownRows {
    pub ram_used: String,
    pub ram_total: String,
    pub temperature: Option<String>,
    pub refresh: String,
    pub quit: String,
}
