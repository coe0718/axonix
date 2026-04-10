//! Detection and analysis: types_newly_at_threshold, most_common_failure_type, count_by_type.

use super::types::FailureType;
use super::store::FailurePatternStore;

impl FailurePatternStore {
    /// Return failure type labels that are newly at or above `threshold` this session.
    ///
    /// A label is "newly" at threshold if its count has just reached `threshold`
    /// AND it has not been returned by a previous call to this method on the same
    /// store instance (tracked in `alerted_types`). Each label is returned at most
    /// once per store lifetime (i.e., per session).
    ///
    /// This is the hook for Telegram threshold alerts (G-118): call this after
    /// `log_failure` + `save`, then send one alert per returned label.
    pub fn types_newly_at_threshold(&mut self, threshold: usize) -> Vec<String> {
        // Build per-type counts from events
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for ev in &self.events {
            *counts.entry(ev.failure_type.key()).or_insert(0) += 1;
        }
        let mut newly_breached = Vec::new();
        for (key, count) in &counts {
            if *count >= threshold && !self.alerted_types.contains(key) {
                self.alerted_types.insert(key.clone());
                newly_breached.push(key.clone());
            }
        }
        newly_breached.sort(); // deterministic order for tests
        newly_breached
    }

    /// Return the label of the most frequently occurring failure type, or `None`
    /// if the store is empty.
    pub fn most_common_failure_type(&self) -> Option<String> {
        if self.events.is_empty() {
            return None;
        }
        let mut map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for ev in &self.events {
            *map.entry(ev.failure_type.key()).or_insert(0) += 1;
        }
        map.into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(key, _)| key)
    }

    /// Count how many events match the given failure type.
    pub fn count_by_type(&self, ft: &FailureType) -> usize {
        let target = ft.key();
        self.events.iter().filter(|ev| ev.failure_type.key() == target).count()
    }
}
