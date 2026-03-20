//! Cycle summary: compact session state persisted across agent restarts.
//!
//! Addresses context window exhaustion (Issue #38): each session writes a
//! compact summary of what was done, what changed, and what's pending.
//! The next session loads this summary and injects it into the system prompt,
//! giving the agent enough context to continue intelligently without replaying
//! the full message history.
//!
//! # Design
//!
//! - Flat JSON file at `.axonix/cycle_summary.json`
//! - Written at session end by the evolve.sh orchestrator (or via `/summary` REPL command)
//! - Loaded at startup alongside memory and predictions
//! - Injected into system prompt as a compact "## Last Session" context block
//! - Bounded size: never grows beyond a fixed number of entries
//!
//! # Format
//!
//! ```json
//! {
//!   "session": "Day 7, Session 5",
//!   "date": "2026-03-20",
//!   "completed": ["Implemented cycle_summary module", "Fixed Issue #38"],
//!   "changed_files": ["src/cycle_summary.rs", "src/lib.rs", "src/main.rs"],
//!   "pending": ["G-031: morning brief on schedule", "G-033: context window fix"],
//!   "learnings": ["cycle_summary.json keeps context bounded across sessions"]
//! }
//! ```
//!
//! # Example
//!
//! ```
//! use axonix::cycle_summary::CycleSummary;
//!
//! let mut summary = CycleSummary::new("/tmp/test-cycle-summary.json");
//! summary.set_session("Day 7, Session 5", "2026-03-20");
//! summary.add_completed("Implemented cycle_summary module");
//! summary.add_changed_file("src/cycle_summary.rs");
//! summary.add_pending("G-031: morning brief on schedule");
//! summary.add_learning("compact summary keeps context bounded");
//! let _ = summary.save();
//!
//! let loaded = CycleSummary::load("/tmp/test-cycle-summary.json");
//! assert!(loaded.format_for_system_prompt().is_some());
//! ```

use std::path::{Path, PathBuf};

/// A compact summary of one session's work.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CycleSummaryData {
    /// Session identifier e.g. "Day 7, Session 5"
    pub session: String,
    /// ISO date e.g. "2026-03-20"
    pub date: String,
    /// What was accomplished this session (bullet points, max ~10 items)
    #[serde(default)]
    pub completed: Vec<String>,
    /// Files modified this session (for quick context)
    #[serde(default)]
    pub changed_files: Vec<String>,
    /// Active goals / pending work for next session
    #[serde(default)]
    pub pending: Vec<String>,
    /// Key facts or patterns learned this session
    #[serde(default)]
    pub learnings: Vec<String>,
    /// Test count at end of session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_count: Option<u32>,
}

impl CycleSummaryData {
    /// Create a minimal summary for a session.
    pub fn new(session: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            session: session.into(),
            date: date.into(),
            completed: Vec::new(),
            changed_files: Vec::new(),
            pending: Vec::new(),
            learnings: Vec::new(),
            test_count: None,
        }
    }

    /// Format this summary as a system prompt injection block.
    ///
    /// Returns None if the summary is completely empty (no meaningful content).
    pub fn format_for_prompt(&self) -> Option<String> {
        if self.completed.is_empty()
            && self.pending.is_empty()
            && self.changed_files.is_empty()
            && self.learnings.is_empty()
        {
            return None;
        }

        let mut out = String::new();
        out.push_str("## Last Session\n");
        out.push_str(&format!("**{}** ({})\n", self.session, self.date));
        if let Some(count) = self.test_count {
            out.push_str(&format!("Tests passing: {count}\n"));
        }
        if !self.completed.is_empty() {
            out.push_str("\nCompleted:\n");
            for item in self.completed.iter().take(10) {
                out.push_str(&format!("- {item}\n"));
            }
        }
        if !self.changed_files.is_empty() {
            out.push_str("\nChanged files:\n");
            for f in self.changed_files.iter().take(15) {
                out.push_str(&format!("- {f}\n"));
            }
        }
        if !self.pending.is_empty() {
            out.push_str("\nPending / next session:\n");
            for item in self.pending.iter().take(10) {
                out.push_str(&format!("- {item}\n"));
            }
        }
        if !self.learnings.is_empty() {
            out.push_str("\nKey learnings:\n");
            for item in self.learnings.iter().take(5) {
                out.push_str(&format!("- {item}\n"));
            }
        }
        Some(out)
    }
}

/// Manages the cycle summary file.
pub struct CycleSummary {
    /// Path to the JSON file.
    pub path: PathBuf,
    /// The in-memory data.
    pub data: Option<CycleSummaryData>,
}

impl CycleSummary {
    /// Open (or create) a cycle summary at the given path.
    ///
    /// Does not read from disk yet — call `load()` to populate `data`.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            data: None,
        }
    }

    /// Default path: `.axonix/cycle_summary.json`
    pub fn default_path() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/workspace".to_string());
        let path = PathBuf::from(&home).join(".axonix").join("cycle_summary.json");
        Self::load(path)
    }

    /// Load from the given path. Returns an instance with data=None if file doesn't exist.
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<CycleSummaryData>(&s).ok());
        Self { path, data }
    }

    /// Set or replace the summary data for this session.
    pub fn set(&mut self, data: CycleSummaryData) {
        self.data = Some(data);
    }

    /// Set session metadata and initialize empty data if not already set.
    pub fn set_session(&mut self, session: impl Into<String>, date: impl Into<String>) {
        if self.data.is_none() {
            self.data = Some(CycleSummaryData::new(session, date));
        } else if let Some(ref mut d) = self.data {
            d.session = session.into();
            d.date = date.into();
        }
    }

    /// Add a completed item.
    pub fn add_completed(&mut self, item: impl Into<String>) {
        if let Some(ref mut d) = self.data {
            d.completed.push(item.into());
        }
    }

    /// Add a changed file.
    pub fn add_changed_file(&mut self, file: impl Into<String>) {
        if let Some(ref mut d) = self.data {
            d.changed_files.push(file.into());
        }
    }

    /// Add a pending item.
    pub fn add_pending(&mut self, item: impl Into<String>) {
        if let Some(ref mut d) = self.data {
            d.pending.push(item.into());
        }
    }

    /// Add a learning.
    pub fn add_learning(&mut self, item: impl Into<String>) {
        if let Some(ref mut d) = self.data {
            d.learnings.push(item.into());
        }
    }

    /// Set the test count.
    pub fn set_test_count(&mut self, count: u32) {
        if let Some(ref mut d) = self.data {
            d.test_count = Some(count);
        }
    }

    /// Save to disk. Creates the parent directory if needed.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(
            self.data
                .as_ref()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no data to save"))?,
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.path, json)
    }

    /// Format for injection into the system prompt.
    pub fn format_for_system_prompt(&self) -> Option<String> {
        self.data.as_ref()?.format_for_prompt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("axonix_cycle_test_{name}.json"))
    }

    fn cleanup(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }

    // ── CycleSummaryData ─────────────────────────────────────────────────────

    #[test]
    fn test_new_summary_data_is_empty() {
        let data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        assert_eq!(data.session, "Day 7, Session 5");
        assert_eq!(data.date, "2026-03-20");
        assert!(data.completed.is_empty());
        assert!(data.changed_files.is_empty());
        assert!(data.pending.is_empty());
        assert!(data.learnings.is_empty());
        assert!(data.test_count.is_none());
    }

    #[test]
    fn test_format_for_prompt_returns_none_when_empty() {
        let data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        assert!(data.format_for_prompt().is_none(),
            "Empty summary should return None");
    }

    #[test]
    fn test_format_for_prompt_contains_session_and_date() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("Fixed Issue #38".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("Day 7, Session 5"), "Should contain session name");
        assert!(formatted.contains("2026-03-20"), "Should contain date");
    }

    #[test]
    fn test_format_for_prompt_contains_completed_items() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("Implemented cycle_summary module".to_string());
        data.completed.push("Fixed Issue #38".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("Implemented cycle_summary module"));
        assert!(formatted.contains("Fixed Issue #38"));
    }

    #[test]
    fn test_format_for_prompt_contains_pending_items() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.pending.push("G-031: morning brief".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("G-031: morning brief"));
        assert!(formatted.contains("Pending"));
    }

    #[test]
    fn test_format_for_prompt_contains_changed_files() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.changed_files.push("src/cycle_summary.rs".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("src/cycle_summary.rs"));
    }

    #[test]
    fn test_format_for_prompt_contains_learnings() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.learnings.push("compact summary keeps context bounded".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("compact summary keeps context bounded"));
    }

    #[test]
    fn test_format_for_prompt_shows_test_count() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("wrote tests".to_string());
        data.test_count = Some(506);
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("506"), "Should show test count");
    }

    #[test]
    fn test_format_for_prompt_caps_completed_at_10() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        for i in 0..15 {
            data.completed.push(format!("item {i}"));
        }
        let formatted = data.format_for_prompt().unwrap();
        // Only first 10 should appear (items 0-9)
        assert!(formatted.contains("item 9"), "Item 9 should appear");
        assert!(!formatted.contains("item 10"), "Item 10 should not appear (capped at 10)");
    }

    #[test]
    fn test_format_for_prompt_caps_changed_files_at_15() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        for i in 0..20 {
            data.changed_files.push(format!("src/file_{i}.rs"));
        }
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("src/file_14.rs"), "File 14 should appear");
        assert!(!formatted.contains("src/file_15.rs"), "File 15 should not appear (capped at 15)");
    }

    #[test]
    fn test_format_for_prompt_caps_learnings_at_5() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        for i in 0..10 {
            data.learnings.push(format!("learning {i}"));
        }
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("learning 4"), "Learning 4 should appear");
        assert!(!formatted.contains("learning 5"), "Learning 5 should not appear (capped at 5)");
    }

    #[test]
    fn test_cycle_summary_data_serializes_to_json() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("thing done".to_string());
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("Day 7, Session 5"));
        assert!(json.contains("thing done"));
    }

    #[test]
    fn test_cycle_summary_data_deserializes_from_json() {
        let json = r#"{
            "session": "Day 7, Session 5",
            "date": "2026-03-20",
            "completed": ["Fixed Issue #38"],
            "changed_files": ["src/main.rs"],
            "pending": ["G-031"],
            "learnings": ["keep it compact"]
        }"#;
        let data: CycleSummaryData = serde_json::from_str(json).unwrap();
        assert_eq!(data.session, "Day 7, Session 5");
        assert_eq!(data.completed, vec!["Fixed Issue #38"]);
        assert_eq!(data.changed_files, vec!["src/main.rs"]);
        assert_eq!(data.pending, vec!["G-031"]);
        assert_eq!(data.learnings, vec!["keep it compact"]);
    }

    #[test]
    fn test_cycle_summary_data_deserializes_with_defaults() {
        // Minimal JSON — missing optional fields should default to empty vecs
        let json = r#"{"session": "Day 7", "date": "2026-03-20"}"#;
        let data: CycleSummaryData = serde_json::from_str(json).unwrap();
        assert!(data.completed.is_empty());
        assert!(data.pending.is_empty());
        assert!(data.test_count.is_none());
    }

    // ── CycleSummary (file manager) ──────────────────────────────────────────

    #[test]
    fn test_cycle_summary_new_has_no_data() {
        let path = tmp_path("new_no_data");
        cleanup(&path);
        let cs = CycleSummary::new(&path);
        assert!(cs.data.is_none());
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_load_missing_file_returns_none_data() {
        let path = tmp_path("load_missing");
        cleanup(&path); // ensure it doesn't exist
        let cs = CycleSummary::load(&path);
        assert!(cs.data.is_none(), "Missing file should produce None data");
    }

    #[test]
    fn test_cycle_summary_load_invalid_json_returns_none_data() {
        let path = tmp_path("load_invalid");
        fs::write(&path, "not valid json {{{{").unwrap();
        let cs = CycleSummary::load(&path);
        assert!(cs.data.is_none(), "Invalid JSON should produce None data");
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_save_and_load_roundtrip() {
        let path = tmp_path("roundtrip");
        cleanup(&path);

        let mut cs = CycleSummary::new(&path);
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("roundtrip test".to_string());
        data.test_count = Some(506);
        cs.set(data);
        cs.save().unwrap();

        let loaded = CycleSummary::load(&path);
        let d = loaded.data.unwrap();
        assert_eq!(d.session, "Day 7, Session 5");
        assert_eq!(d.completed, vec!["roundtrip test"]);
        assert_eq!(d.test_count, Some(506));
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_save_creates_parent_dir() {
        let path = tmp_path("parent_dir_test");
        let subpath = path.join("subdir").join("cycle_summary.json");
        let _ = fs::remove_dir_all(&path);

        let mut cs = CycleSummary::new(&subpath);
        let data = CycleSummaryData::new("Day 7", "2026-03-20");
        cs.set(data);
        // add something so format_for_prompt would return Some (not strictly needed here)
        cs.add_completed("test");
        cs.save().unwrap();
        assert!(subpath.exists(), "File should be created with parent dirs");
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_cycle_summary_save_without_data_returns_error() {
        let path = tmp_path("save_no_data");
        cleanup(&path);
        let cs = CycleSummary::new(&path);
        let result = cs.save();
        assert!(result.is_err(), "Save without data should return error");
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_set_session_initializes_data() {
        let path = tmp_path("set_session");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        assert!(cs.data.is_some());
        assert_eq!(cs.data.as_ref().unwrap().session, "Day 7, Session 5");
    }

    #[test]
    fn test_cycle_summary_set_session_updates_existing_data() {
        let path = tmp_path("update_session");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 4", "2026-03-20");
        cs.set_session("Day 7, Session 5", "2026-03-20"); // update
        assert_eq!(cs.data.as_ref().unwrap().session, "Day 7, Session 5");
    }

    #[test]
    fn test_cycle_summary_add_methods_require_data() {
        let path = tmp_path("add_methods");
        let mut cs = CycleSummary::new(&path);
        // These are no-ops when data is None — should not panic
        cs.add_completed("test");
        cs.add_changed_file("src/main.rs");
        cs.add_pending("G-031");
        cs.add_learning("learned something");
        cs.set_test_count(506);
        assert!(cs.data.is_none(), "Data should still be None");
    }

    #[test]
    fn test_cycle_summary_add_completed() {
        let path = tmp_path("add_completed");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_completed("Fixed Issue #38");
        assert_eq!(cs.data.as_ref().unwrap().completed, vec!["Fixed Issue #38"]);
    }

    #[test]
    fn test_cycle_summary_add_changed_file() {
        let path = tmp_path("add_file");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_changed_file("src/cycle_summary.rs");
        assert_eq!(cs.data.as_ref().unwrap().changed_files, vec!["src/cycle_summary.rs"]);
    }

    #[test]
    fn test_cycle_summary_add_pending() {
        let path = tmp_path("add_pending");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_pending("G-031: morning brief on schedule");
        assert_eq!(cs.data.as_ref().unwrap().pending, vec!["G-031: morning brief on schedule"]);
    }

    #[test]
    fn test_cycle_summary_add_learning() {
        let path = tmp_path("add_learning");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_learning("context bounded by summary");
        assert_eq!(cs.data.as_ref().unwrap().learnings, vec!["context bounded by summary"]);
    }

    #[test]
    fn test_cycle_summary_set_test_count() {
        let path = tmp_path("set_count");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.set_test_count(506);
        assert_eq!(cs.data.as_ref().unwrap().test_count, Some(506));
    }

    #[test]
    fn test_format_for_system_prompt_none_when_no_data() {
        let path = tmp_path("fmt_no_data");
        let cs = CycleSummary::new(&path);
        assert!(cs.format_for_system_prompt().is_none());
    }

    #[test]
    fn test_format_for_system_prompt_none_when_empty_data() {
        let path = tmp_path("fmt_empty");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7", "2026-03-20");
        // No content added — should return None
        assert!(cs.format_for_system_prompt().is_none());
    }

    #[test]
    fn test_format_for_system_prompt_some_when_has_content() {
        let path = tmp_path("fmt_with_content");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_completed("Fixed Issue #38");
        let result = cs.format_for_system_prompt();
        assert!(result.is_some());
        assert!(result.unwrap().contains("Last Session"));
    }

    #[test]
    fn test_cycle_summary_load_valid_json_parses() {
        let path = tmp_path("load_valid");
        let json = r#"{
            "session": "Day 7, Session 4",
            "date": "2026-03-20",
            "completed": ["Hit 500 tests"],
            "pending": ["G-031"]
        }"#;
        fs::write(&path, json).unwrap();
        let cs = CycleSummary::load(&path);
        let data = cs.data.unwrap();
        assert_eq!(data.session, "Day 7, Session 4");
        assert_eq!(data.completed, vec!["Hit 500 tests"]);
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_overwrites_on_save() {
        let path = tmp_path("overwrite");
        cleanup(&path);

        // First save
        let mut cs1 = CycleSummary::new(&path);
        let mut d1 = CycleSummaryData::new("Day 7, Session 4", "2026-03-20");
        d1.completed.push("first save".to_string());
        cs1.set(d1);
        cs1.save().unwrap();

        // Second save (overwrites)
        let mut cs2 = CycleSummary::new(&path);
        let mut d2 = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        d2.completed.push("second save".to_string());
        cs2.set(d2);
        cs2.save().unwrap();

        let loaded = CycleSummary::load(&path);
        let data = loaded.data.unwrap();
        assert_eq!(data.session, "Day 7, Session 5", "Should have second session");
        assert_eq!(data.completed, vec!["second save"]);
        cleanup(&path);
    }

    #[test]
    fn test_default_path_uses_axonix_dir() {
        let cs = CycleSummary::default_path();
        let path_str = cs.path.to_string_lossy();
        assert!(path_str.contains(".axonix"), "Should be in .axonix directory");
        assert!(path_str.contains("cycle_summary.json"), "Should be cycle_summary.json");
    }
}
