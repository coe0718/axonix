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
pub mod store;
pub mod types;

mod tests;

pub use store::{default_memory_path, MemoryStore};
pub use types::MemoryEntry;
