//! SQLite-backed structured memory for Axonix (G-075, Issue #91).
//!
//! Provides four tables:
//! - `kv`          — key/value store for agent state
//! - `sessions`    — per-session records (day, tokens, tests, notes)
//! - `goals`       — goal tracking (active / backlog / done)
//! - `predictions` — prediction tracking with outcome/delta/resolved (G-077)
//!
//! # Example
//! ```rust,no_run
//! use std::path::Path;
//! use axonix::db::AxonixDb;
//!
//! let db = AxonixDb::open(Path::new("/tmp/axonix.db")).unwrap();
//! db.kv_set("last_issue", "91").unwrap();
//! assert_eq!(db.kv_get("last_issue").unwrap(), Some("91".to_string()));
//! ```

use rusqlite::{Connection, Result, params};
use std::path::Path;

// ─── Row types ───────────────────────────────────────────────────────────────

/// A row from the `sessions` table.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: i64,
    pub day: i64,
    pub session: String,
    pub date: String,
    pub tokens: Option<String>,
    pub tests: Option<i64>,
    pub failed: Option<i64>,
    pub notes: Option<String>,
}

/// A row from the `goals` table.
#[derive(Debug, Clone)]
pub struct GoalRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

// ─── Main struct ─────────────────────────────────────────────────────────────

/// SQLite-backed structured memory store.
pub struct AxonixDb {
    conn: Connection,
}

impl AxonixDb {
    /// Open (or create) the database at the given path and run migrations.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Open at the default path: `.axonix/axonix.db`.
    pub fn open_default() -> Result<Self> {
        Self::open(Path::new(".axonix/axonix.db"))
    }

    /// Run schema migrations (idempotent — uses `IF NOT EXISTS`).
    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch("
            CREATE TABLE IF NOT EXISTS kv (
                key        TEXT PRIMARY KEY,
                value      TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                day        INTEGER NOT NULL,
                session    TEXT NOT NULL,
                date       TEXT NOT NULL,
                tokens     TEXT,
                tests      INTEGER,
                failed     INTEGER,
                notes      TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS goals (
                id           TEXT PRIMARY KEY,
                title        TEXT NOT NULL,
                status       TEXT NOT NULL CHECK(status IN ('active','backlog','done')),
                created_at   TEXT NOT NULL,
                completed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS predictions (
                id         TEXT PRIMARY KEY,
                prediction TEXT NOT NULL,
                created    TEXT NOT NULL,
                outcome    TEXT,
                delta      TEXT,
                resolved   TEXT
            );
        ")
    }

    // ─── KV helpers ──────────────────────────────────────────────────────────

    /// Upsert a key-value pair.
    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "INSERT OR REPLACE INTO kv (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, now],
        )?;
        Ok(())
    }

    /// Get a value by key. Returns `None` if the key does not exist.
    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM kv WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Delete a key. Returns `true` if the key existed and was removed.
    pub fn kv_delete(&self, key: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM kv WHERE key = ?1", params![key])?;
        Ok(n > 0)
    }

    /// List all key-value pairs as `(key, value)` tuples.
    pub fn kv_list(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM kv ORDER BY key")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    // ─── Session helpers ─────────────────────────────────────────────────────

    /// Insert a session record and return its new row id.
    #[allow(clippy::too_many_arguments)]
    pub fn session_insert(
        &self,
        day: i64,
        session: &str,
        date: &str,
        tokens: Option<&str>,
        tests: Option<i64>,
        failed: Option<i64>,
        notes: Option<&str>,
    ) -> Result<i64> {
        let now = now_utc();
        self.conn.execute(
            "INSERT INTO sessions (day, session, date, tokens, tests, failed, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![day, session, date, tokens, tests, failed, notes, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return the most recent `limit` sessions, ordered by (day DESC, session DESC).
    pub fn sessions_recent(&self, limit: usize) -> Result<Vec<SessionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, day, session, date, tokens, tests, failed, notes
             FROM sessions
             ORDER BY day DESC, session DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                day: row.get(1)?,
                session: row.get(2)?,
                date: row.get(3)?,
                tokens: row.get(4)?,
                tests: row.get(5)?,
                failed: row.get(6)?,
                notes: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    // ─── Goal helpers ─────────────────────────────────────────────────────────

    /// Upsert a goal (insert or replace). `status` must be one of
    /// `"active"`, `"backlog"`, or `"done"`.
    pub fn goal_upsert(&self, id: &str, title: &str, status: &str) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "INSERT OR REPLACE INTO goals (id, title, status, created_at, completed_at)
             VALUES (
                 ?1, ?2, ?3, ?4,
                 CASE WHEN ?3 = 'done' THEN ?4 ELSE NULL END
             )",
            params![id, title, status, now],
        )?;
        Ok(())
    }

    /// Get a goal by ID. Returns `None` if not found.
    pub fn goal_get(&self, id: &str) -> Result<Option<GoalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, created_at, completed_at FROM goals WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(GoalRow {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                completed_at: row.get(4)?,
            })),
            None => Ok(None),
        }
    }

    /// List goals filtered by `status`.
    pub fn goals_by_status(&self, status: &str) -> Result<Vec<GoalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, created_at, completed_at
             FROM goals WHERE status = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![status], |row| {
            Ok(GoalRow {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                completed_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Mark a goal as done, setting `completed_at` to now.
    /// Returns `true` if the goal existed and was updated.
    pub fn goal_complete(&self, id: &str) -> Result<bool> {
        let now = now_utc();
        let n = self.conn.execute(
            "UPDATE goals SET status = 'done', completed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(n > 0)
    }

    // ─── Prediction helpers ───────────────────────────────────────────────────

    /// Upsert a prediction row. Called on every predict/resolve.
    pub fn prediction_upsert(
        &self,
        id: &str,
        prediction: &str,
        created: &str,
        outcome: Option<&str>,
        delta: Option<&str>,
        resolved: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO predictions (id, prediction, created, outcome, delta, resolved)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               prediction = excluded.prediction,
               created    = excluded.created,
               outcome    = excluded.outcome,
               delta      = excluded.delta,
               resolved   = excluded.resolved",
            params![id, prediction, created, outcome, delta, resolved],
        )?;
        Ok(())
    }

    /// List all predictions ordered by id (numeric sort by casting to int).
    ///
    /// Returns raw tuples of `(id, prediction_text, created, outcome, delta, resolved)`
    /// to avoid a circular dependency between `db` and `predictions` modules.
    pub fn predictions_list(
        &self,
    ) -> Result<Vec<(String, String, String, Option<String>, Option<String>, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, prediction, created, outcome, delta, resolved
             FROM predictions
             ORDER BY CAST(id AS INTEGER)",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.collect()
    }
}

// ─── Utilities ───────────────────────────────────────────────────────────────

/// Return current UTC time formatted as `YYYY-MM-DDTHH:MM:SSZ` using only std.
fn now_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Break into components manually (no external deps needed for tests).
    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert Unix timestamp (seconds) to (year, month, day, hour, min, sec).
/// Gregorian calendar, no leap-second handling — good enough for timestamps.
fn epoch_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;

    // Days since 1970-01-01
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if remaining < dy {
            break;
        }
        remaining -= dy;
        y += 1;
    }
    let months = if is_leap(y) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 1u64;
    for &dm in &months {
        if remaining < dm {
            break;
        }
        remaining -= dm;
        mo += 1;
    }
    let d = remaining + 1;
    (y, mo, d, h, m, s)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_tmp() -> AxonixDb {
        let dir = tempdir().unwrap();
        let path = dir.into_path().join("test.db");
        AxonixDb::open(&path).unwrap()
    }

    // ── KV ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_kv_set_and_get() {
        let db = open_tmp();
        db.kv_set("hello", "world").unwrap();
        assert_eq!(db.kv_get("hello").unwrap(), Some("world".to_string()));
    }

    #[test]
    fn test_kv_overwrite() {
        let db = open_tmp();
        db.kv_set("key", "first").unwrap();
        db.kv_set("key", "second").unwrap();
        assert_eq!(db.kv_get("key").unwrap(), Some("second".to_string()));
    }

    #[test]
    fn test_kv_get_missing() {
        let db = open_tmp();
        assert_eq!(db.kv_get("nonexistent").unwrap(), None);
    }

    #[test]
    fn test_kv_delete() {
        let db = open_tmp();
        db.kv_set("delete_me", "value").unwrap();
        assert!(db.kv_delete("delete_me").unwrap());
        assert_eq!(db.kv_get("delete_me").unwrap(), None);
    }

    #[test]
    fn test_kv_delete_missing() {
        let db = open_tmp();
        assert!(!db.kv_delete("ghost_key").unwrap());
    }

    #[test]
    fn test_kv_list() {
        let db = open_tmp();
        db.kv_set("alpha", "1").unwrap();
        db.kv_set("beta", "2").unwrap();
        db.kv_set("gamma", "3").unwrap();
        let list = db.kv_list().unwrap();
        assert_eq!(list.len(), 3);
        // Ordered by key
        assert_eq!(list[0].0, "alpha");
        assert_eq!(list[1].0, "beta");
        assert_eq!(list[2].0, "gamma");
    }

    // ── Sessions ─────────────────────────────────────────────────────────────

    #[test]
    fn test_session_insert_and_query() {
        let db = open_tmp();
        let id = db
            .session_insert(75, "S1", "2025-01-01", Some("100k"), Some(42), Some(0), Some("all good"))
            .unwrap();
        assert!(id > 0);
        let rows = db.sessions_recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.day, 75);
        assert_eq!(row.session, "S1");
        assert_eq!(row.tokens, Some("100k".to_string()));
        assert_eq!(row.tests, Some(42));
        assert_eq!(row.failed, Some(0));
        assert_eq!(row.notes, Some("all good".to_string()));
    }

    #[test]
    fn test_sessions_recent_limit() {
        let db = open_tmp();
        for i in 1..=5u64 {
            db.session_insert(i as i64, "S1", "2025-01-01", None, None, None, None)
                .unwrap();
        }
        let rows = db.sessions_recent(3).unwrap();
        assert_eq!(rows.len(), 3);
    }

    // ── Goals ────────────────────────────────────────────────────────────────

    #[test]
    fn test_goal_upsert_and_get() {
        let db = open_tmp();
        db.goal_upsert("G-075", "SQLite memory", "active").unwrap();
        let goal = db.goal_get("G-075").unwrap().expect("goal should exist");
        assert_eq!(goal.id, "G-075");
        assert_eq!(goal.title, "SQLite memory");
        assert_eq!(goal.status, "active");
        assert!(goal.completed_at.is_none());
    }

    #[test]
    fn test_goal_complete() {
        let db = open_tmp();
        db.goal_upsert("G-010", "Some goal", "active").unwrap();
        let updated = db.goal_complete("G-010").unwrap();
        assert!(updated);
        let goal = db.goal_get("G-010").unwrap().expect("goal should exist");
        assert_eq!(goal.status, "done");
        assert!(goal.completed_at.is_some());
    }

    #[test]
    fn test_goal_complete_nonexistent() {
        let db = open_tmp();
        let updated = db.goal_complete("G-999").unwrap();
        assert!(!updated);
    }

    #[test]
    fn test_goals_by_status() {
        let db = open_tmp();
        db.goal_upsert("G-001", "Active goal 1", "active").unwrap();
        db.goal_upsert("G-002", "Active goal 2", "active").unwrap();
        db.goal_upsert("G-003", "Backlog goal",  "backlog").unwrap();
        db.goal_upsert("G-004", "Done goal",     "done").unwrap();

        let active = db.goals_by_status("active").unwrap();
        assert_eq!(active.len(), 2);

        let backlog = db.goals_by_status("backlog").unwrap();
        assert_eq!(backlog.len(), 1);
        assert_eq!(backlog[0].id, "G-003");

        let done = db.goals_by_status("done").unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].id, "G-004");
    }
}
