use crate::process_memory::AppMemoryUsage;
use std::collections::VecDeque;

const RISING_BYTES: i64 = 300_000_000;
const RISING_FAST_BYTES: i64 = 1_000_000_000;
const MEMORY_WINDOW_SAMPLES: usize = 25;
pub(crate) const MEANINGFUL_APP_DELTA_BYTES: i64 = 50_000_000;

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

    pub fn record(&mut self, used_bytes: u64) -> MemoryTrend {
        self.samples.push_back(used_bytes);
        while self.samples.len() > MEMORY_WINDOW_SAMPLES {
            self.samples.pop_front();
        }

        let Some(oldest) = self.samples.front() else {
            return MemoryTrend::Stable;
        };
        let delta = used_bytes as i64 - *oldest as i64;
        classify_memory_trend(delta)
    }

    /// Snapshot of the rolling window, oldest first. Used to draw the
    /// menu sparkline.
    pub fn samples(&self) -> Vec<u64> {
        self.samples.iter().copied().collect()
    }
}

pub fn classify_memory_trend(delta_bytes: i64) -> MemoryTrend {
    if delta_bytes >= RISING_FAST_BYTES {
        MemoryTrend::RisingFast
    } else if delta_bytes >= RISING_BYTES {
        MemoryTrend::Rising
    } else {
        MemoryTrend::Stable
    }
}

pub fn app_rows_with_deltas(
    current: Vec<AppMemoryUsage>,
    previous: &[AppMemoryUsage],
) -> Vec<AppMemoryUsage> {
    let mut rows: Vec<AppMemoryUsage> = current
        .into_iter()
        .map(|mut row| {
            row.delta_bytes = previous
                .iter()
                .find(|prev| prev.group_key == row.group_key)
                .map(|prev| row.footprint_bytes as i64 - prev.footprint_bytes as i64);
            row
        })
        .collect();
    rank_app_rows(&mut rows);
    rows
}

pub fn rank_app_rows(rows: &mut [AppMemoryUsage]) {
    let has_meaningful_delta = rows
        .iter()
        .any(|row| row.delta_bytes.unwrap_or(0) >= MEANINGFUL_APP_DELTA_BYTES);
    rows.sort_by(|a, b| {
        if has_meaningful_delta {
            b.delta_bytes
                .unwrap_or(0)
                .max(0)
                .cmp(&a.delta_bytes.unwrap_or(0).max(0))
                .then_with(|| b.footprint_bytes.cmp(&a.footprint_bytes))
                .then_with(|| a.name.cmp(&b.name))
        } else {
            b.footprint_bytes
                .cmp(&a.footprint_bytes)
                .then_with(|| a.name.cmp(&b.name))
        }
    });
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

    #[test]
    fn app_delta_ranking_prefers_meaningful_positive_delta() {
        let previous = vec![
            usage("chrome", "Chrome", 4_000_000_000),
            usage("zen", "Zen", 400_000_000),
        ];
        let current = vec![
            usage("chrome", "Chrome", 4_100_000_000),
            usage("zen", "Zen", 700_000_000),
        ];
        let ranked = app_rows_with_deltas(current, &previous);
        assert_eq!(ranked[0].name, "Zen");
        assert_eq!(ranked[0].delta_bytes, Some(300_000_000));
        assert_eq!(ranked[1].name, "Chrome");
    }

    fn usage(group_key: &str, name: &str, footprint_bytes: u64) -> AppMemoryUsage {
        AppMemoryUsage {
            name: name.to_string(),
            group_key: group_key.to_string(),
            footprint_bytes,
            pids: vec![],
            can_quit: true,
            delta_bytes: None,
        }
    }
}
