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

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single conversation turn (one message from one party).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversationTurn {
    /// ISO 8601 timestamp (e.g. "2026-03-22T14:30:00Z").
    pub timestamp: String,
    /// Who spoke: `"user"` or `"assistant"`.
    pub role: String,
    /// The message text.
    pub text: String,
    /// Channel the message came from: `"telegram"`, `"repl"`, `"prompt"`.
    pub channel: String,
}

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
        let mut mem = Self::new(&path);
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<Vec<ConversationTurn>>(&content) {
                    Ok(turns) => {
                        mem.turns = turns;
                    }
                    Err(e) => {
                        eprintln!(
                            "  ⚠ conversation_memory: failed to parse {:?}: {e}",
                            path
                        );
                    }
                },
                Err(e) => {
                    eprintln!(
                        "  ⚠ conversation_memory: failed to read {:?}: {e}",
                        path
                    );
                }
            }
        }
        mem
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
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "conversation_memory: failed to create dir {:?}: {e}",
                        parent
                    )
                })?;
            }
        }
        let json = serde_json::to_string_pretty(&self.turns)
            .map_err(|e| format!("conversation_memory: serialization error: {e}"))?;
        std::fs::write(&self.path, json)
            .map_err(|e| format!("conversation_memory: failed to write {:?}: {e}", self.path))?;
        Ok(())
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

/// Return the default path for conversation memory: `.axonix/conversation_memory.json`.
pub fn default_conversation_memory_path() -> PathBuf {
    PathBuf::from(".axonix/conversation_memory.json")
}

/// Return the current UTC time as an ISO 8601 string (seconds precision).
fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    unix_to_iso8601(secs)
}

/// Convert a Unix timestamp (seconds) to an ISO 8601 UTC string.
///
/// Example: `1773619200` → `"2026-03-16T00:00:00Z"`
fn unix_to_iso8601(secs: u64) -> String {
    let (year, month, day) = unix_to_ymd(secs);
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    )
}

/// Convert Unix timestamp (seconds) to `(year, month, day)` UTC.
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

    fn tmp_mem(dir: &tempfile::TempDir) -> ConversationMemory {
        let path = dir.path().join("conv.json");
        ConversationMemory::new(path)
    }

    // ── Construction ─────────────────────────────────────────────────────────

    #[test]
    fn test_new_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let mem = tmp_mem(&dir);
        assert!(mem.turns.is_empty(), "new ConversationMemory should be empty");
        assert_eq!(mem.max_turns, 100);
    }

    // ── push ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_push_adds_turns() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = tmp_mem(&dir);
        mem.push("user", "hello", "telegram");
        mem.push("assistant", "hi there", "telegram");
        assert_eq!(mem.turns.len(), 2);
        assert_eq!(mem.turns[0].role, "user");
        assert_eq!(mem.turns[0].text, "hello");
        assert_eq!(mem.turns[1].role, "assistant");
        assert_eq!(mem.turns[1].text, "hi there");
    }

    #[test]
    fn test_push_sets_channel() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = tmp_mem(&dir);
        mem.push("user", "test", "repl");
        assert_eq!(mem.turns[0].channel, "repl");
    }

    #[test]
    fn test_push_max_turns_trims_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = tmp_mem(&dir);
        mem.max_turns = 3;

        mem.push("user", "msg1", "telegram");
        mem.push("user", "msg2", "telegram");
        mem.push("user", "msg3", "telegram");
        // Now at capacity
        assert_eq!(mem.turns.len(), 3);
        // Push a 4th — should drop msg1
        mem.push("user", "msg4", "telegram");
        assert_eq!(mem.turns.len(), 3, "should stay at max_turns");
        assert_eq!(mem.turns[0].text, "msg2", "oldest should be dropped");
        assert_eq!(mem.turns[2].text, "msg4", "newest should be last");
    }

    // ── save / load ───────────────────────────────────────────────────────────

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conv.json");

        let mut mem = ConversationMemory::new(&path);
        mem.push("user", "what's the disk usage?", "telegram");
        mem.push("assistant", "Disk is at 45%.", "telegram");
        mem.save().expect("save should succeed");

        let loaded = ConversationMemory::load(&path);
        assert_eq!(loaded.turns.len(), 2);
        assert_eq!(loaded.turns[0].role, "user");
        assert_eq!(loaded.turns[0].text, "what's the disk usage?");
        assert_eq!(loaded.turns[1].role, "assistant");
        assert_eq!(loaded.turns[1].channel, "telegram");
    }

    #[test]
    fn test_load_nonexistent_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        let mem = ConversationMemory::load(&path);
        assert!(mem.turns.is_empty(), "load of missing file should return empty");
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("conv.json");
        let mut mem = ConversationMemory::new(&path);
        mem.push("user", "hello", "telegram");
        mem.save().expect("save should create parent dirs");
        assert!(path.exists(), "file should exist after save");
    }

    // ── recent() ─────────────────────────────────────────────────────────────

    #[test]
    fn test_recent_returns_last_n() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = tmp_mem(&dir);
        for i in 0..5 {
            mem.push("user", &format!("msg{}", i), "telegram");
        }
        let recent = mem.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text, "msg3");
        assert_eq!(recent[1].text, "msg4");
    }

    #[test]
    fn test_recent_more_than_available_returns_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = tmp_mem(&dir);
        mem.push("user", "only turn", "telegram");
        let recent = mem.recent(100);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_recent_empty_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let mem = tmp_mem(&dir);
        assert!(mem.recent(5).is_empty());
    }

    // ── format_for_context() ─────────────────────────────────────────────────

    #[test]
    fn test_format_for_context_non_empty() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = tmp_mem(&dir);
        mem.push("user", "what's the disk usage?", "telegram");
        mem.push("assistant", "Disk is at 45% (120GB / 250GB).", "telegram");
        let ctx = mem.format_for_context(10);
        assert!(!ctx.is_empty(), "should produce non-empty context");
        assert!(ctx.contains("Recent Conversations"), "should have header");
        assert!(ctx.contains("user"), "should include role");
        assert!(ctx.contains("disk usage"), "should include message content");
        assert!(ctx.contains("assistant"), "should include assistant role");
    }

    #[test]
    fn test_format_for_context_empty_returns_empty_string() {
        let dir = tempfile::tempdir().unwrap();
        let mem = tmp_mem(&dir);
        let ctx = mem.format_for_context(10);
        assert!(ctx.is_empty(), "empty memory should produce empty string");
    }

    #[test]
    fn test_format_for_context_shows_turn_count() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = tmp_mem(&dir);
        mem.push("user", "hello", "telegram");
        mem.push("assistant", "hi", "telegram");
        let ctx = mem.format_for_context(2);
        assert!(ctx.contains("2 turns"), "should show turn count in header");
    }

    // ── ConversationTurn serialization ────────────────────────────────────────

    #[test]
    fn test_turn_serializes_deserializes() {
        let turn = ConversationTurn {
            timestamp: "2026-03-22T14:30:00Z".to_string(),
            role: "user".to_string(),
            text: "hello world".to_string(),
            channel: "telegram".to_string(),
        };
        let json = serde_json::to_string(&turn).expect("serialization failed");
        let decoded: ConversationTurn = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(decoded, turn);
        assert!(json.contains("2026-03-22T14:30:00Z"));
        assert!(json.contains("telegram"));
    }

    // ── timestamp generation ──────────────────────────────────────────────────

    #[test]
    fn test_push_records_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = tmp_mem(&dir);
        mem.push("user", "test", "telegram");
        let ts = &mem.turns[0].timestamp;
        assert_eq!(ts.len(), 20, "timestamp should be 20 chars: {ts}"); // "2026-03-22T14:30:00Z"
        assert!(ts.ends_with('Z'), "timestamp should end with Z: {ts}");
        assert!(ts.contains('T'), "timestamp should contain T separator: {ts}");
    }

    #[test]
    fn test_unix_to_iso8601_known_date() {
        // 2026-03-16T00:00:00Z = 1773619200
        let result = unix_to_iso8601(1773619200);
        assert_eq!(result, "2026-03-16T00:00:00Z");
    }

    #[test]
    fn test_unix_to_iso8601_epoch() {
        let result = unix_to_iso8601(0);
        assert_eq!(result, "1970-01-01T00:00:00Z");
    }

    // ── default_path ─────────────────────────────────────────────────────────

    #[test]
    fn test_default_path_points_to_axonix_dir() {
        let mem = ConversationMemory::default_path();
        let path_str = mem.path.to_string_lossy();
        assert!(
            path_str.contains(".axonix"),
            "default path should be under .axonix: {path_str}"
        );
        assert!(
            path_str.contains("conversation_memory"),
            "default path should contain conversation_memory: {path_str}"
        );
    }
}
