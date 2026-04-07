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

use rusqlite::{Connection, Result};
use std::path::Path;

mod helpers;
mod schema;
mod kv;
mod sessions;
mod goals;
mod predictions;
mod observations;
mod hot_memory;
mod cold_memory;
mod embeddings;

pub mod types;
pub use types::*;

#[cfg(test)]
mod tests;

// ─── Main struct ─────────────────────────────────────────────────────────────

/// SQLite-backed structured memory store.
pub struct AxonixDb {
    pub(crate) conn: Connection,
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
}
