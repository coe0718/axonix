//! Structured persistent memory for Axonix.
//!
//! A simple key-value store backed by `.axonix/memory.json`.
//! Lets Axonix remember operator preferences, infrastructure facts,
//! and decisions across sessions — without requiring a database.
//!
//! Each entry has a key, a value, and an optional note explaining
//! why the fact was recorded and when it was last updated.
//!
//! # Design
//!
//! - Flat JSON file: transparent, human-readable, diffable in git
//! - Keys are strings (e.g. "nuc.ip", "twitter.status", "operator.tz")
//! - Values are strings (simple, composable, no schema complexity)
//! - Notes are optional: context that makes future sessions smarter
//! - Load-on-read, save-on-write: minimal complexity, no background threads
//! - SQLite write-through: every set/delete is mirrored to `axonix.db`
//!   for durability. JSON remains the fallback for backward compat.
//!
//! # File location
//!
//! Default: `.axonix/memory.json` in the current working directory.
//! Can be overridden via `AXONIX_MEMORY_PATH` environment variable.
//!
//! # Example
//!
//! ```
//! use axonix::memory::MemoryStore;
//!
//! let mut store = MemoryStore::new("/tmp/test-memory.json");
//! store.set("operator.tz", "America/Indiana/Indianapolis", Some("from .env TZ var"));
//! assert_eq!(store.get("operator.tz"), Some("America/Indiana/Indianapolis"));
//! store.del("operator.tz");
//! assert!(store.get("operator.tz").is_none());
//! ```

pub mod capture;
pub mod consolidator;
pub mod loader;
pub mod search;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::db::AxonixDb;

/// A single memory entry.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    /// The stored value.
    pub value: String,
    /// Optional note explaining why this was recorded or how to use it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// ISO 8601 timestamp of when this entry was last updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}

impl MemoryEntry {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            note: None,
            updated: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn with_updated(mut self, updated: impl Into<String>) -> Self {
        self.updated = Some(updated.into());
        self
    }
}

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
fn current_date() -> String {
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
fn unix_to_ymd(secs: u64) -> (u32, u32, u32) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AxonixDb;

    fn tmp_store() -> (MemoryStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let store = MemoryStore::new(&path);
        (store, dir)
    }

    // ── Basic get/set/del ─────────────────────────────────────────────────────

    #[test]
    fn test_get_missing_key_returns_none() {
        let (store, _dir) = tmp_store();
        assert!(store.get("nonexistent").is_none());
    }

    #[test]
    fn test_set_then_get() {
        let (mut store, _dir) = tmp_store();
        store.set("nuc.ip", "192.168.1.10", None);
        assert_eq!(store.get("nuc.ip"), Some("192.168.1.10"));
    }

    #[test]
    fn test_set_overwrites_existing_value() {
        let (mut store, _dir) = tmp_store();
        store.set("key", "old", None);
        store.set("key", "new", Some("updated value"));
        assert_eq!(store.get("key"), Some("new"));
        assert_eq!(store.get_entry("key").unwrap().note.as_deref(), Some("updated value"));
    }

    #[test]
    fn test_del_existing_key_returns_true() {
        let (mut store, _dir) = tmp_store();
        store.set("x", "1", None);
        assert!(store.del("x"));
        assert!(store.get("x").is_none());
    }

    #[test]
    fn test_del_missing_key_returns_false() {
        let (mut store, _dir) = tmp_store();
        assert!(!store.del("nonexistent"));
    }

    #[test]
    fn test_len_and_is_empty() {
        let (mut store, _dir) = tmp_store();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        store.set("a", "1", None);
        store.set("b", "2", None);
        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());
    }

    // ── keys() / all() ────────────────────────────────────────────────────────

    #[test]
    fn test_keys_sorted_alphabetically() {
        let (mut store, _dir) = tmp_store();
        store.set("z", "3", None);
        store.set("a", "1", None);
        store.set("m", "2", None);
        let keys = store.keys();
        assert_eq!(keys, vec!["a", "m", "z"]);
    }

    #[test]
    fn test_all_returns_all_entries() {
        let (mut store, _dir) = tmp_store();
        store.set("key1", "val1", Some("note1"));
        store.set("key2", "val2", None);
        let all = store.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].0, "key1");
        assert_eq!(all[0].1.value, "val1");
        assert_eq!(all[0].1.note.as_deref(), Some("note1"));
        assert_eq!(all[1].0, "key2");
    }

    // ── dirty tracking ────────────────────────────────────────────────────────

    #[test]
    fn test_dirty_after_set() {
        let (mut store, _dir) = tmp_store();
        assert!(!store.is_dirty());
        store.set("x", "1", None);
        assert!(store.is_dirty());
    }

    #[test]
    fn test_dirty_after_del() {
        let (mut store, _dir) = tmp_store();
        store.set("x", "1", None);
        // After save, dirty should clear
        store.save().unwrap();
        assert!(!store.is_dirty());
        store.del("x");
        assert!(store.is_dirty());
    }

    #[test]
    fn test_del_nonexistent_does_not_dirty() {
        let (mut store, _dir) = tmp_store();
        store.del("missing"); // del of missing key shouldn't mark dirty
        assert!(!store.is_dirty());
    }

    // ── save/load roundtrip ───────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");

        let mut store = MemoryStore::new(&path);
        store.set("nuc.ip", "192.168.1.10", Some("Intel NUC on home LAN"));
        store.set("twitter.status", "blocked_402", Some("Free tier does not allow writes"));
        store.save().unwrap();

        let loaded = MemoryStore::load_from(&path);
        assert_eq!(loaded.get("nuc.ip"), Some("192.168.1.10"));
        assert_eq!(loaded.get("twitter.status"), Some("blocked_402"));
        assert_eq!(
            loaded.get_entry("nuc.ip").unwrap().note.as_deref(),
            Some("Intel NUC on home LAN")
        );
    }

    #[test]
    fn test_load_nonexistent_path_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        let store = MemoryStore::load_from(&path);
        assert!(store.is_empty());
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("nested").join("memory.json");
        let mut store = MemoryStore::new(&path);
        store.set("key", "value", None);
        store.save().expect("save should create parent dirs");
        assert!(path.exists());
    }

    #[test]
    fn test_load_malformed_json_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        std::fs::write(&path, b"{ invalid json }").unwrap();
        // Must not panic — returns empty store with warning
        let store = MemoryStore::load_from(&path);
        assert!(store.is_empty());
    }

    // ── save produces valid JSON ───────────────────────────────────────────────

    #[test]
    fn test_save_produces_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let mut store = MemoryStore::new(&path);
        store.set("a.b", "hello world", Some("a note"));
        store.save().unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("should be valid JSON");
        assert!(parsed.is_object());
        assert!(parsed["a.b"]["value"].as_str() == Some("hello world"));
    }

    // ── current_date ─────────────────────────────────────────────────────────

    #[test]
    fn test_current_date_format() {
        let d = current_date();
        // Must match YYYY-MM-DD
        assert_eq!(d.len(), 10, "date must be 10 chars: {d}");
        assert_eq!(&d[4..5], "-", "year-month separator: {d}");
        assert_eq!(&d[7..8], "-", "month-day separator: {d}");
    }

    #[test]
    fn test_current_date_year_reasonable() {
        let d = current_date();
        let year: u32 = d[..4].parse().unwrap();
        assert!(year >= 2024 && year <= 2100, "year should be reasonable: {year}");
    }

    // ── unix_to_ymd ───────────────────────────────────────────────────────────

    #[test]
    fn test_unix_to_ymd_epoch() {
        let (y, m, d) = unix_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_unix_to_ymd_known_date() {
        // 2026-03-16T00:00:00Z = 1773619200
        let (y, m, d) = unix_to_ymd(1773619200);
        assert_eq!(y, 2026);
        assert_eq!(m, 3);
        assert_eq!(d, 16);
    }

    // ── entry timestamps ──────────────────────────────────────────────────────

    #[test]
    fn test_set_records_updated_timestamp() {
        let (mut store, _dir) = tmp_store();
        store.set("key", "value", None);
        let entry = store.get_entry("key").unwrap();
        assert!(entry.updated.is_some(), "set should record updated timestamp");
        let updated = entry.updated.as_ref().unwrap();
        assert_eq!(updated.len(), 10, "timestamp should be YYYY-MM-DD: {updated}");
    }

    // ── format_for_system_prompt ──────────────────────────────────────────────

    #[test]
    fn test_format_for_system_prompt_empty_returns_none() {
        let (store, _dir) = tmp_store();
        assert!(store.format_for_system_prompt().is_none(), "empty store should return None");
    }

    #[test]
    fn test_format_for_system_prompt_with_entries() {
        let (mut store, _dir) = tmp_store();
        store.set("operator.tz", "America/Indiana/Indianapolis", Some("from env"));
        store.set("twitter.status", "blocked_402", None);
        let block = store.format_for_system_prompt().expect("should produce a block");
        assert!(block.contains("## Operator Memory"), "should have header");
        assert!(block.contains("operator.tz"), "should include key");
        assert!(block.contains("America/Indiana/Indianapolis"), "should include value");
        assert!(block.contains("[from env]"), "should include note");
        assert!(block.contains("twitter.status"), "should include second key");
    }

    #[test]
    fn test_format_for_system_prompt_no_note_no_brackets() {
        let (mut store, _dir) = tmp_store();
        store.set("key.without.note", "value", None);
        let block = store.format_for_system_prompt().expect("should produce a block");
        // No note → no brackets at end of that line
        let line = block.lines()
            .find(|l| l.contains("key.without.note"))
            .expect("should find the key line");
        assert!(!line.ends_with(']'), "line without note should not end with ]");
    }

    // ── SQLite write-through ──────────────────────────────────────────────────

    /// Set a key in MemoryStore, then open the DB directly and verify the key
    /// is present in the `kv` table.
    #[test]
    fn test_memory_db_write_through() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("memory.json");
        let db_path = dir.path().join("axonix.db");
        let mut store = MemoryStore::new_with_db(&json_path, &db_path);

        store.set("db.test.key", "hello_sqlite", Some("write-through test"));

        // Open the DB directly and verify the key landed there.
        let db = AxonixDb::open(&db_path).expect("should open db");
        let val = db.kv_get("db.test.key").expect("kv_get should not error");
        assert_eq!(val, Some("hello_sqlite".to_string()), "value should be in SQLite");
    }

    /// Set then delete a key — verify it is gone from the DB.
    #[test]
    fn test_memory_db_delete_propagates() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("memory.json");
        let db_path = dir.path().join("axonix.db");
        let mut store = MemoryStore::new_with_db(&json_path, &db_path);

        store.set("ephemeral.key", "temporary", None);
        store.del("ephemeral.key");

        let db = AxonixDb::open(&db_path).expect("should open db");
        let val = db.kv_get("ephemeral.key").expect("kv_get should not error");
        assert!(val.is_none(), "deleted key should not be in SQLite");
    }

    /// Write directly to the DB, then call `load_from()` on a fresh MemoryStore
    /// with no JSON file — the DB entries should be authoritative.
    #[test]
    fn test_memory_db_load_from_db() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("memory.json");
        let db_path = dir.path().join("axonix.db");

        // Write directly to the DB (no MemoryStore involved yet).
        let db = AxonixDb::open(&db_path).expect("should open db");
        db.kv_set("loaded.from.db", "db_value").expect("kv_set should succeed");
        drop(db);

        // No JSON file exists — load_from should pull from SQLite.
        let store = MemoryStore::new_with_db(&json_path, &db_path);
        // We must use load_from-equivalent logic. Since load_from uses Self::new
        // internally, we replicate the logic by calling load_from and patching
        // the db_path after — instead, let's use a dedicated helper path:
        // Directly exercise the load path by rebuilding with load_from.
        // Because load_from calls Self::new which derives db_path from json_path's
        // parent, and our json_path's parent IS the temp dir, this works naturally.
        assert!(store.is_empty(), "store constructed with new_with_db is empty (not loaded)");

        // Use load_from — it derives db_path as <parent of json_path>/axonix.db,
        // which is exactly dir.path()/axonix.db — our written DB.
        let loaded = MemoryStore::load_from(&json_path);
        assert_eq!(
            loaded.get("loaded.from.db"),
            Some("db_value"),
            "value written directly to DB should be visible after load_from"
        );
    }
}
