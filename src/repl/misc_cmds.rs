//! Miscellaneous REPL command handlers extracted from `handle_command`.
//!
//! Covers: `/failures`, `/summary`, `/recap`, `/archive-journal`,
//!          `/brief`, `/search`.

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

/// Handle the `/brief` command: collect and display the morning brief.
///
/// Calls `Brief::collect()` and `Brief::format_terminal()` — the same
/// path used by the `--brief` startup flag.  Works entirely from disk
/// (goals, predictions, health, journal) without an Anthropic API call.
pub fn handle_brief() -> CommandResult {
    use crate::brief::Brief;
    let brief = Brief::collect();
    let text = brief.format_terminal();
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    lines.push(String::new());
    CommandResult::Handled(lines)
}

/// Handle the `/search <query>` command: semantic similarity search over
/// the embeddings store.
///
/// Embeds `query` via Ollama, loads every stored `(obs_key, vector)` row,
/// computes cosine similarity, and returns the top 5.
/// Gracefully handles an empty query or an unreachable Ollama instance.
pub fn handle_search(query: &str) -> CommandResult {
    if query.trim().is_empty() {
        return CommandResult::Handled(vec![
            "  Usage: /search <query>".to_string(),
            "  Example: /search rust borrow checker".to_string(),
            String::new(),
        ]);
    }

    // Embed the query via Ollama.
    let query_vec = match crate::embeddings::embed(query) {
        Ok(v) => v,
        Err(e) => {
            return CommandResult::Handled(vec![
                format!("  [search] embedding failed: {e}"),
                "  Is Ollama running? Check OLLAMA_URL environment variable.".to_string(),
                String::new(),
            ]);
        }
    };

    // Load all stored embeddings.
    let db = match crate::db::AxonixDb::open_default() {
        Ok(d) => d,
        Err(e) => {
            return CommandResult::Handled(vec![
                format!("  [search] could not open embeddings store: {e}"),
                String::new(),
            ]);
        }
    };

    let rows = match db.embeddings_list_by_key() {
        Ok(r) => r,
        Err(e) => {
            return CommandResult::Handled(vec![
                format!("  [search] could not read embeddings: {e}"),
                String::new(),
            ]);
        }
    };

    if rows.is_empty() {
        return CommandResult::Handled(vec![
            "  [search] embeddings store is empty — no results.".to_string(),
            String::new(),
        ]);
    }

    // Score and rank.
    let mut scored: Vec<(f32, String)> = rows
        .into_iter()
        .map(|(key, vec)| {
            let sim = crate::embeddings::cosine_similarity(&query_vec, &vec);
            (sim, key)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(5);

    let mut lines = vec![
        format!("  [search] top {} results for \"{query}\":", scored.len()),
        String::new(),
    ];
    for (rank, (score, key)) in scored.iter().enumerate() {
        // obs_key format is typically "source:excerpt" or just a key string.
        let (source, excerpt) = if let Some(colon) = key.find(':') {
            (&key[..colon], &key[colon + 1..])
        } else {
            ("result", key.as_str())
        };
        let preview = if excerpt.len() > 70 {
            format!("{}…", &excerpt[..70])
        } else {
            excerpt.to_string()
        };
        lines.push(format!("  {}. [{:.2}] {source}: \"{preview}\"", rank + 1, score));
    }
    lines.push(String::new());
    CommandResult::Handled(lines)
}
