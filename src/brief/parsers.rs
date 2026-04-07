//! Parsing and collection functions for the morning brief.

use std::path::Path;
use crate::predictions::PredictionStore;
use super::types::{HealthSummary, SessionSummary};
use super::helpers::truncate_str;
use super::helpers::parse_pct_from_str;
use super::helpers::parse_uptime_hours;

pub fn parse_active_goals() -> Vec<String> {
    let content = match std::fs::read_to_string("GOALS.md") {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut in_active = false;
    let mut goals = Vec::new();

    for line in content.lines() {
        if line.trim_start().starts_with("## Active") {
            in_active = true;
            continue;
        }
        if in_active && line.trim_start().starts_with("## ") {
            break; // left the Active section
        }
        if in_active {
            let trimmed = line.trim();
            // Only pick up unchecked goals: "- [ ] [G-NNN] ..."
            if trimmed.starts_with("- [ ]") {
                // Extract just the readable text
                let rest = trimmed.trim_start_matches("- [ ]").trim();
                // Strip the [G-NNN] tag if present
                let text = if rest.starts_with('[') {
                    rest.find(']')
                        .map(|i| rest[i + 1..].trim())
                        .unwrap_or(rest)
                } else {
                    rest
                };
                if !text.is_empty() {
                    goals.push(text.to_string());
                }
            }
        }
    }
    goals
}

/// Parse goal titles from the `## Backlog` section of GOALS.md.
///
/// Returns `Vec<String>` of goal titles (from `### G-NNN — <title>` heading lines),
/// each truncated to 60 characters to fit in Telegram messages.
pub fn parse_backlog_goals() -> Vec<String> {
    let content = match std::fs::read_to_string("GOALS.md") {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut in_backlog = false;
    let mut goals = Vec::new();

    for line in content.lines() {
        if line.trim_start().starts_with("## Backlog") {
            in_backlog = true;
            continue;
        }
        if in_backlog && line.trim_start().starts_with("## ") {
            break; // left the Backlog section
        }
        if in_backlog {
            let trimmed = line.trim();
            if trimmed.starts_with("### ") {
                let title = trimmed.trim_start_matches("### ").trim();
                if !title.is_empty() {
                    goals.push(truncate_str(title, 60).to_string());
                }
            }
        }
    }
    goals
}

/// Parse the most recent N sessions from METRICS.md table rows.
///
/// METRICS.md is newest-first (G-071: rows inserted after the header separator),
/// so we take the first N rows rather than the last N. Stub rows where tests == "?"
/// (in-progress placeholders) are skipped.
pub fn parse_recent_metrics(n: usize) -> Vec<SessionSummary> {
    let content = match std::fs::read_to_string("METRICS.md") {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let rows: Vec<SessionSummary> = content
        .lines()
        .filter(|l| l.starts_with('|') && !l.contains("---") && !l.contains("Day |") && !l.contains("<!-- "))
        .filter_map(|line| parse_metrics_row(line))
        .collect();

    // METRICS.md is newest-first (G-071), so take the first N rows.
    // Skip stub rows (tests == "?" means in-progress placeholder).
    rows.into_iter()
        .filter(|s| s.tests != "?")
        .take(n)
        .collect()
}

/// Parse journal entry titles (lines starting with `## `) from a string.
///
/// Returns the last `n` such headings (full text after `## `), preserving
/// document order (oldest → newest within the slice).
pub fn parse_journal_entries_from_str(content: &str, n: usize) -> Vec<String> {
    let all: Vec<String> = content
        .lines()
        .filter(|l| l.starts_with("## "))
        .map(|l| l.trim_start_matches("## ").to_string())
        .collect();
    let skip = all.len().saturating_sub(n);
    all.into_iter().skip(skip).collect()
}

/// Read JOURNAL.md and return the last `n` entry headings.
///
/// Looks first at the workspace root (`/workspace/JOURNAL.md`), then falls
/// back to `JOURNAL.md` relative to the current working directory. Returns an
/// empty vec if the file does not exist or cannot be read.
pub fn parse_recent_journal_entries(n: usize) -> Vec<String> {
    let paths = [
        Path::new("/workspace/JOURNAL.md"),
        Path::new("JOURNAL.md"),
    ];
    for path in &paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            return parse_journal_entries_from_str(&content, n);
        }
    }
    vec![]
}

/// Parse a single METRICS.md table row.
/// Format: | Day | Session | Date | Tokens | Tests | Failed | Files | +Lines | -Lines | Committed | Notes |
pub(crate) fn parse_metrics_row(line: &str) -> Option<SessionSummary> {
    let cols: Vec<&str> = line
        .split('|')
        .map(|s| s.trim())
        .collect();
    // Need at least 12 columns:
    //   [0]="", [1]=Day, [2]=Session, [3]=Date, [4]=Tokens, [5]=Tests,
    //   [6]=Failed, [7]=Files, [8]=+Lines, [9]=-Lines, [10]=Committed, [11]=Notes, [12]=""
    if cols.len() < 12 {
        return None;
    }
    let day = cols.get(1)?.trim().to_string();
    let session = cols.get(2).unwrap_or(&"").trim().to_string();
    let date = cols.get(3)?.trim().to_string();
    let tests = cols.get(5)?.trim().to_string();
    let notes = cols.get(11).unwrap_or(&"").trim().to_string();

    // Skip header row: Day must parse as a number
    if day.parse::<u32>().is_err() {
        return None;
    }
    if date.is_empty() {
        return None;
    }

    Some(SessionSummary { day, session, date, tests, notes })
}

/// Collect a HealthSummary from the system, returning None on any failure.
pub(crate) fn collect_health_summary() -> Option<HealthSummary> {
    let snap = crate::health::HealthSnapshot::collect();
    // Parse CPU: use 1-min load average as a proxy percentage (clamped 0–100)
    let cpu_pct = snap
        .load_avg
        .split(',')
        .next()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|v| (v * 100.0).min(100.0))
        .unwrap_or(0.0);
    // Parse memory %: look for "(N% used)" or "(N%)" in the memory string
    let mem_pct = parse_pct_from_str(&snap.memory);
    // Parse disk %: look for "(N%)" in the disk string
    let disk_pct = parse_pct_from_str(&snap.disk);
    // Parse uptime hours from the uptime string (e.g. "3d 4h 22m" or "4h 22m")
    let uptime_hours = parse_uptime_hours(&snap.uptime);
    Some(HealthSummary { cpu_pct, mem_pct, disk_pct, uptime_hours })
}

/// Collect open predictions from the default predictions store.
pub(crate) fn collect_open_predictions() -> Vec<(u32, String, String)> {
    let store = PredictionStore::default_path();
    store
        .open()
        .into_iter()
        .map(|(id, pred)| (id, pred.created.clone(), truncate_str(&pred.prediction, 60).to_string()))
        .collect()
}

/// Collect predictions due within 3 days (have "By Day N" where N ≤ current_day + 3).
///
/// Uses the DAY_COUNT env var (format: "N YYYY-MM-DD") to determine the current day.
/// Returns an empty vec when DAY_COUNT is unset or zero, or when no predictions match.
pub(crate) fn collect_predictions_due_soon() -> Vec<(u32, String, String)> {
    let open = collect_open_predictions();
    let current_day: u32 = std::env::var("DAY_COUNT")
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if current_day == 0 {
        return vec![];
    }

    let threshold = current_day + 3;

    open.into_iter()
        .filter(|(_id, _date, text)| {
            // Look for "by day N" (case-insensitive) and extract N
            let lower = text.to_lowercase();
            if let Some(pos) = lower.find("by day ") {
                let rest = &text[pos + 7..];
                let num_str: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    return n <= threshold;
                }
            }
            false
        })
        .collect()
}

/// Search the axonix DB memory for the top `limit` results matching `goal_title`.
///
/// Tries semantic search (via Ollama embeddings) first; falls back to keyword
/// search if Ollama is unavailable or returns no results.
///
/// Returns an empty vec when the DB is unavailable, the query is empty, or no
/// results are found — never panics.
pub fn collect_memory_context(goal_title: &str) -> Vec<(String, f64)> {
    if goal_title.is_empty() {
        return vec![];
    }
    match crate::db::AxonixDb::open_default() {
        Ok(db) => {
            // Try semantic search first; fall back to keyword search
            let semantic = db.semantic_search_memory(goal_title, 3).unwrap_or_default();
            if !semantic.is_empty() {
                return semantic.into_iter().map(|row| (row.text, row.score)).collect();
            }
            // Keyword fallback
            db.search_memory(goal_title, 3)
                .unwrap_or_default()
                .into_iter()
                .map(|row| (row.text, row.score))
                .collect()
        }
        Err(_) => vec![],
    }
}
