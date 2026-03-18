//! Prediction tracking for Axonix (G-021, Issue #24).
//!
//! Logs predictions about outcomes, then compares against what actually happened.
//! Over time builds a calibration corpus: where my model of my own codebase was wrong.
//!
//! # Design
//!
//! - Flat JSON file: `.axonix/predictions.json`
//! - Each prediction has: id, text, created date, optional resolution (outcome + delta)
//! - Predictions are "open" until resolved
//! - `/predict` REPL command to create, resolve, and list predictions
//!
//! # File location
//!
//! Default: `.axonix/predictions.json` in the current working directory.
//! Can be overridden via `AXONIX_PREDICTIONS_PATH` environment variable.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A single prediction entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Human-readable prediction text.
    pub prediction: String,
    /// Date the prediction was made (YYYY-MM-DD).
    pub created: String,
    /// Resolution: what actually happened (None if still open).
    pub outcome: Option<String>,
    /// What was different from the prediction (None if unresolved).
    pub delta: Option<String>,
    /// Date resolved (YYYY-MM-DD), if resolved.
    pub resolved: Option<String>,
}

impl Prediction {
    /// Whether this prediction has been resolved.
    pub fn is_resolved(&self) -> bool {
        self.outcome.is_some()
    }
}

/// Persistent prediction store backed by a JSON file.
pub struct PredictionStore {
    path: PathBuf,
    entries: BTreeMap<u32, Prediction>,
    next_id: u32,
}

impl PredictionStore {
    /// Create a new store at the given path. Loads existing data if the file exists.
    pub fn new(path: PathBuf) -> Self {
        let mut store = Self {
            path,
            entries: BTreeMap::new(),
            next_id: 1,
        };
        store.load_if_exists();
        store
    }

    /// Create a store using the default path (`.axonix/predictions.json`).
    /// Respects `AXONIX_PREDICTIONS_PATH` environment variable.
    pub fn default_path() -> Self {
        let path = std::env::var("AXONIX_PREDICTIONS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".axonix/predictions.json"));
        Self::new(path)
    }

    /// Load entries from disk if the file exists.
    fn load_if_exists(&mut self) {
        if !self.path.exists() {
            return;
        }
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(_) => return,
        };
        let map: BTreeMap<String, Prediction> = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(_) => return,
        };
        for (key_str, pred) in map {
            if let Ok(id) = key_str.parse::<u32>() {
                self.entries.insert(id, pred);
                if id >= self.next_id {
                    self.next_id = id + 1;
                }
            }
        }
    }

    /// Save entries to disk.
    pub fn save(&self) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create directory: {e}"))?;
        }
        let map: BTreeMap<String, &Prediction> = self
            .entries
            .iter()
            .map(|(id, p)| (id.to_string(), p))
            .collect();
        let json = serde_json::to_string_pretty(&map)
            .map_err(|e| format!("serialization error: {e}"))?;
        std::fs::write(&self.path, json).map_err(|e| format!("write error: {e}"))
    }

    /// Add a new prediction. Returns the assigned ID.
    pub fn predict(&mut self, text: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let prediction = Prediction {
            prediction: text.to_string(),
            created: today_str(),
            outcome: None,
            delta: None,
            resolved: None,
        };
        self.entries.insert(id, prediction);
        id
    }

    /// Resolve a prediction with an outcome and optional delta.
    /// Returns Ok with the prediction text if found, Err if not found.
    pub fn resolve(&mut self, id: u32, outcome: &str, delta: Option<&str>) -> Result<String, String> {
        match self.entries.get_mut(&id) {
            Some(pred) => {
                if pred.is_resolved() {
                    return Err(format!("prediction #{id} is already resolved"));
                }
                pred.outcome = Some(outcome.to_string());
                pred.delta = delta.map(|s| s.to_string());
                pred.resolved = Some(today_str());
                Ok(pred.prediction.clone())
            }
            None => Err(format!("prediction #{id} not found")),
        }
    }

    /// Get a prediction by ID.
    pub fn get(&self, id: u32) -> Option<&Prediction> {
        self.entries.get(&id)
    }

    /// List all open (unresolved) predictions.
    pub fn open(&self) -> Vec<(u32, &Prediction)> {
        self.entries
            .iter()
            .filter(|(_, p)| !p.is_resolved())
            .map(|(id, p)| (*id, p))
            .collect()
    }

    /// List all resolved predictions.
    pub fn resolved(&self) -> Vec<(u32, &Prediction)> {
        self.entries
            .iter()
            .filter(|(_, p)| p.is_resolved())
            .map(|(id, p)| (*id, p))
            .collect()
    }

    /// Total number of predictions.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Number of open predictions.
    pub fn open_count(&self) -> usize {
        self.entries.values().filter(|p| !p.is_resolved()).count()
    }

    /// Number of resolved predictions.
    pub fn resolved_count(&self) -> usize {
        self.entries.values().filter(|p| p.is_resolved()).count()
    }

    /// Format open predictions as a block suitable for injection into a system prompt.
    ///
    /// Returns `None` if there are no open predictions.
    /// Format:
    ///   ## Open Predictions
    ///   These are predictions I made but haven't resolved yet.
    ///   #1 [2026-03-17]: I expect the build to succeed on first try
    ///
    /// Used by G-024: inject prediction context at agent startup so I remember
    /// what outcomes I committed to before each session starts.
    pub fn format_for_system_prompt(&self) -> Option<String> {
        let open = self.open();
        if open.is_empty() {
            return None;
        }
        let mut lines = vec!["## Open Predictions".to_string()];
        lines.push("These predictions were made but not yet resolved:".to_string());
        for (id, pred) in &open {
            lines.push(format!("- #{} [{}]: {}", id, pred.created, pred.prediction));
        }
        Some(lines.join("\n"))
    }
}

/// Get today's date as YYYY-MM-DD.
fn today_str() -> String {
    // Use the same approach as memory.rs — compute from system time
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = unix_to_ymd(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Convert Unix timestamp to (year, month, day).
/// Matches the implementation in memory.rs.
fn unix_to_ymd(secs: u64) -> (u32, u32, u32) {
    let days = (secs / 86400) as u32;
    let mut y = 1970u32;
    let mut remaining = days;
    loop {
        let year_days = if is_leap(y) { 366 } else { 365 };
        if remaining < year_days {
            break;
        }
        remaining -= year_days;
        y += 1;
    }
    let month_days: [u32; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1u32;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        m += 1;
    }
    let d = remaining + 1;
    (y, m, d)
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_store() -> (PredictionStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        let store = PredictionStore::new(path);
        (store, dir)
    }

    // ── basic operations ────────────────────────────────────────────────────────

    #[test]
    fn test_predict_returns_incrementing_ids() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("test will pass");
        let id2 = store.predict("build will succeed");
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_predict_stores_text() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("adding 10 tests");
        let pred = store.get(id).unwrap();
        assert_eq!(pred.prediction, "adding 10 tests");
        assert!(!pred.is_resolved());
    }

    #[test]
    fn test_predict_records_date() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("some prediction");
        let pred = store.get(id).unwrap();
        assert_eq!(pred.created.len(), 10, "date should be YYYY-MM-DD");
        assert!(pred.created.starts_with("20"), "date should start with 20xx");
    }

    #[test]
    fn test_resolve_prediction() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("will add 10 tests");
        let result = store.resolve(id, "actually added 12 tests", Some("underestimated by 2"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "will add 10 tests");
        let pred = store.get(id).unwrap();
        assert!(pred.is_resolved());
        assert_eq!(pred.outcome.as_deref(), Some("actually added 12 tests"));
        assert_eq!(pred.delta.as_deref(), Some("underestimated by 2"));
        assert!(pred.resolved.is_some());
    }

    #[test]
    fn test_resolve_nonexistent() {
        let (mut store, _dir) = tmp_store();
        let result = store.resolve(999, "outcome", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_resolve_already_resolved() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("test");
        store.resolve(id, "outcome", None).unwrap();
        let result = store.resolve(id, "second outcome", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already resolved"));
    }

    #[test]
    fn test_resolve_without_delta() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("will succeed");
        store.resolve(id, "succeeded", None).unwrap();
        let pred = store.get(id).unwrap();
        assert!(pred.delta.is_none());
    }

    // ── listing ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_open_returns_unresolved() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("open prediction");
        let id2 = store.predict("resolved prediction");
        store.resolve(id2, "done", None).unwrap();
        let open = store.open();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].0, id1);
    }

    #[test]
    fn test_resolved_returns_resolved() {
        let (mut store, _dir) = tmp_store();
        store.predict("open");
        let id2 = store.predict("resolved");
        store.resolve(id2, "done", None).unwrap();
        let resolved = store.resolved();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, id2);
    }

    #[test]
    fn test_counts() {
        let (mut store, _dir) = tmp_store();
        assert_eq!(store.count(), 0);
        assert_eq!(store.open_count(), 0);
        assert_eq!(store.resolved_count(), 0);
        store.predict("one");
        store.predict("two");
        let id3 = store.predict("three");
        store.resolve(id3, "done", None).unwrap();
        assert_eq!(store.count(), 3);
        assert_eq!(store.open_count(), 2);
        assert_eq!(store.resolved_count(), 1);
    }

    // ── persistence ─────────────────────────────────────────────────────────────

    #[test]
    fn test_save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");

        // Create and save
        let mut store = PredictionStore::new(path.clone());
        let id1 = store.predict("first prediction");
        let id2 = store.predict("second prediction");
        store.resolve(id2, "it happened", Some("exactly as expected")).unwrap();
        store.save().unwrap();

        // Reload
        let store2 = PredictionStore::new(path);
        assert_eq!(store2.count(), 2);
        let p1 = store2.get(id1).unwrap();
        assert_eq!(p1.prediction, "first prediction");
        assert!(!p1.is_resolved());
        let p2 = store2.get(id2).unwrap();
        assert!(p2.is_resolved());
        assert_eq!(p2.outcome.as_deref(), Some("it happened"));
        assert_eq!(p2.delta.as_deref(), Some("exactly as expected"));
    }

    #[test]
    fn test_load_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        let store = PredictionStore::new(path);
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_load_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        std::fs::write(&path, b"{ invalid json }").unwrap();
        let store = PredictionStore::new(path);
        assert_eq!(store.count(), 0, "corrupt file should result in empty store");
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("predictions.json");
        let mut store = PredictionStore::new(path.clone());
        store.predict("test");
        assert!(store.save().is_ok());
        assert!(path.exists());
    }

    #[test]
    fn test_next_id_continues_after_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        let mut store = PredictionStore::new(path.clone());
        store.predict("first");
        store.predict("second");
        store.save().unwrap();

        let mut store2 = PredictionStore::new(path);
        let id3 = store2.predict("third");
        assert_eq!(id3, 3, "next_id should continue from where we left off");
    }

    // ── date ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_unix_to_ymd_epoch() {
        let (y, m, d) = unix_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_unix_to_ymd_known_date() {
        // 2026-03-17T00:00:00Z = 1773705600
        let (y, m, d) = unix_to_ymd(1773705600);
        assert_eq!(y, 2026);
        assert_eq!(m, 3);
        assert_eq!(d, 17);
    }

    #[test]
    fn test_today_str_format() {
        let today = today_str();
        assert_eq!(today.len(), 10);
        assert!(today.contains('-'));
        assert!(today.starts_with("20"));
    }

    // ── get nonexistent ─────────────────────────────────────────────────────────

    #[test]
    fn test_get_nonexistent() {
        let (store, _dir) = tmp_store();
        assert!(store.get(999).is_none());
    }

    // ── empty store operations ──────────────────────────────────────────────────

    #[test]
    fn test_open_empty() {
        let (store, _dir) = tmp_store();
        assert!(store.open().is_empty());
    }

    #[test]
    fn test_resolved_empty() {
        let (store, _dir) = tmp_store();
        assert!(store.resolved().is_empty());
    }
}
