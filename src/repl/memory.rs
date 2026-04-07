//! `/memory` command handler extracted from `handle_command`.

use crate::render::truncate;
use super::types::{CommandResult, ReplState};

/// Handle the `/memory` command and all its subcommands.
///
/// `arg` is the text after `/memory ` (or `""` when bare `/memory` is typed).
pub fn handle_memory(arg: &str, state: &mut ReplState) -> CommandResult {
    if arg.is_empty() || arg == "list" {
        if state.memory.is_empty() {
            CommandResult::Handled(vec![
                "  Memory: (empty)".to_string(),
                "  Use /memory set <key> <value> to store a fact.".to_string(),
                String::new(),
            ])
        } else {
            let mut lines = vec![
                format!("  Memory ({} entries):", state.memory.len()),
            ];
            for (key, entry) in state.memory.all() {
                let note_str = entry.note.as_deref()
                    .map(|n| format!("  — {}", truncate(n, 50)))
                    .unwrap_or_default();
                let date_str = entry.updated.as_deref()
                    .map(|d| format!(" [{d}]"))
                    .unwrap_or_default();
                lines.push(format!("    {:<30} = {}{note_str}{date_str}",
                    key,
                    truncate(&entry.value, 40)
                ));
            }
            lines.push(String::new());
            CommandResult::Handled(lines)
        }
    } else if let Some(rest) = arg.strip_prefix("get ") {
        let key = rest.trim();
        if key.is_empty() {
            CommandResult::Handled(vec![
                "  Usage: /memory get <key>".to_string(),
                String::new(),
            ])
        } else {
            match state.memory.get_entry(key) {
                None => CommandResult::Handled(vec![
                    format!("  Memory: '{key}' not set"),
                    String::new(),
                ]),
                Some(entry) => {
                    let mut lines = vec![
                        format!("  {key} = {}", entry.value),
                    ];
                    if let Some(note) = &entry.note {
                        lines.push(format!("  note: {note}"));
                    }
                    if let Some(updated) = &entry.updated {
                        lines.push(format!("  updated: {updated}"));
                    }
                    lines.push(String::new());
                    CommandResult::Handled(lines)
                }
            }
        }
    } else if let Some(rest) = arg.strip_prefix("set ") {
        let rest = rest.trim();
        // Split into key and value: "key value with spaces"
        let mut parts = rest.splitn(2, ' ');
        let key = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();
        if key.is_empty() || value.is_empty() {
            CommandResult::Handled(vec![
                "  Usage: /memory set <key> <value>".to_string(),
                "  Example: /memory set nuc.ip 192.168.1.10".to_string(),
                String::new(),
            ])
        } else {
            state.memory.set(key, value, None);
            let save_result = state.memory.save()
                .map(|_| format!("  ✓ memory: {key} = {value}"))
                .unwrap_or_else(|e| format!("  ⚠ memory saved in-session but failed to write: {e}"));
            CommandResult::Handled(vec![save_result, String::new()])
        }
    } else if let Some(rest) = arg.strip_prefix("note ") {
        let rest = rest.trim();
        let mut parts = rest.splitn(2, ' ');
        let key = parts.next().unwrap_or("").trim();
        let note = parts.next().unwrap_or("").trim();
        if key.is_empty() || note.is_empty() {
            CommandResult::Handled(vec![
                "  Usage: /memory note <key> <note text>".to_string(),
                "  Adds or updates the note on an existing key.".to_string(),
                String::new(),
            ])
        } else {
            match state.memory.get(key).map(|s| s.to_string()) {
                None => CommandResult::Handled(vec![
                    format!("  Memory: '{key}' not set — use /memory set first"),
                    String::new(),
                ]),
                Some(existing_value) => {
                    state.memory.set(key, &existing_value, Some(note));
                    let save_result = state.memory.save()
                        .map(|_| format!("  ✓ note added to '{key}'"))
                        .unwrap_or_else(|e| format!("  ⚠ note saved in-session but failed to write: {e}"));
                    CommandResult::Handled(vec![save_result, String::new()])
                }
            }
        }
    } else if let Some(rest) = arg.strip_prefix("del ") {
        let key = rest.trim();
        if key.is_empty() {
            CommandResult::Handled(vec![
                "  Usage: /memory del <key>".to_string(),
                String::new(),
            ])
        } else {
            if state.memory.del(key) {
                let save_result = state.memory.save()
                    .map(|_| format!("  ✓ memory: '{key}' deleted"))
                    .unwrap_or_else(|e| format!("  ⚠ deleted in-session but failed to write: {e}"));
                CommandResult::Handled(vec![save_result, String::new()])
            } else {
                CommandResult::Handled(vec![
                    format!("  Memory: '{key}' not set"),
                    String::new(),
                ])
            }
        }
    } else {
        CommandResult::Handled(vec![
            "  Usage:".to_string(),
            "    /memory list              Show all stored facts".to_string(),
            "    /memory get <key>         Get value of a key".to_string(),
            "    /memory set <key> <value> Store a key-value pair".to_string(),
            "    /memory note <key> <text> Add a note to an existing key".to_string(),
            "    /memory del <key>         Delete a key".to_string(),
            String::new(),
            "  Key naming convention: category.attribute".to_string(),
            "  Example: nuc.ip, twitter.status, operator.tz".to_string(),
            String::new(),
        ])
    }
}
