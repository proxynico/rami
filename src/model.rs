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

/// Coarse memory-pressure level derived from the reclaimable pool. This is a
/// proxy (available bytes vs. total) rather than the kernel's
/// `dispatch_source` memory-pressure events, so it tracks "am I nearly out of
/// RAM" without new bindings; the gauge tints red/orange accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Normal,
    Warning,
    Critical,
}

const CRITICAL_AVAILABLE_PCT: f64 = 0.05;
const WARNING_AVAILABLE_PCT: f64 = 0.12;

pub fn classify_pressure(available_bytes: u64, total_bytes: u64) -> MemoryPressure {
    if total_bytes == 0 {
        return MemoryPressure::Normal;
    }
    let avail_pct = available_bytes as f64 / total_bytes as f64;
    if avail_pct <= CRITICAL_AVAILABLE_PCT {
        MemoryPressure::Critical
    } else if avail_pct <= WARNING_AVAILABLE_PCT {
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
        assert_eq!(
            classify_pressure(8_000_000_000, 16_000_000_000),
            MemoryPressure::Normal
        );
    }

    #[test]
    fn warning_when_available_drops_below_threshold() {
        // 10% available -> warning band
        assert_eq!(
            classify_pressure(1_600_000_000, 16_000_000_000),
            MemoryPressure::Warning
        );
    }

    #[test]
    fn critical_when_available_nearly_exhausted() {
        assert_eq!(
            classify_pressure(500_000_000, 16_000_000_000),
            MemoryPressure::Critical
        );
    }

    #[test]
    fn normal_when_total_unknown() {
        assert_eq!(classify_pressure(0, 0), MemoryPressure::Normal);
    }

    #[test]
    fn bands_classify_by_available_share() {
        // 4% -> critical, 10% -> warning, 13% -> normal (avoid exact floating
        // point thresholds so the boundaries stay stable).
        assert_eq!(
            classify_pressure(640_000_000, 16_000_000_000),
            MemoryPressure::Critical
        );
        assert_eq!(
            classify_pressure(1_600_000_000, 16_000_000_000),
            MemoryPressure::Warning
        );
        assert_eq!(
            classify_pressure(2_080_000_000, 16_000_000_000),
            MemoryPressure::Normal
        );
    }
}
