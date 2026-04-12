//! Miscellaneous REPL command handlers extracted from `handle_command`.
//!
//! Covers: `/failures`, `/summary`, `/recap`, `/archive-journal`,
//!          `/brief`, `/search`, `/files`.

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

/// Walk `src/` recursively and collect all `.rs` files with their line counts.
/// Returns a sorted-descending list of `(line_count, path_string)`.
fn collect_rs_files(root: &str) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from(root)];
    while let Some(dir) = dirs.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let contents = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let line_count = contents.chars().filter(|&c| c == '\n').count() + 1;
                let display = path.display().to_string();
                results.push((line_count, display));
            }
        }
    }
    results.sort_by(|a, b| b.0.cmp(&a.0));
    results
}

/// Handle the `/files [N]` command: list `.rs` source files over `threshold` lines.
///
/// Files are sorted descending by line count. Test files (`tests.rs`) are shown
/// with a `(tests — exempt)` tag rather than counted as violations.
pub fn handle_files(threshold: usize) -> CommandResult {
    let all_files = collect_rs_files("src");

    if all_files.is_empty() {
        return CommandResult::Handled(vec![
            "  [files] No .rs files found under src/".to_string(),
            "  Run this command from the workspace root.".to_string(),
            String::new(),
        ]);
    }

    // Separate violations from exempt test files.
    let mut violations: Vec<(usize, String)> = Vec::new();
    let mut exempt: Vec<(usize, String)> = Vec::new();
    for (lines, path) in &all_files {
        if *lines > threshold {
            let is_test = path.ends_with("tests.rs")
                || path.ends_with("/tests.rs")
                || path.contains("tests.rs");
            if is_test {
                exempt.push((*lines, path.clone()));
            } else {
                violations.push((*lines, path.clone()));
            }
        }
    }

    let mut out = Vec::new();
    out.push(format!(
        "  Source files over {threshold} lines ({} violation{}):",
        violations.len(),
        if violations.len() == 1 { "" } else { "s" }
    ));
    out.push(String::new());

    if violations.is_empty() {
        out.push(format!(
            "  ✓ No violations — all non-test .rs files are under {threshold} lines."
        ));
    } else {
        for (count, path) in &violations {
            out.push(format!("  {:>5}  {}", count, path));
        }
    }

    if !exempt.is_empty() {
        out.push(String::new());
        out.push("  (tests — exempt):".to_string());
        for (count, path) in &exempt {
            out.push(format!("  {:>5}  {}  (tests — exempt)", count, path));
        }
    }

    out.push(String::new());
    out.push(format!(
        "  Run /files <N> to use a custom threshold (current: {threshold})."
    ));
    out.push(String::new());
    CommandResult::Handled(out)
}
