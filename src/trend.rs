use std::collections::VecDeque;

const RISING_BYTES: i64 = 300_000_000;
const RISING_FAST_BYTES: i64 = 1_000_000_000;
const MEMORY_WINDOW_SAMPLES: usize = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTrend {
    Stable,
    Rising,
    RisingFast,
}

#[derive(Debug, Default)]
pub struct MemoryTrendTracker {
    samples: VecDeque<u64>,
}

impl MemoryTrendTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// The raw sample window, oldest first — the memory-history sparkline
    /// draws exactly this. Kept at [`MEMORY_WINDOW_SAMPLES`]: enlarging it
    /// would also change what the trend classifies over.
    pub fn window(&self) -> Vec<u64> {
        self.samples.iter().copied().collect()
    }

    pub fn record(&mut self, used_bytes: u64) -> MemoryTrend {
        self.samples.push_back(used_bytes);
        while self.samples.len() > MEMORY_WINDOW_SAMPLES {
            self.samples.pop_front();
        }
        self.classify(used_bytes)
    }

    /// The trend of `used_bytes` against the oldest sample, without adding
    /// it to the window. Stable while the window is empty.
    pub fn classify(&self, used_bytes: u64) -> MemoryTrend {
        match self.samples.front() {
            Some(&oldest) => classify_memory_trend(used_bytes as i64 - oldest as i64),
            None => MemoryTrend::Stable,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

fn classify_memory_trend(delta_bytes: i64) -> MemoryTrend {
    if delta_bytes >= RISING_FAST_BYTES {
        MemoryTrend::RisingFast
    } else if delta_bytes >= RISING_BYTES {
        MemoryTrend::Rising
    } else {
        MemoryTrend::Stable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_two_minute_memory_trend_by_byte_thresholds() {
        assert_eq!(classify_memory_trend(299_999_999), MemoryTrend::Stable);
        assert_eq!(classify_memory_trend(300_000_000), MemoryTrend::Rising);
        assert_eq!(classify_memory_trend(999_999_999), MemoryTrend::Rising);
        assert_eq!(
            classify_memory_trend(1_000_000_000),
            MemoryTrend::RisingFast
        );
        assert_eq!(classify_memory_trend(-2_000_000_000), MemoryTrend::Stable);
    }

    #[test]
    fn trend_tracker_uses_oldest_sample_in_two_minute_window() {
        let mut tracker = MemoryTrendTracker::new();
        assert_eq!(tracker.record(1_000_000_000), MemoryTrend::Stable);
        for _ in 0..23 {
            assert_eq!(tracker.record(1_100_000_000), MemoryTrend::Stable);
        }
        assert_eq!(tracker.record(1_350_000_000), MemoryTrend::Rising);
    }
}
