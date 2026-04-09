//! Miscellaneous REPL command handlers extracted from `handle_command`.
//!
//! Covers: `/failures`, `/summary`, `/recap`, `/archive-journal`.

use super::types::CommandResult;

/// Handle the `/failures` command: show logged failure patterns.
pub fn handle_failures() -> CommandResult {
    use crate::failure_patterns::FailurePatternStore;
    let mut store = FailurePatternStore::default_path();
    let summary = store.failure_summary();
    let mut lines: Vec<String> = summary.lines().map(|l| format!("  {l}")).collect();
    // G-118: check for threshold-3 breaches and surface them inline.
    let newly_breached = store.types_newly_at_threshold(3);
    if !newly_breached.is_empty() {
        lines.push(String::new());
        lines.push("  ⚠️  Threshold alert (G-118): the following failure types have 3+ occurrences:".to_string());
        for label in &newly_breached {
            lines.push(format!("     • {label}"));
        }
        lines.push("  Consider reviewing and addressing these patterns.".to_string());
    }
    lines.push(String::new());
    CommandResult::Handled(lines)
}

/// Handle the `/summary` command: show or update the cycle summary.
pub fn handle_summary(arg: &str) -> CommandResult {
    use crate::cycle_summary::CycleSummary;
    let mut cs = CycleSummary::default_path();

    if arg.is_empty() {
        match cs.format_for_system_prompt() {
            None => CommandResult::Handled(vec![
                "  No cycle summary yet for this session.".to_string(),
                "  Use: /summary <what you did> to add a completed item.".to_string(),
                "  The summary is automatically loaded by the next session.".to_string(),
                String::new(),
            ]),
            Some(text) => {
                let mut lines = vec!["  Current cycle summary:".to_string(), String::new()];
                for line in text.lines() {
                    lines.push(format!("  {line}"));
                }
                lines.push(String::new());
                CommandResult::Handled(lines)
            }
        }
    } else {
        cs.set_session("current session", "today");
        cs.add_completed(arg);
        match cs.save() {
            Ok(()) => CommandResult::Handled(vec![
                format!("  ✓ Added to cycle summary: {arg}"),
                "  Summary saved to .axonix/cycle_summary.json".to_string(),
                "  The next session will load this as startup context.".to_string(),
                String::new(),
            ]),
            Err(e) => CommandResult::Handled(vec![
                format!("  ✗ Failed to save cycle summary: {e}"),
                String::new(),
            ]),
        }
    }
}

/// Handle the `/recap` command: emit the sentinel for the repl loop.
pub fn handle_recap() -> CommandResult {
    CommandResult::Handled(vec!["__recap".to_string()])
}

/// Handle the `/archive-journal` command.
pub fn handle_archive_journal() -> CommandResult {
    CommandResult::ArchiveJournal
}
