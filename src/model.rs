#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub used_percent: u8,
    pub pressure_percent: u8,
    pub pressure_source: PressureSource,
    pub app_memory_bytes: u64,
    pub wired_bytes: u64,
    pub compressed_bytes: u64,
    pub free_bytes: u64,
    pub swap_used_bytes: u64,
    /// Reclaimable pool: free + inactive + speculative + purgeable pages.
    /// Like `used_bytes`, this is a simple approximation that can drift from
    /// Activity Monitor's "Memory Available" by a few percent.
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemSnapshot {
    pub memory: MemorySnapshot,
    pub cpu: CpuModuleState,
    pub gpu: GpuModuleState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuSnapshot {
    pub user_percent: u8,
    pub system_percent: u8,
    pub idle_percent: u8,
    pub efficiency_percent: Option<u8>,
    pub performance_percent: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuModuleState {
    Disabled,
    Loading,
    Available(CpuSnapshot),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuSnapshot {
    pub utilization_percent: u8,
    pub renderer_percent: Option<u8>,
    pub tiler_percent: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuModuleState {
    Disabled,
    Available(GpuSnapshot),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureSource {
    Kernel,
    AvailableFallback,
}

/// Coarse pressure state used by both the status gauge and dropdown Accent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Normal,
    Warning,
    Critical,
}

pub const CRITICAL_PRESSURE_PCT: u8 = 95;
pub const WARNING_PRESSURE_PCT: u8 = 88;

pub fn classify_pressure(pressure_percent: u8) -> MemoryPressure {
    if pressure_percent >= CRITICAL_PRESSURE_PCT {
        MemoryPressure::Critical
    } else if pressure_percent >= WARNING_PRESSURE_PCT {
        MemoryPressure::Warning
    } else {
        MemoryPressure::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_when_plenty_available() {
        assert_eq!(classify_pressure(50), MemoryPressure::Normal);
    }

    #[test]
    fn warning_when_available_drops_below_threshold() {
        // 10% available -> warning band
        assert_eq!(classify_pressure(90), MemoryPressure::Warning);
    }

    #[test]
    fn critical_when_available_nearly_exhausted() {
        assert_eq!(classify_pressure(97), MemoryPressure::Critical);
    }

    #[test]
    fn normal_when_pressure_is_zero() {
        assert_eq!(classify_pressure(0), MemoryPressure::Normal);
    }

    #[test]
    fn bands_classify_by_pressure_percent() {
        assert_eq!(classify_pressure(95), MemoryPressure::Critical);
        assert_eq!(classify_pressure(88), MemoryPressure::Warning);
        assert_eq!(classify_pressure(87), MemoryPressure::Normal);
    }
}
