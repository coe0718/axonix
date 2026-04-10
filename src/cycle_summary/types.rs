//! Data types: `CycleSummaryData` and the `CycleSummary` container struct.

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
}
