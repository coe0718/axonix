//! File I/O for conversation memory: load from JSON, save to JSON, path resolution.

use std::path::PathBuf;
use crate::conversation_memory::types::ConversationTurn;

/// Return the default path for conversation memory: `.axonix/conversation_memory.json`.
pub fn default_conversation_memory_path() -> PathBuf {
    PathBuf::from(".axonix/conversation_memory.json")
}

/// Load conversation turns from a JSON file.
///
/// Returns an empty vec (not an error) if the file doesn't exist or can't be parsed.
pub(super) fn load_turns(path: &PathBuf) -> Vec<ConversationTurn> {
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<Vec<ConversationTurn>>(&content) {
            Ok(turns) => turns,
            Err(e) => {
                eprintln!(
                    "  ⚠ conversation_memory: failed to parse {:?}: {e}",
                    path
                );
                Vec::new()
            }
        },
        Err(e) => {
            eprintln!(
                "  ⚠ conversation_memory: failed to read {:?}: {e}",
                path
            );
            Vec::new()
        }
    }
}

/// Save conversation turns to a JSON file as pretty-printed JSON.
///
/// Creates parent directories if they don't exist.
pub(super) fn save_turns(path: &PathBuf, turns: &[ConversationTurn]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "conversation_memory: failed to create dir {:?}: {e}",
                    parent
                )
            })?;
        }
    }
    let json = serde_json::to_string_pretty(turns)
        .map_err(|e| format!("conversation_memory: serialization error: {e}"))?;
    std::fs::write(path, json)
        .map_err(|e| format!("conversation_memory: failed to write {:?}: {e}", path))?;
    Ok(())
}
