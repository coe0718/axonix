//! Persistent conversation memory for the always-on listener.
//!
//! Stores conversation turns (user + assistant) to `.axonix/conversation_memory.json`.
//! Read at session start to give context about what was discussed since last session.
//!
//! # Design
//!
//! - Separate from `memory.rs` (key-value facts) — this is turn-by-turn conversation history.
//! - Rolling window: keeps the most recent `max_turns` turns to bound file size.
//! - Flat JSON array: human-readable, diffable, zero external dependencies.
//!
//! # File location
//!
//! Default: `.axonix/conversation_memory.json` in the current working directory.
//!
//! # Example
//!
//! ```
//! use axonix::conversation_memory::ConversationMemory;
//!
//! let dir = tempfile::tempdir().unwrap();
//! let path = dir.path().join("conv.json");
//! let mut mem = ConversationMemory::new(&path);
//! mem.push("user", "what's the disk usage?", "telegram");
//! mem.push("assistant", "Disk is at 45%.", "telegram");
//! assert_eq!(mem.turns.len(), 2);
//! ```

mod storage;
mod trim;
mod types;
#[cfg(test)]
mod tests;

pub use storage::default_conversation_memory_path;
pub use types::ConversationTurn;

use std::path::PathBuf;
use storage::{load_turns, save_turns};
use trim::current_timestamp;

/// Persistent turn-by-turn conversation log.
///
/// Use `ConversationMemory::new(path)` for a fresh in-memory store, or
/// `ConversationMemory::load(path)` to load an existing file (or start empty).
pub struct ConversationMemory {
    /// Path to the backing JSON file.
    pub path: PathBuf,
    /// All stored turns (in order, oldest first).
    pub turns: Vec<ConversationTurn>,
    /// Maximum number of turns to keep (rolling window).  Default: 100.
    pub max_turns: usize,
}

impl ConversationMemory {
    /// Create a new, empty `ConversationMemory` at the given path.
    ///
    /// Does NOT load from disk — call `load()` for that.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            turns: Vec::new(),
            max_turns: 100,
        }
    }

    /// Create a new, empty `ConversationMemory` at the default path
    /// (`.axonix/conversation_memory.json`).
    ///
    /// Does NOT load from disk.  Use `load(default_conversation_memory_path())` to load.
    pub fn default_path() -> Self {
        Self::new(default_conversation_memory_path())
    }

    /// Load conversation memory from the given path.
    ///
    /// Returns an empty store (not an error) if the file doesn't exist or can't be parsed.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let turns = load_turns(&path);
        Self {
            path,
            turns,
            max_turns: 100,
        }
    }

    /// Add a turn to the conversation log.
    ///
    /// Automatically trims to `max_turns` if the log exceeds the limit,
    /// keeping the most recent turns.
    pub fn push(&mut self, role: &str, text: &str, channel: &str) {
        let timestamp = current_timestamp();
        self.turns.push(ConversationTurn {
            timestamp,
            role: role.to_string(),
            text: text.to_string(),
            channel: channel.to_string(),
        });
        // Trim to max_turns: keep the most recent
        if self.turns.len() > self.max_turns {
            let excess = self.turns.len() - self.max_turns;
            self.turns.drain(0..excess);
        }
    }

    /// Save the conversation memory to disk as pretty-printed JSON.
    ///
    /// Creates parent directories if they don't exist.
    pub fn save(&self) -> Result<(), String> {
        save_turns(&self.path, &self.turns)
    }

    /// Return the last `n` turns (or all turns if fewer than `n` exist).
    pub fn recent(&self, n: usize) -> &[ConversationTurn] {
        if n >= self.turns.len() {
            &self.turns
        } else {
            &self.turns[self.turns.len() - n..]
        }
    }

    /// Format the last `n` turns as a block suitable for injection into a system prompt.
    ///
    /// Returns an empty string if there are no turns.
    ///
    /// Example output:
    /// ```text
    /// ## Recent Conversations (last 2 turns)
    /// [2026-03-22 14:30] user: what's the disk usage?
    /// [2026-03-22 14:30] assistant: Disk is at 45% (120GB / 250GB).
    /// ```
    pub fn format_for_context(&self, n: usize) -> String {
        let recent = self.recent(n);
        if recent.is_empty() {
            return String::new();
        }
        let mut lines = vec![format!("## Recent Conversations (last {} turns)", recent.len())];
        for turn in recent {
            // Show only date + time (first 16 chars of ISO 8601: "2026-03-22T14:30")
            // and replace the 'T' separator with a space for readability.
            let display_ts = if turn.timestamp.len() >= 16 {
                turn.timestamp[..16].replace('T', " ")
            } else {
                turn.timestamp.clone()
            };
            lines.push(format!("[{}] {}: {}", display_ts, turn.role, turn.text));
        }
        lines.join("\n")
    }
}


