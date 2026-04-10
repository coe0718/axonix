//! Formatting and display: failure_summary, get_recent_failures.

use super::types::FailureEvent;
use super::store::FailurePatternStore;

impl FailurePatternStore {
    /// Return the most recent `n` events, most-recent first.
    pub fn get_recent_failures(&self, n: usize) -> Vec<&FailureEvent> {
        let total = self.events.len();
        let start = total.saturating_sub(n);
        self.events[start..].iter().rev().collect()
    }

    /// Human-readable summary for the REPL `/failures` command.
    pub fn failure_summary(&self) -> String {
        if self.events.is_empty() {
            return "(no failures logged yet)".to_string();
        }
        let mut lines = vec![format!(
            "Failure patterns ({} total):",
            self.events.len()
        )];
        // Count by type
        let counts: Vec<(String, usize)> = {
            let mut map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for ev in &self.events {
                *map.entry(ev.failure_type.key()).or_insert(0) += 1;
            }
            let mut v: Vec<(String, usize)> = map.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            v
        };
        for (key, count) in &counts {
            lines.push(format!("  {key}: {count}"));
        }
        // Show the 5 most recent events
        lines.push(String::new());
        lines.push("Recent:".to_string());
        for ev in self.get_recent_failures(5) {
            lines.push(format!(
                "  [{} {}] {}: {}",
                ev.date,
                ev.session,
                ev.failure_type.label(),
                ev.description,
            ));
        }
        let _ = counts; // suppress unused warning
        lines.join("\n")
    }
}
