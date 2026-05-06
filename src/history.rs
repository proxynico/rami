use std::collections::VecDeque;

const HISTORY_CAPACITY: usize = 60;

#[derive(Debug, Default)]
pub struct MemoryHistory {
    samples: VecDeque<u8>,
}

impl MemoryHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, used_percent: u8) {
        self.samples.push_back(used_percent);
        while self.samples.len() > HISTORY_CAPACITY {
            self.samples.pop_front();
        }
    }

    pub fn samples(&self) -> Vec<u8> {
        self.samples.iter().copied().collect()
    }

    pub fn capacity(&self) -> usize {
        HISTORY_CAPACITY
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_keeps_at_most_capacity_samples() {
        let mut history = MemoryHistory::new();
        for value in 0u8..100 {
            history.record(value);
        }
        assert_eq!(history.samples().len(), HISTORY_CAPACITY);
        assert_eq!(*history.samples().first().unwrap(), 100 - HISTORY_CAPACITY as u8);
        assert_eq!(*history.samples().last().unwrap(), 99);
    }

    #[test]
    fn empty_history_reports_no_samples() {
        let history = MemoryHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.samples().is_empty());
    }

    #[test]
    fn record_grows_until_capacity_is_reached() {
        let mut history = MemoryHistory::new();
        for value in 0u8..5 {
            history.record(value);
        }
        assert_eq!(history.len(), 5);
        assert_eq!(history.samples(), vec![0, 1, 2, 3, 4]);
    }
}
