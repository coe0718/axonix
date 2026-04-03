//! SQLite-backed structured memory for Axonix (G-075, Issue #91).
//!
//! Provides nine tables:
//! - `kv`                      — key/value store for agent state
//! - `sessions`                — per-session records (day, tokens, tests, notes)
//! - `goals`                   — goal tracking (active / backlog / done)
//! - `predictions`             — prediction tracking with outcome/delta/resolved (G-077)
//! - `observations`            — keyword-searchable observations with tags (G-088)
//! - `hot_memories`            — recent extracted memories with TTL (30 days)
//! - `cold_memories`           — synthesized memories with TTL (90 days)
//! - `memory_contradictions`   — conflict tracking between cold memories
//! - `structured_observations` — categorised observations with goal/session metadata (Issue #104)
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

/// A row from the `observations` table, with a computed relevance score.
#[derive(Debug, Clone)]
pub struct ObservationRow {
    pub id: i64,
    pub key: String,
    pub text: String,
    pub tags: String,
    pub created_at: String,
    /// Relevance score set by `search_memory`; not stored in DB.
    pub score: f64,
}

/// A row from the `structured_observations` table.
#[derive(Debug, Clone)]
pub struct StructuredObservation {
    pub id: i64,
    pub content: String,
    /// One of: learned | tried_and_failed | blocked_by | dependency_discovered | pattern_noticed
    pub category: String,
    /// Source file, or empty string if unknown.
    pub source_file: String,
    /// Goal ID, e.g. "G-110", or empty string if unknown.
    pub goal_id: String,
    /// Session label, e.g. "Day 19 S1".
    pub session: String,
    /// Comma-separated tags.
    pub tags: String,
    pub created_at: String,
}

/// A row from the `hot_memories` table.
#[derive(Debug, Clone)]
pub struct HotMemoryRow {
    pub id: i64,
    pub content: String,
    pub summary: String,
    pub entities: String,
    pub topics: String,
    pub importance: f64,
    pub created_at: String,
    pub last_accessed: String,
    pub access_count: i64,
    pub expires_at: String,
}

/// A row from the `cold_memories` table.
#[derive(Debug, Clone)]
pub struct ColdMemoryRow {
    pub id: i64,
    pub content: String,
    pub topics: String,
    pub importance: f64,
    pub created_at: String,
    pub reinforcement_count: i64,
    pub last_reinforced: String,
    pub expires_at: String,
}

/// A row from the `memory_contradictions` table.
#[derive(Debug, Clone)]
pub struct ContradictionRow {
    pub id: i64,
    pub cold_memory_id: i64,
    pub new_memory: String,
    pub created_at: String,
    pub resolved: bool,
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

            CREATE TABLE IF NOT EXISTS observations (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                key        TEXT NOT NULL,
                text       TEXT NOT NULL,
                tags       TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                UNIQUE(key)
            );

            CREATE TABLE IF NOT EXISTS hot_memories (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                content       TEXT NOT NULL,
                summary       TEXT NOT NULL,
                entities      TEXT NOT NULL DEFAULT '[]',
                topics        TEXT NOT NULL DEFAULT '[]',
                importance    REAL NOT NULL DEFAULT 0.5,
                created_at    TEXT NOT NULL,
                last_accessed TEXT NOT NULL,
                access_count  INTEGER NOT NULL DEFAULT 0,
                expires_at    TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS cold_memories (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                content             TEXT NOT NULL,
                topics              TEXT NOT NULL DEFAULT '[]',
                importance          REAL NOT NULL DEFAULT 0.5,
                created_at          TEXT NOT NULL,
                reinforcement_count INTEGER NOT NULL DEFAULT 0,
                last_reinforced     TEXT NOT NULL,
                expires_at          TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS memory_contradictions (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                cold_memory_id INTEGER NOT NULL,
                new_memory     TEXT NOT NULL,
                created_at     TEXT NOT NULL,
                resolved       INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS structured_observations (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                content     TEXT NOT NULL,
                category    TEXT NOT NULL DEFAULT 'learned',
                source_file TEXT NOT NULL DEFAULT '',
                goal_id     TEXT NOT NULL DEFAULT '',
                session     TEXT NOT NULL DEFAULT '',
                tags        TEXT NOT NULL DEFAULT '',
                created_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS embeddings (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                obs_key     TEXT NOT NULL,
                source_type TEXT NOT NULL DEFAULT 'observation',
                vector      BLOB NOT NULL,
                created_at  TEXT NOT NULL,
                UNIQUE(obs_key)
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

    // ─── Observation helpers ──────────────────────────────────────────────────

    /// Store (insert or replace) an observation by key.
    /// `tags` is a comma-separated list of topic tags (e.g. `"repl,slash-command,bug"`).
    ///
    /// After storing, best-effort embeds the text via Ollama for semantic search.
    /// If Ollama is unavailable, the observation is still stored — embedding is skipped.
    pub fn observation_store(&self, key: &str, text: &str, tags: &str) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "INSERT INTO observations (key, text, tags, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key) DO UPDATE SET
               text       = excluded.text,
               tags       = excluded.tags",
            // created_at intentionally NOT updated — preserve original timestamp
            params![key, text, tags, now],
        )?;
        // Best-effort: embed and store vector for semantic search
        if let Ok(vec) = crate::embeddings::embed(text) {
            let _ = self.embedding_store(key, &vec);
        }
        Ok(())
    }

    /// Return up to `limit` observations most relevant to `query`.
    ///
    /// Uses keyword overlap scoring:
    /// - tag token matches contribute 2.0 each
    /// - text token matches contribute 1.0 each
    ///
    /// Only rows with score > 0 are returned.  Tie-breaks by most-recently
    /// created first.
    pub fn search_memory(&self, query: &str, limit: usize) -> Result<Vec<ObservationRow>> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        // Fetch all observations
        let all = self.observations_list(usize::MAX)?;

        // Score each row
        let mut scored: Vec<ObservationRow> = all
            .into_iter()
            .filter_map(|mut row| {
                let text_tokens = tokenize(&row.text);
                let tag_tokens = tokenize(&row.tags);
                let mut score = 0.0f64;
                for qt in &query_tokens {
                    if text_tokens.contains(qt) {
                        score += 1.0;
                    }
                    if tag_tokens.contains(qt) {
                        score += 2.0;
                    }
                }
                if score > 0.0 {
                    row.score = score;
                    Some(row)
                } else {
                    None
                }
            })
            .collect();

        // Sort: score DESC, then created_at DESC
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.created_at.cmp(&a.created_at))
        });

        scored.truncate(limit);
        Ok(scored)
    }

    /// Parse `journal_path` (a `JOURNAL.md`-format file) and upsert each `## ` section
    /// as an observation.
    ///
    /// Key  = `journal:` + lowercase slug of the heading, truncated to 80 chars.
    /// Tags = `"journal"`.
    /// Text = heading line + `"\n"` + body, truncated to 800 chars.
    ///
    /// The operation is idempotent: calling it twice on the same file produces
    /// the same DB state (upsert by key).
    ///
    /// Returns the number of entries stored (including upserted duplicates).
    pub fn seed_from_journal(&self, journal_path: &std::path::Path) -> Result<usize> {
        let content = match std::fs::read_to_string(journal_path) {
            Ok(s) => s,
            Err(e) => {
                return Err(rusqlite::Error::InvalidParameterName(
                    format!("cannot read journal: {e}"),
                ));
            }
        };

        // Split on "\n## " to get sections (first chunk may be a preamble).
        // We also handle a file that starts with "## " directly.
        let raw_sections: Vec<&str> = content.split("\n## ").collect();

        let mut count = 0usize;
        for (i, section) in raw_sections.iter().enumerate() {
            // The very first chunk from split("\n## ") still has its "## " prefix
            // only if the file starts with "## " — otherwise the first chunk is
            // whatever precedes the first "## " heading (e.g. "# Journal\n\n").
            let section_trimmed = if i == 0 {
                // Strip a possible leading "## " if the file starts with it.
                section.strip_prefix("## ").unwrap_or(section)
            } else {
                section
            };

            // First line is the heading; the rest is the body.
            let mut lines = section_trimmed.splitn(2, '\n');
            let heading = lines.next().unwrap_or("").trim();
            let body = lines.next().unwrap_or("").trim();

            // Skip sections with an empty heading (e.g. the preamble before the first "## ")
            // or preamble sections starting with a title-level "#" heading.
            if heading.is_empty() || heading.starts_with('#') {
                continue;
            }

            // Build the key slug: lowercase, non-alphanumeric → '-', truncate to 80 chars.
            let slug: String = heading
                .chars()
                .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
                .collect::<String>()
                .chars()
                .take(80)
                .collect();
            let key = format!("journal:{slug}");

            // Build the text: heading + newline + body, truncated to 800 chars.
            let combined = format!("{heading}\n{body}");
            let text: String = combined.chars().take(800).collect();

            self.observation_store(&key, &text, "journal")?;
            count += 1;
        }

        Ok(count)
    }

    /// Return all observations, most recent first.
    pub fn observations_list(&self, limit: usize) -> Result<Vec<ObservationRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, key, text, tags, created_at
             FROM observations
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ObservationRow {
                id: row.get(0)?,
                key: row.get(1)?,
                text: row.get(2)?,
                tags: row.get(3)?,
                created_at: row.get(4)?,
                score: 0.0,
            })
        })?;
        rows.collect()
    }

    // ─── Hot memory helpers ───────────────────────────────────────────────────

    /// Insert a hot memory. expires_at is set to 30 days from now.
    pub fn hot_memory_insert(
        &self,
        content: &str,
        summary: &str,
        entities: &str,
        topics: &str,
        importance: f64,
    ) -> Result<i64> {
        let now = now_utc();
        let expires_at = add_days_to_utc(&now, 30);
        self.conn.execute(
            "INSERT INTO hot_memories
             (content, summary, entities, topics, importance, created_at, last_accessed, access_count, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)",
            params![content, summary, entities, topics, importance, now, now, expires_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return up to `limit` hot memories, most recent first.
    pub fn hot_memories_list(&self, limit: usize) -> Result<Vec<HotMemoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, summary, entities, topics, importance,
                    created_at, last_accessed, access_count, expires_at
             FROM hot_memories
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(HotMemoryRow {
                id: row.get(0)?,
                content: row.get(1)?,
                summary: row.get(2)?,
                entities: row.get(3)?,
                topics: row.get(4)?,
                importance: row.get(5)?,
                created_at: row.get(6)?,
                last_accessed: row.get(7)?,
                access_count: row.get(8)?,
                expires_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    /// Keyword search over hot memories (content + summary + topics).
    /// Uses the same tokenize() approach as search_memory().
    pub fn hot_memory_search(&self, query: &str, limit: usize) -> Result<Vec<HotMemoryRow>> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        let all = self.hot_memories_list(usize::MAX)?;
        let mut scored: Vec<(f64, HotMemoryRow)> = all
            .into_iter()
            .filter_map(|row| {
                let content_tokens = tokenize(&row.content);
                let summary_tokens = tokenize(&row.summary);
                let topic_tokens = tokenize(&row.topics);
                let mut score = 0.0f64;
                for qt in &query_tokens {
                    if content_tokens.contains(qt) { score += 1.0; }
                    if summary_tokens.contains(qt) { score += 1.5; }
                    if topic_tokens.contains(qt) { score += 2.0; }
                }
                if score > 0.0 { Some((score, row)) } else { None }
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, row)| row).collect())
    }

    /// Update last_accessed to now and increment access_count for a hot memory.
    pub fn hot_memory_touch(&self, id: i64) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "UPDATE hot_memories SET last_accessed = ?1, access_count = access_count + 1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// Return hot memories created more than 7 days ago (candidates for consolidation).
    pub fn hot_memories_consolidatable(&self) -> Result<Vec<HotMemoryRow>> {
        // Compute the cutoff timestamp: 7 days ago
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cutoff_secs = secs.saturating_sub(7 * 86400);
        let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(cutoff_secs);
        let cutoff = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z");

        let mut stmt = self.conn.prepare(
            "SELECT id, content, summary, entities, topics, importance,
                    created_at, last_accessed, access_count, expires_at
             FROM hot_memories
             WHERE created_at < ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![cutoff], |row| {
            Ok(HotMemoryRow {
                id: row.get(0)?,
                content: row.get(1)?,
                summary: row.get(2)?,
                entities: row.get(3)?,
                topics: row.get(4)?,
                importance: row.get(5)?,
                created_at: row.get(6)?,
                last_accessed: row.get(7)?,
                access_count: row.get(8)?,
                expires_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    /// Delete a hot memory by id. Returns true if the row existed and was deleted.
    pub fn hot_memory_delete(&self, id: i64) -> Result<bool> {
        let n = self.conn.execute("DELETE FROM hot_memories WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    // ─── Cold memory helpers ──────────────────────────────────────────────────

    /// Insert a cold memory. expires_at is set to 90 days from now.
    pub fn cold_memory_insert(&self, content: &str, topics: &str, importance: f64) -> Result<i64> {
        let now = now_utc();
        let expires_at = add_days_to_utc(&now, 90);
        self.conn.execute(
            "INSERT INTO cold_memories
             (content, topics, importance, created_at, reinforcement_count, last_reinforced, expires_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
            params![content, topics, importance, now, now, expires_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return up to `limit` cold memories, most recent first.
    pub fn cold_memories_list(&self, limit: usize) -> Result<Vec<ColdMemoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, topics, importance, created_at,
                    reinforcement_count, last_reinforced, expires_at
             FROM cold_memories
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ColdMemoryRow {
                id: row.get(0)?,
                content: row.get(1)?,
                topics: row.get(2)?,
                importance: row.get(3)?,
                created_at: row.get(4)?,
                reinforcement_count: row.get(5)?,
                last_reinforced: row.get(6)?,
                expires_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    /// Keyword search over cold memories (content + topics).
    pub fn cold_memory_search(&self, query: &str, limit: usize) -> Result<Vec<ColdMemoryRow>> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        let all = self.cold_memories_list(usize::MAX)?;
        let mut scored: Vec<(f64, ColdMemoryRow)> = all
            .into_iter()
            .filter_map(|row| {
                let content_tokens = tokenize(&row.content);
                let topic_tokens = tokenize(&row.topics);
                let mut score = 0.0f64;
                for qt in &query_tokens {
                    if content_tokens.contains(qt) { score += 1.0; }
                    if topic_tokens.contains(qt) { score += 2.0; }
                }
                if score > 0.0 { Some((score, row)) } else { None }
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, row)| row).collect())
    }

    /// Increment reinforcement_count, update last_reinforced, and extend expires_at by 90 days.
    pub fn cold_memory_reinforce(&self, id: i64) -> Result<()> {
        let now = now_utc();
        let new_expires = add_days_to_utc(&now, 90);
        self.conn.execute(
            "UPDATE cold_memories
             SET reinforcement_count = reinforcement_count + 1,
                 last_reinforced = ?1,
                 expires_at = ?2
             WHERE id = ?3",
            params![now, new_expires, id],
        )?;
        Ok(())
    }

    /// Delete a cold memory by id. Returns true if the row existed and was deleted.
    pub fn cold_memory_delete(&self, id: i64) -> Result<bool> {
        let n = self.conn.execute("DELETE FROM cold_memories WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    // ─── Contradiction helpers ────────────────────────────────────────────────

    /// Record a new contradiction between an existing cold memory and a new memory.
    pub fn contradiction_insert(&self, cold_memory_id: i64, new_memory: &str) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "INSERT INTO memory_contradictions (cold_memory_id, new_memory, created_at, resolved)
             VALUES (?1, ?2, ?3, 0)",
            params![cold_memory_id, new_memory, now],
        )?;
        Ok(())
    }

    /// Return all unresolved contradictions.
    pub fn contradictions_open(&self) -> Result<Vec<ContradictionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, cold_memory_id, new_memory, created_at, resolved
             FROM memory_contradictions
             WHERE resolved = 0
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let resolved_int: i64 = row.get(4)?;
            Ok(ContradictionRow {
                id: row.get(0)?,
                cold_memory_id: row.get(1)?,
                new_memory: row.get(2)?,
                created_at: row.get(3)?,
                resolved: resolved_int != 0,
            })
        })?;
        rows.collect()
    }

    // ─── Structured observation helpers ──────────────────────────────────────

    /// Valid categories for structured observations.
    /// If the supplied category is not in this list, "learned" is used.
    const VALID_CATEGORIES: &'static [&'static str] = &[
        "learned",
        "tried_and_failed",
        "blocked_by",
        "dependency_discovered",
        "pattern_noticed",
    ];

    /// Insert a structured observation and return its new row id.
    pub fn sobs_insert(
        &self,
        content: &str,
        category: &str,
        source_file: &str,
        goal_id: &str,
        session: &str,
        tags: &str,
    ) -> Result<i64> {
        let effective_category = if Self::VALID_CATEGORIES.contains(&category) {
            category
        } else {
            "learned"
        };
        let now = now_utc();
        self.conn.execute(
            "INSERT INTO structured_observations
             (content, category, source_file, goal_id, session, tags, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![content, effective_category, source_file, goal_id, session, tags, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Search structured observations by keyword match on content + tags.
    /// Returns up to `limit` results ordered by relevance then created_at DESC.
    /// Only rows with score > 0 are returned.
    pub fn sobs_search(&self, query: &str, limit: usize) -> Result<Vec<StructuredObservation>> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        let all = self.sobs_list(usize::MAX)?;
        let mut scored: Vec<(f64, StructuredObservation)> = all
            .into_iter()
            .filter_map(|row| {
                let content_tokens = tokenize(&row.content);
                let tag_tokens = tokenize(&row.tags);
                let mut score = 0.0f64;
                for qt in &query_tokens {
                    if content_tokens.contains(qt) {
                        score += 1.0;
                    }
                    if tag_tokens.contains(qt) {
                        score += 2.0;
                    }
                }
                if score > 0.0 { Some((score, row)) } else { None }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.1.created_at.cmp(&a.1.created_at))
        });
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, row)| row).collect())
    }

    /// List structured observations, most recent first, up to `limit`.
    pub fn sobs_list(&self, limit: usize) -> Result<Vec<StructuredObservation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, category, source_file, goal_id, session, tags, created_at
             FROM structured_observations
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(StructuredObservation {
                id: row.get(0)?,
                content: row.get(1)?,
                category: row.get(2)?,
                source_file: row.get(3)?,
                goal_id: row.get(4)?,
                session: row.get(5)?,
                tags: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    // ─── Embedding helpers ────────────────────────────────────────────────────

    /// Store an embedding for an observation identified by its unique key.
    /// Uses INSERT OR REPLACE to update if already exists.
    pub fn embedding_store(&self, obs_key: &str, vector: &[f32]) -> Result<()> {
        let now = now_utc();
        let blob = crate::embeddings::serialize_vec(vector);
        self.conn.execute(
            "INSERT OR REPLACE INTO embeddings (obs_key, vector, created_at)
             VALUES (?1, ?2, ?3)",
            params![obs_key, blob, now],
        )?;
        Ok(())
    }

    /// Return all (obs_key, vector) rows from the embeddings table.
    pub fn embeddings_list_by_key(&self) -> Result<Vec<(String, Vec<f32>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT obs_key, vector FROM embeddings")?;
        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((key, blob))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (key, blob) = row?;
            let vec = crate::embeddings::deserialize_vec(&blob);
            result.push((key, vec));
        }
        Ok(result)
    }

    /// Semantic search using Ollama embeddings. Returns top-`limit` observations
    /// ordered by cosine similarity to `query`.
    ///
    /// Gracefully returns an empty vec if Ollama is unavailable or no embeddings
    /// are stored yet — never panics.
    pub fn semantic_search_memory(&self, query: &str, limit: usize) -> Result<Vec<ObservationRow>> {
        if limit == 0 || query.is_empty() {
            return Ok(vec![]);
        }
        // Get query embedding — graceful fallback if Ollama is down
        let query_vec = match crate::embeddings::embed(query) {
            Ok(v) => v,
            Err(_) => return Ok(vec![]),
        };
        // Get all stored embeddings
        let all_embeddings = self.embeddings_list_by_key()?;
        if all_embeddings.is_empty() {
            return Ok(vec![]);
        }
        // Score each stored embedding against the query vector
        let mut scored: Vec<(String, f32)> = all_embeddings
            .into_iter()
            .map(|(key, vec)| {
                let sim = crate::embeddings::cosine_similarity(&query_vec, &vec);
                (key, sim)
            })
            .filter(|(_, sim)| *sim > 0.01)
            .collect();
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);

        // Fetch matching observations from DB
        let mut result = Vec::new();
        for (key, sim) in scored {
            let mut stmt = self.conn.prepare(
                "SELECT id, key, text, tags, created_at FROM observations WHERE key = ?1",
            )?;
            let rows: Vec<ObservationRow> = stmt
                .query_map(params![key], |row| {
                    Ok(ObservationRow {
                        id: row.get(0)?,
                        key: row.get(1)?,
                        text: row.get(2)?,
                        tags: row.get(3)?,
                        created_at: row.get(4)?,
                        score: 0.0,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            for mut row in rows {
                row.score = sim as f64;
                result.push(row);
            }
        }
        Ok(result)
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

/// Add `days` to a UTC timestamp string (YYYY-MM-DDTHH:MM:SSZ).
///
/// Parses the timestamp, adds days * 86400 seconds, re-formats.
/// Falls back to `now_utc()` if parsing fails.
fn add_days_to_utc(ts: &str, days: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Try to parse the timestamp or fall back to current time
    let base_secs = if ts.len() >= 19 {
        // Parse YYYY-MM-DDTHH:MM:SSZ
        let y: u64 = ts[0..4].parse().unwrap_or(0);
        let mo: u64 = ts[5..7].parse().unwrap_or(0);
        let d: u64 = ts[8..10].parse().unwrap_or(0);
        let h: u64 = ts[11..13].parse().unwrap_or(0);
        let mi: u64 = ts[14..16].parse().unwrap_or(0);
        let s: u64 = ts[17..19].parse().unwrap_or(0);
        if y > 0 && mo > 0 && d > 0 {
            ymd_hms_to_epoch(y, mo, d, h, mi, s)
        } else {
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        }
    } else {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
    };
    let target_secs = base_secs + days * 86400;
    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(target_secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert (year, month, day, hour, min, sec) to Unix timestamp (seconds).
fn ymd_hms_to_epoch(y: u64, mo: u64, d: u64, h: u64, mi: u64, s: u64) -> u64 {
    // Days from 1970-01-01 to year-01-01
    let days_to_year: u64 = (1970..y).map(|yr| if is_leap(yr) { 366 } else { 365 }).sum();
    let month_days: [u64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let days_to_month: u64 = month_days[..(mo as usize).saturating_sub(1)].iter().sum();
    let total_days = days_to_year + days_to_month + d.saturating_sub(1);
    total_days * 86400 + h * 3600 + mi * 60 + s
}

/// Tokenize a string for keyword search.
///
/// Lowercases, splits on whitespace and non-alphanumeric characters,
/// filters out stop words, deduplicates, and returns the result.
fn tokenize(s: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "the", "is", "it", "in", "on", "at", "to", "of",
        "for", "and", "or", "but", "not", "was", "has", "be",
    ];

    let lower = s.to_lowercase();
    let mut tokens: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .filter(|t| !STOP_WORDS.contains(t))
        .map(|t| t.to_string())
        .collect();

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    tokens.retain(|t| seen.insert(t.clone()));
    tokens
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

    // ── Observations ─────────────────────────────────────────────────────────

    #[test]
    fn test_observation_store_and_list() {
        let db = open_tmp();
        db.observation_store("obs:1", "rust borrow checker error", "rust,error").unwrap();
        db.observation_store("obs:2", "repl command dispatch", "repl,slash-command").unwrap();
        let rows = db.observations_list(10).unwrap();
        assert_eq!(rows.len(), 2, "should have 2 observations");
    }

    #[test]
    fn test_observation_store_upsert() {
        let db = open_tmp();
        db.observation_store("obs:dup", "original text", "tag1").unwrap();
        db.observation_store("obs:dup", "updated text", "tag1,tag2").unwrap();
        let rows = db.observations_list(10).unwrap();
        assert_eq!(rows.len(), 1, "upsert should result in only 1 row");
        assert_eq!(rows[0].text, "updated text", "text should be updated");
        assert_eq!(rows[0].tags, "tag1,tag2", "tags should be updated");
    }

    #[test]
    fn test_search_memory_finds_by_keyword() {
        let db = open_tmp();
        db.observation_store("obs:rust", "rust borrow checker error", "rust,error").unwrap();
        let results = db.search_memory("rust", 10).unwrap();
        assert!(!results.is_empty(), "search for 'rust' should find the observation");
        assert_eq!(results[0].key, "obs:rust");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_memory_tags_score_higher() {
        let db = open_tmp();
        // "repl" in tags → score 2.0
        db.observation_store("obs:tag", "something about commands", "repl,commands").unwrap();
        // "repl" only in text → score 1.0
        db.observation_store("obs:text", "the repl handles user input", "").unwrap();
        let results = db.search_memory("repl", 10).unwrap();
        assert_eq!(results.len(), 2, "both observations should match");
        assert_eq!(results[0].key, "obs:tag",
            "tag match should rank higher than text match");
        assert!(results[0].score > results[1].score,
            "tag score ({}) should beat text score ({})", results[0].score, results[1].score);
    }

    #[test]
    fn test_search_memory_no_results() {
        let db = open_tmp();
        db.observation_store("obs:a", "something about rust", "rust").unwrap();
        let results = db.search_memory("xyzzy nothing", 10).unwrap();
        assert!(results.is_empty(), "search for nonsense words should return empty");
    }

    #[test]
    fn test_search_memory_limit() {
        let db = open_tmp();
        for i in 0..5 {
            db.observation_store(
                &format!("obs:{i}"),
                &format!("rust error number {i}"),
                "rust,error",
            ).unwrap();
        }
        let results = db.search_memory("rust", 3).unwrap();
        assert_eq!(results.len(), 3, "limit should cap results at 3");
    }

    #[test]
    fn test_search_memory_filters_stop_words() {
        let db = open_tmp();
        db.observation_store("obs:sw", "database error occurred", "error,database").unwrap();
        // "the" is a stop word; "error" is meaningful
        let results = db.search_memory("the error", 10).unwrap();
        assert!(!results.is_empty(), "should match on 'error' even though 'the' is filtered");
        assert_eq!(results[0].key, "obs:sw");
    }

    // ── seed_from_journal ────────────────────────────────────────────────────

    #[test]
    fn test_seed_from_journal_parses_entries() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = AxonixDb::open(&db_path).unwrap();

        // Write a mock journal
        let journal = dir.path().join("JOURNAL.md");
        std::fs::write(
            &journal,
            "# Journal\n\n## Day 16, S7 — G-088: memory search\n\nBuilt TF-IDF search over SQLite observations table.\n\n## Day 16, S6 — G-087: archive fix\n\nFixed /archive-journal slash-command dispatch.\n",
        ).unwrap();

        let n = db.seed_from_journal(&journal).unwrap();
        assert_eq!(n, 2);

        let results = db.search_memory("memory search", 10).unwrap();
        assert!(!results.is_empty(), "should find memory search entry");
    }

    #[test]
    fn test_seed_from_journal_upserts_idempotent() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = AxonixDb::open(&db_path).unwrap();

        let journal = dir.path().join("JOURNAL.md");
        std::fs::write(
            &journal,
            "# Journal\n\n## Day 1, S1 — test entry\n\nSome content.\n",
        ).unwrap();

        let n1 = db.seed_from_journal(&journal).unwrap();
        let n2 = db.seed_from_journal(&journal).unwrap();
        assert_eq!(n1, 1);
        assert_eq!(n2, 1); // idempotent, still upserts same entry

        // Should still only have 1 observation
        let list = db.observations_list(10).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_seed_from_journal_missing_file() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = AxonixDb::open(&db_path).unwrap();

        let missing = dir.path().join("NONEXISTENT.md");
        let result = db.seed_from_journal(&missing);
        assert!(result.is_err() || result.unwrap() == 0);
    }

    // ── Hot memories ──────────────────────────────────────────────────────────

    #[test]
    fn test_hot_memory_insert_and_list() {
        let db = open_tmp();
        let id = db.hot_memory_insert(
            "Operator prefers concise summaries",
            "concise summaries",
            "[]",
            "[\"style\",\"preferences\"]",
            0.9,
        ).unwrap();
        assert!(id > 0);

        let rows = db.hot_memories_list(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "Operator prefers concise summaries");
        assert_eq!(rows[0].importance, 0.9);
        assert_eq!(rows[0].access_count, 0);
    }

    #[test]
    fn test_hot_memory_search_finds_by_keyword() {
        let db = open_tmp();
        db.hot_memory_insert("rust borrow checker patterns", "rust borrow", "[]", "[\"rust\"]", 0.7).unwrap();
        db.hot_memory_insert("SQLite migration scripts", "sqlite migration", "[]", "[\"database\"]", 0.6).unwrap();

        let results = db.hot_memory_search("rust", 10).unwrap();
        assert!(!results.is_empty(), "should find rust memory");
        assert_eq!(results[0].summary, "rust borrow");
    }

    #[test]
    fn test_hot_memory_touch_increments_access_count() {
        let db = open_tmp();
        let id = db.hot_memory_insert("some fact", "fact", "[]", "[]", 0.6).unwrap();
        db.hot_memory_touch(id).unwrap();
        db.hot_memory_touch(id).unwrap();

        let rows = db.hot_memories_list(10).unwrap();
        assert_eq!(rows[0].access_count, 2, "access_count should be 2 after two touches");
    }

    #[test]
    fn test_hot_memory_delete() {
        let db = open_tmp();
        let id = db.hot_memory_insert("to be deleted", "delete me", "[]", "[]", 0.5).unwrap();
        assert!(db.hot_memory_delete(id).unwrap());
        let rows = db.hot_memories_list(10).unwrap();
        assert!(rows.is_empty(), "hot memory should be deleted");
    }

    #[test]
    fn test_hot_memory_search_no_results() {
        let db = open_tmp();
        db.hot_memory_insert("rust memory", "rust", "[]", "[\"rust\"]", 0.7).unwrap();
        let results = db.hot_memory_search("xyzzy_nothing_here", 10).unwrap();
        assert!(results.is_empty());
    }

    // ── Cold memories ─────────────────────────────────────────────────────────

    #[test]
    fn test_cold_memory_insert_and_list() {
        let db = open_tmp();
        let id = db.cold_memory_insert(
            "Axonix consistently uses async patterns across all modules",
            "[\"architecture\",\"async\"]",
            0.8,
        ).unwrap();
        assert!(id > 0);

        let rows = db.cold_memories_list(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "Axonix consistently uses async patterns across all modules");
        assert_eq!(rows[0].reinforcement_count, 0);
    }

    #[test]
    fn test_cold_memory_search_finds_by_keyword() {
        let db = open_tmp();
        db.cold_memory_insert("async patterns dominate the codebase", "[\"async\",\"architecture\"]", 0.8).unwrap();
        db.cold_memory_insert("SQLite is used for persistent storage", "[\"database\",\"sqlite\"]", 0.7).unwrap();

        let results = db.cold_memory_search("async", 10).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].content.contains("async"));
    }

    #[test]
    fn test_cold_memory_reinforce() {
        let db = open_tmp();
        let id = db.cold_memory_insert("a synthesized pattern", "[\"pattern\"]", 0.7).unwrap();
        db.cold_memory_reinforce(id).unwrap();

        let rows = db.cold_memories_list(10).unwrap();
        assert_eq!(rows[0].reinforcement_count, 1, "reinforcement_count should be 1");
    }

    #[test]
    fn test_cold_memory_delete() {
        let db = open_tmp();
        let id = db.cold_memory_insert("temporary cold memory", "[]", 0.5).unwrap();
        assert!(db.cold_memory_delete(id).unwrap());
        let rows = db.cold_memories_list(10).unwrap();
        assert!(rows.is_empty());
    }

    // ── Contradictions ────────────────────────────────────────────────────────

    #[test]
    fn test_contradiction_insert_and_open() {
        let db = open_tmp();
        let cold_id = db.cold_memory_insert("old belief about X", "[\"topic\"]", 0.7).unwrap();
        db.contradiction_insert(cold_id, "new conflicting belief about X").unwrap();

        let open = db.contradictions_open().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].cold_memory_id, cold_id);
        assert_eq!(open[0].new_memory, "new conflicting belief about X");
        assert!(!open[0].resolved);
    }

    #[test]
    fn test_contradictions_open_empty_when_none() {
        let db = open_tmp();
        let open = db.contradictions_open().unwrap();
        assert!(open.is_empty());
    }

    // ── add_days_to_utc ───────────────────────────────────────────────────────

    #[test]
    fn test_add_days_to_utc_basic() {
        let ts = "2026-01-01T00:00:00Z";
        let result = add_days_to_utc(ts, 30);
        assert!(result.starts_with("2026-01-31"), "30 days from Jan 1 should be Jan 31: {result}");
    }

    #[test]
    fn test_add_days_crosses_month() {
        let ts = "2026-01-20T12:00:00Z";
        let result = add_days_to_utc(ts, 30);
        // Jan 20 + 30 days = Feb 19
        assert!(result.starts_with("2026-02-19"), "should be Feb 19: {result}");
    }

    // ── Structured observations ───────────────────────────────────────────────

    #[test]
    fn test_sobs_insert_returns_id() {
        let db = open_tmp();
        let id = db.sobs_insert("learned something new", "learned", "", "", "", "").unwrap();
        assert!(id > 0, "insert should return a positive row id");
    }

    #[test]
    fn test_sobs_insert_invalid_category_defaults_to_learned() {
        let db = open_tmp();
        let id = db.sobs_insert("some content", "garbage", "", "", "", "").unwrap();
        assert!(id > 0);
        let rows = db.sobs_list(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category, "learned", "invalid category should default to 'learned'");
    }

    #[test]
    fn test_sobs_list_returns_most_recent_first() {
        let db = open_tmp();
        // Insert 3 observations
        let id1 = db.sobs_insert("first observation", "learned", "", "", "", "").unwrap();
        let id2 = db.sobs_insert("second observation", "learned", "", "", "", "").unwrap();
        let id3 = db.sobs_insert("third observation", "learned", "", "", "", "").unwrap();
        assert!(id3 > id2 && id2 > id1);

        // List only 2 — should return the two most recent (ORDER BY created_at DESC, id DESC)
        let rows = db.sobs_list(2).unwrap();
        assert_eq!(rows.len(), 2, "limit=2 should return at most 2 rows");
        // When timestamps are equal, ordering falls back to id DESC, so id3 and id2 come first
        assert_eq!(rows[0].id, id3, "most recent by id should be first");
        assert_eq!(rows[1].id, id2, "second most recent by id should be second");
    }

    #[test]
    fn test_sobs_search_finds_by_content_keyword() {
        let db = open_tmp();
        db.sobs_insert("rusqlite connection pool setup", "learned", "", "", "", "").unwrap();
        let results = db.sobs_search("connection", 10).unwrap();
        assert!(!results.is_empty(), "search for 'connection' should find the observation");
        assert!(results[0].content.contains("connection"));
    }

    #[test]
    fn test_sobs_search_finds_by_tag() {
        let db = open_tmp();
        db.sobs_insert("setting up the bot", "learned", "", "", "", "telegram,bot").unwrap();
        let results = db.sobs_search("telegram", 10).unwrap();
        assert!(!results.is_empty(), "search for 'telegram' should match via tags");
        assert!(results[0].tags.contains("telegram"));
    }

    #[test]
    fn test_sobs_search_no_match_returns_empty() {
        let db = open_tmp();
        db.sobs_insert("completely unrelated content", "learned", "", "", "", "unrelated").unwrap();
        let results = db.sobs_search("zzz_nomatch", 10).unwrap();
        assert!(results.is_empty(), "search for 'zzz_nomatch' should return empty");
    }

    #[test]
    fn test_sobs_search_respects_limit() {
        let db = open_tmp();
        for i in 0..5 {
            db.sobs_insert(&format!("memory about connection {i}"), "learned", "", "", "", "").unwrap();
        }
        let results = db.sobs_search("connection", 2).unwrap();
        assert_eq!(results.len(), 2, "limit=2 should cap results at 2");
    }

    #[test]
    fn test_sobs_category_field_preserved() {
        let db = open_tmp();
        db.sobs_insert("hit a wall on this feature", "blocked_by", "", "", "", "").unwrap();
        let rows = db.sobs_list(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category, "blocked_by", "category should be preserved as stored");
    }

    #[test]
    fn test_sobs_goal_id_and_session_preserved() {
        let db = open_tmp();
        db.sobs_insert("discovered dep", "dependency_discovered", "", "G-110", "Day 19 S1", "").unwrap();
        let rows = db.sobs_list(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].goal_id, "G-110", "goal_id should be preserved");
        assert_eq!(rows[0].session, "Day 19 S1", "session should be preserved");
    }

    #[test]
    fn test_sobs_search_ranks_tag_match_higher() {
        let db = open_tmp();
        // content-only match: score 1.0
        db.sobs_insert("we use sqlite heavily", "learned", "", "", "", "database").unwrap();
        // tag match: score 2.0 (tag "sqlite" matches)
        db.sobs_insert("persistence layer redesign", "learned", "", "", "", "sqlite,storage").unwrap();
        let results = db.sobs_search("sqlite", 10).unwrap();
        assert_eq!(results.len(), 2, "both observations should match");
        // The tag-match result should come first (higher score)
        assert_eq!(
            results[0].tags, "sqlite,storage",
            "tag-match observation should rank first"
        );
    }

    // ── Embeddings ────────────────────────────────────────────────────────────

    #[test]
    fn test_embedding_store_and_retrieve() {
        let db = open_tmp();
        let vec = vec![1.0f32, 2.0, 3.0];
        db.embedding_store("obs:test", &vec).unwrap();
        let list = db.embeddings_list_by_key().unwrap();
        assert_eq!(list.len(), 1, "should have 1 embedding");
        assert_eq!(list[0].0, "obs:test");
        assert_eq!(list[0].1.len(), 3);
        assert!((list[0].1[0] - 1.0).abs() < 1e-6);
        assert!((list[0].1[1] - 2.0).abs() < 1e-6);
        assert!((list[0].1[2] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_embedding_store_upsert() {
        let db = open_tmp();
        db.embedding_store("obs:dup", &[1.0f32, 0.0]).unwrap();
        db.embedding_store("obs:dup", &[0.0f32, 1.0]).unwrap();
        let list = db.embeddings_list_by_key().unwrap();
        assert_eq!(list.len(), 1, "upsert should keep only 1 embedding row");
        assert!((list[0].1[0] - 0.0).abs() < 1e-6);
        assert!((list[0].1[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_semantic_search_memory_no_ollama_returns_empty() {
        // With no Ollama available, should return empty vec (graceful fallback)
        let db = open_tmp();
        // Point to a port that will refuse connections
        std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1");
        let result = db.semantic_search_memory("test query", 5);
        std::env::remove_var("OLLAMA_URL");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_semantic_search_memory_empty_query_returns_empty() {
        let db = open_tmp();
        let result = db.semantic_search_memory("", 5).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_semantic_search_memory_zero_limit_returns_empty() {
        let db = open_tmp();
        let result = db.semantic_search_memory("query", 0).unwrap();
        assert!(result.is_empty());
    }
}
