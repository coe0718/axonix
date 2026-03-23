//! Failure pattern tracking for Axonix self-monitoring (G-066).
//!
//! Persists a log of known failure types to `.axonix/failure_patterns.json`
//! so that patterns can be reviewed across sessions and surfaced in the
//! morning brief.
//!
//! # Design
//!
//! - Flat JSON array: transparent, human-readable, diffable in git
//! - Each event records type, description, session label, and date
//! - Load-on-read, save-on-write: minimal complexity, no background threads
//! - `most_common_failure_type()` drives the morning brief summary
//!
//! # Example
//!
//! ```
//! use axonix::failure_patterns::{FailurePatternStore, FailureType};
//!
//! let mut store = FailurePatternStore::new("/tmp/test_fp_doctest.json");
//! store.log_failure(
//!     FailureType::FalseCompletion,
//!     "marked goal done without verifying in code",
//!     "Day 10, Session 3",
//!     "2026-03-23",
//! );
//! assert_eq!(store.total_count(), 1);
//! ```

use std::path::{Path, PathBuf};

/// Known categories of agent failure for self-monitoring.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum FailureType {
    /// Marked a goal done without verifying in code.
    FalseCompletion,
    /// Claimed an infra blocker that didn't exist.
    InfraBlindness,
    /// Claimed something was implemented that wasn't.
    FalseClaim,
    /// Committed code that had to be reverted.
    RevertRequired,
    /// Marked a goal complete then had to reopen it.
    GoalReopened,
    /// Session ended without updating GOALS.md/METRICS.md.
    MissedWrapUp,
    /// Any other failure type with a free-form description.
    Other(String),
}

impl FailureType {
    /// Return a human-readable label for this failure type.
    pub fn label(&self) -> String {
        match self {
            FailureType::FalseCompletion => "FalseCompletion".to_string(),
            FailureType::InfraBlindness  => "InfraBlindness".to_string(),
            FailureType::FalseClaim      => "FalseClaim".to_string(),
            FailureType::RevertRequired  => "RevertRequired".to_string(),
            FailureType::GoalReopened    => "GoalReopened".to_string(),
            FailureType::MissedWrapUp    => "MissedWrapUp".to_string(),
            FailureType::Other(s)        => format!("Other({})", s),
        }
    }

    /// Return the canonical string key used for counting / bucketing.
    fn key(&self) -> String {
        match self {
            FailureType::Other(s) => format!("Other:{}", s),
            _ => self.label(),
        }
    }
}

/// A single logged failure event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FailureEvent {
    /// The category of failure.
    pub failure_type: FailureType,
    /// Human-readable description of what went wrong.
    pub description: String,
    /// Session label, e.g. "Day 10, Session 5".
    pub session: String,
    /// ISO date string, e.g. "2026-03-23".
    pub date: String,
}

/// Persistent store for failure pattern events.
///
/// Backed by a flat JSON array at the configured path.
pub struct FailurePatternStore {
    /// Path to the backing JSON file.
    pub path: PathBuf,
    /// All events, in insertion order (oldest first).
    pub events: Vec<FailureEvent>,
}

impl FailurePatternStore {
    /// Create a new empty store at the given path (does NOT read from disk).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            events: Vec::new(),
        }
    }

    /// Load from the given path. Returns an empty store if the file doesn't
    /// exist or can't be parsed.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut store = Self::new(path.clone());
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<Vec<FailureEvent>>(&content) {
                        Ok(events) => store.events = events,
                        Err(e) => eprintln!("  ⚠ failure_patterns: failed to parse {:?}: {e}", path),
                    }
                }
                Err(e) => eprintln!("  ⚠ failure_patterns: failed to read {:?}: {e}", path),
            }
        }
        store
    }

    /// Load from the default path (`.axonix/failure_patterns.json`).
    pub fn default_path() -> Self {
        Self::load(default_failure_patterns_path())
    }

    /// Log a new failure event.
    pub fn log_failure(
        &mut self,
        failure_type: FailureType,
        description: &str,
        session: &str,
        date: &str,
    ) {
        self.events.push(FailureEvent {
            failure_type,
            description: description.to_string(),
            session: session.to_string(),
            date: date.to_string(),
        });
    }

    /// Save the store to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.events)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }

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
        let mut counts: Vec<(String, usize)> = {
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

    /// Total number of logged failure events.
    pub fn total_count(&self) -> usize {
        self.events.len()
    }
}

/// Return the default path for the failure patterns store.
///
/// Uses `AXONIX_FAILURE_PATTERNS_PATH` env var if set, otherwise
/// `.axonix/failure_patterns.json` in the current working directory.
pub fn default_failure_patterns_path() -> PathBuf {
    if let Ok(path) = std::env::var("AXONIX_FAILURE_PATTERNS_PATH") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    PathBuf::from(".axonix/failure_patterns.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 1. test_new_store_is_empty ──────────────────────────────────────────────

    #[test]
    fn test_new_store_is_empty() {
        let store = FailurePatternStore::new("/tmp/fp_test_new.json");
        assert_eq!(store.total_count(), 0);
        assert!(store.events.is_empty());
    }

    // ── 2. test_log_failure_adds_event ──────────────────────────────────────────

    #[test]
    fn test_log_failure_adds_event() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_log1.json");
        store.log_failure(
            FailureType::FalseCompletion,
            "marked done without verifying",
            "Day 10, Session 3",
            "2026-03-23",
        );
        assert_eq!(store.total_count(), 1);
        assert_eq!(store.events[0].failure_type, FailureType::FalseCompletion);
        assert_eq!(store.events[0].session, "Day 10, Session 3");
    }

    // ── 3. test_log_multiple_failures ──────────────────────────────────────────

    #[test]
    fn test_log_multiple_failures() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_multi.json");
        store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
        store.log_failure(FailureType::InfraBlindness, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseClaim, "c", "S3", "2026-01-03");
        assert_eq!(store.total_count(), 3);
    }

    // ── 4. test_get_recent_failures_empty ──────────────────────────────────────

    #[test]
    fn test_get_recent_failures_empty() {
        let store = FailurePatternStore::new("/tmp/fp_test_empty_recent.json");
        let recent = store.get_recent_failures(5);
        assert!(recent.is_empty());
    }

    // ── 5. test_get_recent_failures_limit ──────────────────────────────────────

    #[test]
    fn test_get_recent_failures_limit() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_limit.json");
        for i in 0..5 {
            store.log_failure(
                FailureType::FalseCompletion,
                &format!("event {i}"),
                "S1",
                "2026-01-01",
            );
        }
        let recent = store.get_recent_failures(3);
        assert_eq!(recent.len(), 3);
    }

    // ── 6. test_get_recent_failures_order ──────────────────────────────────────

    #[test]
    fn test_get_recent_failures_order() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_order.json");
        store.log_failure(FailureType::FalseCompletion, "first", "S1", "2026-01-01");
        store.log_failure(FailureType::FalseCompletion, "second", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseCompletion, "third", "S3", "2026-01-03");
        let recent = store.get_recent_failures(3);
        // Most recent first
        assert_eq!(recent[0].description, "third");
        assert_eq!(recent[1].description, "second");
        assert_eq!(recent[2].description, "first");
    }

    // ── 7. test_failure_summary_empty ──────────────────────────────────────────

    #[test]
    fn test_failure_summary_empty() {
        let store = FailurePatternStore::new("/tmp/fp_test_summary_empty.json");
        let summary = store.failure_summary();
        assert!(
            summary.contains("no failures"),
            "empty store should say no failures: {summary}"
        );
    }

    // ── 8. test_failure_summary_nonempty ───────────────────────────────────────

    #[test]
    fn test_failure_summary_nonempty() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_summary_nonempty.json");
        store.log_failure(
            FailureType::RevertRequired,
            "had to revert a bad commit",
            "Day 9, Session 2",
            "2026-03-20",
        );
        let summary = store.failure_summary();
        assert!(
            summary.contains("RevertRequired"),
            "summary should mention failure type: {summary}"
        );
        assert!(
            summary.contains("1"),
            "summary should show count: {summary}"
        );
    }

    // ── 9. test_most_common_failure_type_empty ─────────────────────────────────

    #[test]
    fn test_most_common_failure_type_empty() {
        let store = FailurePatternStore::new("/tmp/fp_test_mcft_empty.json");
        assert!(store.most_common_failure_type().is_none());
    }

    // ── 10. test_most_common_failure_type_single ───────────────────────────────

    #[test]
    fn test_most_common_failure_type_single() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_mcft_single.json");
        store.log_failure(FailureType::MissedWrapUp, "forgot to update GOALS", "S1", "2026-01-01");
        let result = store.most_common_failure_type();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "MissedWrapUp");
    }

    // ── 11. test_most_common_failure_type_multiple ─────────────────────────────

    #[test]
    fn test_most_common_failure_type_multiple() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_mcft_multi.json");
        // FalseCompletion appears 3×, InfraBlindness 1×
        store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
        store.log_failure(FailureType::InfraBlindness, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseCompletion, "c", "S3", "2026-01-03");
        store.log_failure(FailureType::FalseCompletion, "d", "S4", "2026-01-04");
        let result = store.most_common_failure_type();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "FalseCompletion");
    }

    // ── 12. test_count_by_type ─────────────────────────────────────────────────

    #[test]
    fn test_count_by_type() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_cbt.json");
        store.log_failure(FailureType::GoalReopened, "a", "S1", "2026-01-01");
        store.log_failure(FailureType::GoalReopened, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseClaim, "c", "S3", "2026-01-03");
        assert_eq!(store.count_by_type(&FailureType::GoalReopened), 2);
        assert_eq!(store.count_by_type(&FailureType::FalseClaim), 1);
        assert_eq!(store.count_by_type(&FailureType::RevertRequired), 0);
    }

    // ── 13. test_total_count ───────────────────────────────────────────────────

    #[test]
    fn test_total_count() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_tc.json");
        assert_eq!(store.total_count(), 0);
        store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
        assert_eq!(store.total_count(), 1);
        store.log_failure(FailureType::MissedWrapUp, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::InfraBlindness, "c", "S3", "2026-01-03");
        assert_eq!(store.total_count(), 3);
    }

    // ── 14. test_failure_type_display ─────────────────────────────────────────

    #[test]
    fn test_failure_type_display() {
        assert_eq!(FailureType::FalseCompletion.label(), "FalseCompletion");
        assert_eq!(FailureType::InfraBlindness.label(), "InfraBlindness");
        assert_eq!(FailureType::FalseClaim.label(), "FalseClaim");
        assert_eq!(FailureType::RevertRequired.label(), "RevertRequired");
        assert_eq!(FailureType::GoalReopened.label(), "GoalReopened");
        assert_eq!(FailureType::MissedWrapUp.label(), "MissedWrapUp");
        assert_eq!(FailureType::Other("custom".to_string()).label(), "Other(custom)");
    }

    // ── 15. test_save_and_load_roundtrip ──────────────────────────────────────

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("failure_patterns.json");

        let mut store = FailurePatternStore::new(&path);
        store.log_failure(
            FailureType::FalseCompletion,
            "marked done without verifying",
            "Day 10, Session 3",
            "2026-03-23",
        );
        store.log_failure(
            FailureType::Other("custom type".to_string()),
            "something unusual happened",
            "Day 10, Session 4",
            "2026-03-24",
        );
        store.save().expect("save should succeed");

        let loaded = FailurePatternStore::load(&path);
        assert_eq!(loaded.total_count(), 2);
        assert_eq!(loaded.events[0].failure_type, FailureType::FalseCompletion);
        assert_eq!(loaded.events[0].description, "marked done without verifying");
        assert_eq!(loaded.events[0].session, "Day 10, Session 3");
        assert_eq!(loaded.events[0].date, "2026-03-23");
        assert_eq!(
            loaded.events[1].failure_type,
            FailureType::Other("custom type".to_string())
        );
    }

    // ── 16. test_load_nonexistent_path ────────────────────────────────────────

    #[test]
    fn test_load_nonexistent_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        let store = FailurePatternStore::load(&path);
        assert_eq!(store.total_count(), 0);
        assert!(store.events.is_empty());
    }

    // ── 17. test_default_path_string ──────────────────────────────────────────

    #[test]
    fn test_default_path_string() {
        // When the env var is not set, default path ends with failure_patterns.json
        // (We unset the env var to get deterministic results)
        std::env::remove_var("AXONIX_FAILURE_PATTERNS_PATH");
        let path = default_failure_patterns_path();
        assert!(
            path.to_string_lossy().ends_with("failure_patterns.json"),
            "default path should end with failure_patterns.json: {path:?}"
        );
    }

    // ── Bonus: test_other_variant_serde_roundtrip ──────────────────────────────

    #[test]
    fn test_other_variant_serde_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fp_other.json");
        let mut store = FailurePatternStore::new(&path);
        store.log_failure(
            FailureType::Other("unusual blocker".to_string()),
            "misc failure",
            "Day 5, Session 1",
            "2026-02-01",
        );
        store.save().unwrap();

        let loaded = FailurePatternStore::load(&path);
        assert_eq!(loaded.total_count(), 1);
        match &loaded.events[0].failure_type {
            FailureType::Other(s) => assert_eq!(s, "unusual blocker"),
            other => panic!("expected Other variant, got {other:?}"),
        }
    }

    // ── Bonus: test_get_recent_fewer_than_n ───────────────────────────────────

    #[test]
    fn test_get_recent_fewer_than_n() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_fewer.json");
        store.log_failure(FailureType::FalseCompletion, "only one", "S1", "2026-01-01");
        // Ask for 10 but only 1 exists
        let recent = store.get_recent_failures(10);
        assert_eq!(recent.len(), 1);
    }

    // ── Bonus: test_count_by_type_other ───────────────────────────────────────

    #[test]
    fn test_count_by_type_other() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_cbt_other.json");
        store.log_failure(FailureType::Other("network".to_string()), "a", "S1", "2026-01-01");
        store.log_failure(FailureType::Other("network".to_string()), "b", "S2", "2026-01-02");
        store.log_failure(FailureType::Other("disk".to_string()), "c", "S3", "2026-01-03");
        // "network" and "disk" are distinct Other values
        assert_eq!(store.count_by_type(&FailureType::Other("network".to_string())), 2);
        assert_eq!(store.count_by_type(&FailureType::Other("disk".to_string())), 1);
    }
}
