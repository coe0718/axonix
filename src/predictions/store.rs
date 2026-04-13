//! `PredictionStore` — the main prediction CRUD and persistence layer.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::db::AxonixDb;
use super::types::Prediction;
use super::helpers::{extract_goal_ids, today_str, db_path_for};

pub struct PredictionStore {
    path: PathBuf,
    /// Path to `.axonix/axonix.db` — sibling of the JSON file.
    db_path: PathBuf,
    entries: BTreeMap<u32, Prediction>,
    next_id: u32,
}

impl PredictionStore {
    /// Create a new store at the given path. Loads existing data if the file exists.
    ///
    /// Derives the SQLite path as `<parent>/axonix.db` alongside the JSON file.
    pub fn new(path: PathBuf) -> Self {
        let db_path = db_path_for(&path);
        let mut store = Self {
            path,
            db_path,
            entries: BTreeMap::new(),
            next_id: 1,
        };
        store.load_if_exists();
        store
    }

    /// Create a new store with an explicit SQLite database path.
    ///
    /// Used by tests to avoid touching `.axonix/axonix.db`.
    pub fn new_with_db(path: PathBuf, db_path: PathBuf) -> Self {
        let mut store = Self {
            path,
            db_path,
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

    /// Load entries from disk.
    ///
    /// Tries SQLite first — if the DB exists and has rows, loads from it.
    /// Falls back to JSON if SQLite is empty or unavailable.
    fn load_if_exists(&mut self) {
        // ── Try SQLite first ──────────────────────────────────────────────────
        if let Ok(db) = AxonixDb::open(&self.db_path) {
            if let Ok(rows) = db.predictions_list() {
                if !rows.is_empty() {
                    for (id_str, prediction, created, outcome, delta, resolved) in rows {
                        if let Ok(id) = id_str.parse::<u32>() {
                            self.entries.insert(id, Prediction {
                                prediction,
                                created,
                                outcome,
                                delta,
                                resolved,
                            });
                            if id >= self.next_id {
                                self.next_id = id + 1;
                            }
                        }
                    }
                    return; // SQLite was authoritative — skip JSON
                }
            }
        }

        // ── Fall back to JSON ─────────────────────────────────────────────────
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

    /// Save entries to disk (JSON) and sync all rows to SQLite.
    ///
    /// JSON write always happens first for backward compatibility.
    /// SQLite sync failures are logged as warnings — never propagated.
    pub fn save(&self) -> Result<(), String> {
        // ── Write JSON ────────────────────────────────────────────────────────
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
        std::fs::write(&self.path, json).map_err(|e| format!("write error: {e}"))?;

        // ── Sync all rows to SQLite ───────────────────────────────────────────
        if let Ok(db) = AxonixDb::open(&self.db_path) {
            for (id, pred) in &self.entries {
                if let Err(e) = db.prediction_upsert(
                    &id.to_string(),
                    &pred.prediction,
                    &pred.created,
                    pred.outcome.as_deref(),
                    pred.delta.as_deref(),
                    pred.resolved.as_deref(),
                ) {
                    eprintln!("warning: prediction SQLite sync failed for #{id}: {e}");
                }
            }
        }

        Ok(())
    }

    /// Add a new prediction. Returns the assigned ID.
    ///
    /// Writes through to SQLite immediately; JSON is written on the next `save()`.
    pub fn predict(&mut self, text: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let today = today_str();
        let prediction = Prediction {
            prediction: text.to_string(),
            created: today.clone(),
            outcome: None,
            delta: None,
            resolved: None,
        };
        self.entries.insert(id, prediction);

        // ── Write-through to SQLite ───────────────────────────────────────────
        if let Ok(db) = AxonixDb::open(&self.db_path) {
            if let Err(e) = db.prediction_upsert(
                &id.to_string(),
                text,
                &today,
                None,
                None,
                None,
            ) {
                eprintln!("warning: prediction SQLite write-through failed: {e}");
            }
        }

        id
    }

    /// Resolve a prediction with an outcome and optional delta.
    /// Returns Ok with the prediction text if found, Err if not found.
    ///
    /// Writes through to SQLite immediately after updating in-memory state.
    pub fn resolve(&mut self, id: u32, outcome: &str, delta: Option<&str>) -> Result<String, String> {
        match self.entries.get_mut(&id) {
            Some(pred) => {
                if pred.is_resolved() {
                    return Err(format!("prediction #{id} is already resolved"));
                }
                pred.outcome = Some(outcome.to_string());
                pred.delta = delta.map(|s| s.to_string());
                pred.resolved = Some(today_str());
                let text = pred.prediction.clone();
                let created = pred.created.clone();
                let resolved_date = pred.resolved.clone();

                // ── Write-through to SQLite ───────────────────────────────────
                if let Ok(db) = AxonixDb::open(&self.db_path) {
                    if let Err(e) = db.prediction_upsert(
                        &id.to_string(),
                        &text,
                        &created,
                        Some(outcome),
                        delta,
                        resolved_date.as_deref(),
                    ) {
                        eprintln!("warning: prediction SQLite write-through failed: {e}");
                    }
                }

                Ok(text)
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

    // calibration_score, format_calibration_for_system_prompt, format_for_system_prompt
    // live in store_format.rs (included via predictions/mod.rs)

    /// Auto-resolve predictions that mention a goal ID (e.g. "G-120") when that goal
    /// is found as completed in a goals file (GOALS_ARCHIVE.md or GOALS.md).
    ///
    /// Returns a list of (id, prediction_text) for each newly resolved prediction.
    pub fn auto_resolve_from_goals(&mut self, goals_archive_content: &str) -> Vec<(u32, String)> {
        // Collect open predictions and their IDs up front (avoid borrow conflict)
        let open_list: Vec<(u32, Prediction)> = self
            .open()
            .into_iter()
            .map(|(id, p)| (id, p.clone()))
            .collect();

        let mut resolved = Vec::new();

        for (id, pred) in open_list {
            let goal_ids = extract_goal_ids(&pred.prediction);
            for goal_id in &goal_ids {
                let marker_bracket = format!("[x] [{goal_id}]");
                if goals_archive_content.contains(&marker_bracket) {
                    let outcome = format!(
                        "TRUE. {} was completed (found as [x] in goals archive).",
                        goal_id
                    );
                    let delta = format!("Goal {} verified complete in GOALS_ARCHIVE.md.", goal_id);
                    if self.resolve(id, &outcome, Some(&delta)).is_ok() {
                        resolved.push((id, pred.prediction.clone()));
                    }
                    break; // one goal match is enough
                }
            }
        }

        resolved
    }
}
