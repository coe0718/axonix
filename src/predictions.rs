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

/// Calibration score across all resolved predictions.
#[derive(Debug, Clone)]
pub struct CalibrationScore {
    /// Total number of resolved predictions.
    pub total_resolved: usize,
    /// Number of predictions whose outcome contains "TRUE".
    pub correct: usize,
    /// correct / total_resolved (0.0 if no resolved predictions).
    pub hit_rate: f64,
    /// Average days early (positive = early, negative = late). 0.0 if no delta data.
    pub avg_days_early: f64,
    /// "optimistic", "pessimistic", or "calibrated".
    pub direction_bias: String,
}

impl CalibrationScore {
    /// Format for injection into a system prompt.
    ///
    /// Returns `None` if `total_resolved == 0`.
    pub fn format_for_system_prompt(&self) -> Option<String> {
        if self.total_resolved == 0 {
            return None;
        }
        let hit_pct = self.hit_rate * 100.0;
        let timing_line = if self.avg_days_early > 0.0 {
            format!("Average resolution: {:.1} days early", self.avg_days_early)
        } else if self.avg_days_early < 0.0 {
            format!("Average resolution: {:.1} days late", self.avg_days_early.abs())
        } else {
            "Average resolution: on time".to_string()
        };
        let bias_advice = match self.direction_bias.as_str() {
            "optimistic" => "make bold, specific predictions",
            "pessimistic" => "be more confident — you may be underestimating",
            _ => "keep making precise, verifiable predictions",
        };
        Some(format!(
            "## Prediction Calibration\n\
             {} resolved predictions: {}/{} correct ({:.1}% hit rate)\n\
             {}\n\
             Bias: {} — {}",
            self.total_resolved,
            self.correct,
            self.total_resolved,
            hit_pct,
            timing_line,
            self.direction_bias,
            bias_advice,
        ))
    }
}

/// Parse days from a delta string.
///
/// Looks for patterns like:
/// - "1 day(s) early" → +1.0
/// - "3 days early"   → +3.0
/// - "2 day(s) late"  → -2.0
/// - "1 days late"    → -1.0
///
/// Returns `None` if no parseable pattern found.
fn parse_days_from_delta(delta: &str) -> Option<f64> {
    let lower = delta.to_lowercase();
    // Find a number followed by "day"
    let day_pos = lower.find("day")?;
    // Work backwards from "day" to find the number
    let before_day = lower[..day_pos].trim_end();
    let num_str: String = before_day.chars().rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars().rev().collect();
    let n: f64 = num_str.parse().ok()?;
    // Now determine direction: look after "day..." for "early" or "late"
    let after_day = &lower[day_pos..];
    if after_day.contains("early") {
        Some(n)
    } else if after_day.contains("late") {
        Some(-n)
    } else {
        None
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

    /// Compute a calibration score across all resolved predictions.
    ///
    /// Returns a `CalibrationScore` with hit rate, avg_days_early, and direction_bias.
    /// If there are no resolved predictions, returns a zero-state score.
    pub fn calibration_score(&self) -> CalibrationScore {
        let resolved = self.resolved();
        let total_resolved = resolved.len();

        if total_resolved == 0 {
            return CalibrationScore {
                total_resolved: 0,
                correct: 0,
                hit_rate: 0.0,
                avg_days_early: 0.0,
                direction_bias: String::new(),
            };
        }

        let correct = resolved
            .iter()
            .filter(|(_, p)| {
                p.outcome.as_deref().map(|o| o.contains("TRUE")).unwrap_or(false)
            })
            .count();

        let hit_rate = correct as f64 / total_resolved as f64;

        // Parse avg_days_early from delta fields.
        // Looks for patterns like "N day(s) early" or "N day(s) late".
        let mut delta_sum: f64 = 0.0;
        let mut delta_count: usize = 0;

        for (_, pred) in &resolved {
            if let Some(delta_text) = &pred.delta {
                if let Some(days) = parse_days_from_delta(delta_text) {
                    delta_sum += days;
                    delta_count += 1;
                }
            }
        }

        let avg_days_early = if delta_count > 0 {
            delta_sum / delta_count as f64
        } else {
            0.0
        };

        let direction_bias = if hit_rate > 0.7 && avg_days_early > 0.5 {
            "optimistic".to_string()
        } else if hit_rate < 0.4 {
            "pessimistic".to_string()
        } else {
            "calibrated".to_string()
        };

        CalibrationScore {
            total_resolved,
            correct,
            hit_rate,
            avg_days_early,
            direction_bias,
        }
    }

    /// Format calibration data for injection into a system prompt.
    ///
    /// Returns `None` if there are no resolved predictions.
    pub fn format_calibration_for_system_prompt(&self) -> Option<String> {
        let score = self.calibration_score();
        score.format_for_system_prompt()
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

    // ── calibration ──────────────────────────────────────────────────────────────

    #[test]
    fn test_calibration_zero_resolved() {
        let (store, _dir) = tmp_store();
        let score = store.calibration_score();
        assert_eq!(score.total_resolved, 0);
        assert_eq!(score.correct, 0);
        assert_eq!(score.hit_rate, 0.0);
        assert_eq!(score.avg_days_early, 0.0);
        assert!(score.direction_bias.is_empty(), "empty store should have empty bias");
    }

    #[test]
    fn test_calibration_all_correct() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("prediction one");
        let id2 = store.predict("prediction two");
        let id3 = store.predict("prediction three");
        store.resolve(id1, "TRUE", None).unwrap();
        store.resolve(id2, "TRUE — exactly right", None).unwrap();
        store.resolve(id3, "TRUE", None).unwrap();
        let score = store.calibration_score();
        assert_eq!(score.total_resolved, 3);
        assert_eq!(score.correct, 3);
        assert!((score.hit_rate - 1.0).abs() < 1e-9, "hit_rate should be 1.0");
    }

    #[test]
    fn test_calibration_mixed() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("will succeed");
        let id2 = store.predict("will also succeed");
        let id3 = store.predict("will fail");
        store.resolve(id1, "TRUE", None).unwrap();
        store.resolve(id2, "TRUE", None).unwrap();
        store.resolve(id3, "FALSE", None).unwrap();
        let score = store.calibration_score();
        assert_eq!(score.total_resolved, 3);
        assert_eq!(score.correct, 2);
        let expected = 2.0 / 3.0;
        assert!((score.hit_rate - expected).abs() < 1e-9,
            "hit_rate should be ~0.667, got {}", score.hit_rate);
        assert_eq!(score.direction_bias, "calibrated");
    }

    #[test]
    fn test_calibration_format_some() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("will pass");
        store.resolve(id1, "TRUE", Some("1 day(s) early")).unwrap();
        let result = store.format_calibration_for_system_prompt();
        assert!(result.is_some(), "should return Some when there are resolved predictions");
        let text = result.unwrap();
        assert!(text.contains("Prediction Calibration"), "should contain section header");
        assert!(text.contains("1/1"), "should show 1/1 correct");
        assert!(text.contains("100.0%"), "should show 100% hit rate");
    }

    #[test]
    fn test_calibration_format_none() {
        let (store, _dir) = tmp_store();
        let result = store.format_calibration_for_system_prompt();
        assert!(result.is_none(), "should return None when 0 resolved predictions");
    }

    #[test]
    fn test_calibration_avg_days_early_early() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("will finish early");
        let id2 = store.predict("will also finish early");
        store.resolve(id1, "TRUE", Some("3 day(s) early")).unwrap();
        store.resolve(id2, "TRUE", Some("1 days early")).unwrap();
        let score = store.calibration_score();
        assert!((score.avg_days_early - 2.0).abs() < 1e-9,
            "avg_days_early should be 2.0, got {}", score.avg_days_early);
    }

    #[test]
    fn test_calibration_avg_days_late() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("will be late");
        store.resolve(id1, "FALSE", Some("2 day(s) late")).unwrap();
        let score = store.calibration_score();
        assert!((score.avg_days_early - (-2.0)).abs() < 1e-9,
            "avg_days_early should be -2.0 for late, got {}", score.avg_days_early);
    }

    #[test]
    fn test_calibration_no_delta_skipped() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("no delta");
        store.resolve(id1, "TRUE", None).unwrap();
        let score = store.calibration_score();
        assert_eq!(score.avg_days_early, 0.0, "no delta → avg_days_early should be 0.0");
    }

    #[test]
    fn test_calibration_direction_optimistic() {
        let (mut store, _dir) = tmp_store();
        for _ in 0..4 {
            let id = store.predict("will succeed");
            store.resolve(id, "TRUE", Some("2 days early")).unwrap();
        }
        let id5 = store.predict("this one too");
        store.resolve(id5, "TRUE", Some("1 days early")).unwrap();
        let score = store.calibration_score();
        // hit_rate = 1.0 > 0.7, avg_days_early > 0.5 → optimistic
        assert_eq!(score.direction_bias, "optimistic");
    }

    #[test]
    fn test_calibration_direction_pessimistic() {
        let (mut store, _dir) = tmp_store();
        for _ in 0..4 {
            let id = store.predict("will not succeed");
            store.resolve(id, "FALSE", None).unwrap();
        }
        let score = store.calibration_score();
        // hit_rate = 0.0 < 0.4 → pessimistic
        assert_eq!(score.direction_bias, "pessimistic");
    }

    #[test]
    fn test_parse_days_from_delta_early() {
        assert_eq!(parse_days_from_delta("1 day(s) early"), Some(1.0));
        assert_eq!(parse_days_from_delta("3 days early"), Some(3.0));
        assert_eq!(parse_days_from_delta("10 day early"), Some(10.0));
    }

    #[test]
    fn test_parse_days_from_delta_late() {
        assert_eq!(parse_days_from_delta("2 day(s) late"), Some(-2.0));
        assert_eq!(parse_days_from_delta("5 days late"), Some(-5.0));
    }

    #[test]
    fn test_parse_days_from_delta_unparseable() {
        assert_eq!(parse_days_from_delta("no timing info"), None);
        assert_eq!(parse_days_from_delta(""), None);
        assert_eq!(parse_days_from_delta("something happened"), None);
    }
}
