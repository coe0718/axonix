//! Session startup memory loader (Layer 5 — Session startup).
//!
//! Loads relevant memories at session start and formats them for
//! injection into the system prompt. Hard cap of 12 memories.

use super::search::memory_search;
use std::path::Path;

/// Maximum number of memories to inject per session.
pub const SESSION_MEMORY_CAP: usize = 12;

/// Load the top-N memories relevant to `goal_title` and format them
/// as a markdown block suitable for system prompt injection.
///
/// Returns `None` if no relevant memories are found.
pub fn load_session_memories(db_path: &Path, goal_title: &str) -> Option<String> {
    if goal_title.trim().is_empty() {
        return None;
    }

    let results = memory_search(db_path, goal_title, SESSION_MEMORY_CAP);
    if results.is_empty() {
        return None;
    }

    let mut lines = vec!["## Memory Context".to_string()];
    lines.push(format!("Top {} relevant memories for current work:", results.len()));
    for r in &results {
        let tier = r.source.label();
        lines.push(format!("- [{}] {}", tier, r.content));
    }
    Some(lines.join("\n"))
}

/// Format open contradictions (conflicting memories) for system prompt injection.
///
/// Returns `None` if there are no unresolved contradictions.
pub fn load_contradictions(db_path: &Path) -> Option<String> {
    let db = crate::db::AxonixDb::open(db_path).ok()?;
    let rows = db.contradictions_open().ok()?;
    if rows.is_empty() {
        return None;
    }
    let mut lines = vec!["## Conflicting Memories (resolve this session)".to_string()];
    for row in rows {
        lines.push(format!("- Contradiction #{}: {}", row.id, row.new_memory));
    }
    Some(lines.join("\n"))
}
