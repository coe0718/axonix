//! `MemoryStore` — the full persistent memory store backed by JSON + SQLite.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::db::AxonixDb;
use super::types::MemoryEntry;

/// The full memory store.
///
/// Use `MemoryStore::load()` or `MemoryStore::new()` to open.
/// Call `save()` after mutations to persist.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    /// Path to the JSON file.
    pub path: PathBuf,
    /// Path to the SQLite database used for write-through backing.
    /// Defaults to `.axonix/axonix.db` relative to the JSON file's parent.
    db_path: PathBuf,
    /// The in-memory map. BTreeMap for stable key ordering in JSON output.
    entries: BTreeMap<String, MemoryEntry>,
    /// Whether the store has unsaved changes.
    dirty: bool,
}

impl MemoryStore {
    /// Create a new store at the given path.
    ///
    /// Does NOT load from disk — call `load()` for that.
    /// Useful for in-memory testing.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path: PathBuf = path.into();
        let db_path = default_db_path_for(&path);
        Self {
            path,
            db_path,
            entries: BTreeMap::new(),
            dirty: false,
        }
    }

    /// Create a new store with an explicit SQLite database path.
    ///
    /// Used by tests to avoid touching `.axonix/axonix.db`.
    pub fn new_with_db(path: impl Into<PathBuf>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            db_path: db_path.into(),
            entries: BTreeMap::new(),
            dirty: false,
        }
    }

    /// Load the store from the default path (`.axonix/memory.json`).
    ///
    /// Prefers SQLite (`axonix.db`) when it has entries; falls back to JSON.
    /// If the file doesn't exist, returns an empty store.
    /// If the file exists but is malformed, logs a warning and returns empty.
    pub fn load_default() -> Self {
        let path = default_memory_path();
        Self::load_from(&path)
    }

    /// Load the store from a specific path.
    ///
    /// Tries SQLite first (if available and non-empty), then falls back to JSON.
    /// When both sources are available, SQLite values are used as authoritative
    /// while notes and timestamps are merged in from JSON (they are JSON-only metadata).
    /// Returns an empty store if neither source exists or can be parsed.
    pub fn load_from(path: &Path) -> Self {
        let mut store = Self::new(path.to_path_buf());

        // ── Try SQLite first ──────────────────────────────────────────────────
        let db_path = store.db_path.clone();
        if let Ok(db) = AxonixDb::open(&db_path) {
            if let Ok(pairs) = db.kv_list() {
                if !pairs.is_empty() {
                    // Load the SQLite KV pairs as the authoritative values.
                    for (key, value) in pairs {
                        store.entries.insert(
                            key,
                            MemoryEntry {
                                value,
                                note: None,
                                updated: None,
                            },
                        );
                    }
                    // Merge notes/timestamps from JSON if the file exists.
                    // JSON metadata (note, updated) enriches SQLite values.
                    if path.exists() {
                        if let Ok(content) = std::fs::read_to_string(path) {
                            if let Ok(json_entries) =
                                serde_json::from_str::<BTreeMap<String, MemoryEntry>>(&content)
                            {
                                for (key, json_entry) in json_entries {
                                    if let Some(entry) = store.entries.get_mut(&key) {
                                        // Overwrite value from SQLite is authoritative;
                                        // take note and updated from JSON.
                                        entry.note = json_entry.note;
                                        entry.updated = json_entry.updated;
                                    }
                                }
                            }
                        }
                    }
                    return store;
                }
            }
        }

        // ── Fall back to JSON ─────────────────────────────────────────────────
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str::<BTreeMap<String, MemoryEntry>>(&content) {
                        Ok(entries) => {
                            store.entries = entries;
                        }
                        Err(e) => {
                            eprintln!("  ⚠ memory: failed to parse {:?}: {e}", path);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  ⚠ memory: failed to read {:?}: {e}", path);
                }
            }
        }
        store
    }

    /// Save the store to disk.
    ///
    /// Creates parent directories if they don't exist.
    /// Returns an error string on failure.
    pub fn save(&mut self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("memory: failed to create dir {:?}: {e}", parent))?;
        }
        let json = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| format!("memory: serialization error: {e}"))?;
        std::fs::write(&self.path, json)
            .map_err(|e| format!("memory: failed to write {:?}: {e}", self.path))?;
        self.dirty = false;
        Ok(())
    }

    /// Get the value of a key, or `None` if not set.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|e| e.value.as_str())
    }

    /// Get the full entry for a key (including note and timestamp).
    pub fn get_entry(&self, key: &str) -> Option<&MemoryEntry> {
        self.entries.get(key)
    }

    /// Set a key to a value, with an optional note.
    ///
    /// Records the current UTC time as `updated`.
    /// Writes through to SQLite (`axonix.db`) in addition to marking dirty for
    /// the next `save()` call. SQLite failures are logged but never propagated.
    /// Does not auto-save JSON — call `save()` when done.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>, note: Option<&str>) {
        let key = key.into();
        let value_str: String = value.into();
        let entry = MemoryEntry {
            value: value_str.clone(),
            note: note.map(|s| s.to_string()),
            updated: Some(current_date()),
        };
        self.entries.insert(key.clone(), entry);
        self.dirty = true;

        // ── Write-through to SQLite ───────────────────────────────────────────
        if let Ok(db) = AxonixDb::open(&self.db_path) {
            if let Err(e) = db.kv_set(&key, &value_str) {
                eprintln!("  ⚠ memory: SQLite write failed for key {:?}: {e}", key);
            }
        }
    }

    /// Delete a key. Returns true if the key existed.
    ///
    /// Propagates the deletion to SQLite as well. SQLite failures are logged
    /// but never propagated — JSON remains authoritative.
    pub fn del(&mut self, key: &str) -> bool {
        let existed = self.entries.remove(key).is_some();
        if existed {
            self.dirty = true;

            // ── Propagate delete to SQLite ────────────────────────────────────
            if let Ok(db) = AxonixDb::open(&self.db_path) {
                if let Err(e) = db.kv_delete(key) {
                    eprintln!("  ⚠ memory: SQLite delete failed for key {:?}: {e}", key);
                }
            }
        }
        existed
    }

    /// List all keys in the store, sorted alphabetically.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Return all entries as a vec of (key, entry) pairs, sorted by key.
    pub fn all(&self) -> Vec<(&str, &MemoryEntry)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v)).collect()
    }

    /// Number of entries in the store.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if the store has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// True if the store has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Format all memory entries as a block suitable for injection into a system prompt.
    ///
    /// Returns `None` if the store is empty (so callers can skip appending).
    /// Format:
    ///   ## Operator Memory
    ///   key: value [note if present]
    ///
    /// Used by G-024: inject operator context at agent startup so every conversation
    /// is aware of stored facts without requiring a manual `/memory list` call.
    pub fn format_for_system_prompt(&self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let mut lines = vec!["## Operator Memory".to_string()];
        lines.push("Known facts about this operator and environment:".to_string());
        for (key, entry) in &self.entries {
            let note_part = entry
                .note
                .as_deref()
                .map(|n| format!(" [{}]", n))
                .unwrap_or_default();
            lines.push(format!("- {}: {}{}", key, entry.value, note_part));
        }
        Some(lines.join("\n"))
    }
}

/// Return the default path for the memory store.
///
/// Uses `AXONIX_MEMORY_PATH` env var if set, otherwise `.axonix/memory.json`
/// in the current working directory.
pub fn default_memory_path() -> PathBuf {
    if let Ok(path) = std::env::var("AXONIX_MEMORY_PATH") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    PathBuf::from(".axonix/memory.json")
}

/// Derive the default SQLite DB path given a JSON memory path.
///
/// Looks for `.axonix/` as the parent directory; if the JSON file lives there,
/// the DB lives alongside it as `axonix.db`. Otherwise uses `.axonix/axonix.db`
/// relative to the current working directory.
fn default_db_path_for(json_path: &Path) -> PathBuf {
    if let Some(parent) = json_path.parent() {
        if !parent.as_os_str().is_empty() {
            return parent.join("axonix.db");
        }
    }
    PathBuf::from(".axonix/axonix.db")
}

/// Return today's date as a compact string (YYYY-MM-DD).
///
/// Used to timestamp memory writes.
pub(crate) fn current_date() -> String {
    // Use the same unix_to_ymd algorithm from bluesky.rs (no chrono dep)
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day) = unix_to_ymd(secs);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Convert Unix timestamp to (year, month, day) UTC.
pub(crate) fn unix_to_ymd(secs: u64) -> (u32, u32, u32) {
    let days_total = secs / 86400;
    let z = days_total + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (if month <= 2 { y + 1 } else { y }) as u32;
    (year, month, day)
}
