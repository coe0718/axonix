//! Tests for `MemoryStore` and related utilities.

#[cfg(test)]
mod tests {
    use super::super::*;
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
        let d = crate::memory::store::current_date();
        // Must match YYYY-MM-DD
        assert_eq!(d.len(), 10, "date must be 10 chars: {d}");
        assert_eq!(&d[4..5], "-", "year-month separator: {d}");
        assert_eq!(&d[7..8], "-", "month-day separator: {d}");
    }

    #[test]
    fn test_current_date_year_reasonable() {
        let d = crate::memory::store::current_date();
        let year: u32 = d[..4].parse().unwrap();
        assert!(year >= 2024 && year <= 2100, "year should be reasonable: {year}");
    }

    // ── unix_to_ymd ───────────────────────────────────────────────────────────

    #[test]
    fn test_unix_to_ymd_epoch() {
        let (y, m, d) = crate::memory::store::unix_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_unix_to_ymd_known_date() {
        // 2026-03-16T00:00:00Z = 1773619200
        let (y, m, d) = crate::memory::store::unix_to_ymd(1773619200);
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
