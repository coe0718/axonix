use rusqlite::Result;
use super::AxonixDb;

impl AxonixDb {
    /// Run schema migrations (idempotent — uses `IF NOT EXISTS`).
    pub(super) fn migrate(&self) -> Result<()> {
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
}
