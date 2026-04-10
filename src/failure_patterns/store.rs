//! `FailurePatternStore`: the main struct with new/load/save/log methods.

use std::path::PathBuf;
use super::types::{FailureEvent, FailureType};

/// Persistent store for failure pattern events.
///
/// Backed by a flat JSON array at the configured path.
pub struct FailurePatternStore {
    /// Path to the backing JSON file.
    pub path: PathBuf,
    /// All events, in insertion order (oldest first).
    pub events: Vec<FailureEvent>,
    /// In-memory set of failure type labels for which a threshold alert has
    /// already been issued this session. Resets on restart (acceptable trade-off:
    /// avoids cross-session re-alerting without adding persistence complexity).
    pub alerted_types: std::collections::HashSet<String>,
}

impl FailurePatternStore {
    /// Create a new empty store at the given path (does NOT read from disk).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            events: Vec::new(),
            alerted_types: std::collections::HashSet::new(),
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
        Self::load(super::default_failure_patterns_path())
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

    /// Total number of logged failure events.
    pub fn total_count(&self) -> usize {
        self.events.len()
    }
}
