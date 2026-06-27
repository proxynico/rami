#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub used_percent: u8,
    pub swap_used_bytes: u64,
    /// Reclaimable pool: free + inactive + speculative + purgeable pages.
    /// Like `used_bytes`, this is a simple approximation that can drift from
    /// Activity Monitor's "Memory Available" by a few percent.
    pub available_bytes: u64,
}
