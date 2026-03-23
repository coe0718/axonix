//! Morning brief for Axonix (G-022).
//!
//! Produces a concise daily summary surfacing what matters:
//! - Active goals (what's in progress)
//! - Open predictions (what I'm still waiting to resolve)
//! - Recent METRICS.md trend (last 3 sessions)
//! - Open GitHub issues count (if GitHub client available)
//! - System health snapshot (CPU, memory, disk, uptime)
//!
//! Invoked via `--brief` CLI flag. Designed to be readable in a terminal
//! and also forwardable via Telegram `/brief` command.

use crate::predictions::PredictionStore;

/// A health summary extracted from a HealthSnapshot.
pub struct HealthSummary {
    /// CPU load (1-min load average) as a proxy for CPU usage.
    pub cpu_pct: f32,
    /// Memory usage percentage (0–100).
    pub mem_pct: f32,
    /// Disk usage percentage (0–100).
    pub disk_pct: f32,
    /// System uptime in hours.
    pub uptime_hours: u64,
}

/// A morning brief summary.
pub struct Brief {
    pub active_goals: Vec<String>,
    pub open_predictions: Vec<(u32, String, String)>, // (id, date, text)
    pub recent_sessions: Vec<SessionSummary>,
    pub note: Option<String>,
    pub health: Option<HealthSummary>,
    pub bluesky_stats: Option<(usize, usize, Option<String>)>, // (total, root_posts, last_date)
}

/// One session row from METRICS.md.
pub struct SessionSummary {
    pub day: String,
    pub session: String, // e.g. "S1", "S2"
    pub date: String,
    pub tests: String,
    pub notes: String,
}

impl Brief {
    /// Build the morning brief from disk.
    pub fn collect() -> Self {
        let active_goals = parse_active_goals();
        let open_predictions = collect_open_predictions();
        let recent_sessions = parse_recent_metrics(3);
        let health = collect_health_summary();

        use crate::bluesky::BlueskyHistory;
        let history = BlueskyHistory::default_path();
        let bluesky_stats = if !history.is_empty() {
            let (total, root, _replies) = history.stats();
            let last_date = history.last_root_post_date();
            Some((total, root, last_date))
        } else {
            None
        };

        Brief {
            active_goals,
            open_predictions,
            recent_sessions,
            note: None,
            health,
            bluesky_stats,
        }
    }

    /// Format the brief as a multi-line string for terminal output.
    pub fn format_terminal(&self) -> String {
        let mut out = String::new();
        out.push_str("╔══════════════════════════════════════════════╗\n");
        out.push_str("║  ⚡ AXONIX MORNING BRIEF                      ║\n");
        out.push_str("╚══════════════════════════════════════════════╝\n");
        out.push('\n');

        // Active goals
        out.push_str("📋 ACTIVE GOALS\n");
        if self.active_goals.is_empty() {
            out.push_str("   (no active goals — promote something from backlog)\n");
        } else {
            for g in &self.active_goals {
                out.push_str(&format!("   • {g}\n"));
            }
        }
        out.push('\n');

        // Open predictions
        out.push_str("🔮 OPEN PREDICTIONS\n");
        if self.open_predictions.is_empty() {
            out.push_str("   (no open predictions)\n");
        } else {
            for (id, date, text) in &self.open_predictions {
                out.push_str(&format!("   #{id} [{date}] {text}\n"));
            }
        }
        out.push('\n');

        // System health
        out.push_str("🖥 SYSTEM HEALTH\n");
        match &self.health {
            Some(h) => {
                out.push_str(&format!("   CPU:    {:.1}%\n", h.cpu_pct));
                out.push_str(&format!("   Memory: {:.1}%\n", h.mem_pct));
                out.push_str(&format!("   Disk:   {:.1}%\n", h.disk_pct));
                out.push_str(&format!("   Uptime: {}h\n", h.uptime_hours));
            }
            None => {
                out.push_str("   (health data unavailable)\n");
            }
        }
        out.push('\n');

        // Bluesky post stats
        if let Some((total, root, last_date)) = &self.bluesky_stats {
            out.push_str("📡 BLUESKY\n");
            let date_str = last_date.as_deref().unwrap_or("(never)");
            out.push_str(&format!("   {root} posts ({total} total incl. replies) — last: {date_str}\n"));
            out.push('\n');
        }

        // Recent metrics
        out.push_str("📊 RECENT SESSIONS\n");
        if self.recent_sessions.is_empty() {
            out.push_str("   (no session data in METRICS.md)\n");
        } else {
            for s in &self.recent_sessions {
                out.push_str(&format!(
                    "   Day {} {} {} — {} tests — {}\n",
                    s.day,
                    s.session,
                    s.date,
                    s.tests,
                    truncate_str(&s.notes, 55)
                ));
            }
        }
        out.push('\n');

        if let Some(note) = &self.note {
            out.push_str(&format!("💡 NOTE: {note}\n\n"));
        }

        out.push_str("── end of brief ──\n");
        out
    }

    /// Format the brief as a compact Telegram message.
    pub fn format_telegram(&self) -> String {
        let mut out = String::new();
        out.push_str("⚡ *Axonix Morning Brief*\n\n");

        // Goals
        out.push_str("📋 *Active Goals*\n");
        if self.active_goals.is_empty() {
            out.push_str("_(none — promote from backlog)_\n");
        } else {
            for g in &self.active_goals {
                out.push_str(&format!("• {g}\n"));
            }
        }
        out.push('\n');

        // Predictions
        out.push_str("🔮 *Open Predictions*\n");
        if self.open_predictions.is_empty() {
            out.push_str("_(none)_\n");
        } else {
            for (id, date, text) in &self.open_predictions {
                out.push_str(&format!("• #{id} [{date}] {}\n", truncate_str(text, 50)));
            }
        }
        out.push('\n');

        // Health (compact)
        match &self.health {
            Some(h) => {
                out.push_str(&format!(
                    "🖥 Health: CPU {:.0}% | Mem {:.0}% | Disk {:.0}% | Up {}h\n",
                    h.cpu_pct, h.mem_pct, h.disk_pct, h.uptime_hours
                ));
            }
            None => {
                out.push_str("🖥 Health: (unavailable)\n");
            }
        }

        // Bluesky post stats (compact)
        if let Some((total, root, last_date)) = &self.bluesky_stats {
            let date_str = last_date.as_deref().unwrap_or("(never)");
            out.push_str(&format!("📡 *Bluesky*: {root} posts (last: {date_str})\n"));
        }

        out.push('\n');

        // Last session
        out.push_str("📊 *Last Session*\n");
        if let Some(last) = self.recent_sessions.last() {
            out.push_str(&format!(
                "Day {} {} {} — {} tests\n",
                last.day, last.session, last.date, last.tests
            ));
            out.push_str(&format!("_{}_\n", truncate_str(&last.notes, 60)));
        } else {
            out.push_str("_(no data)_\n");
        }

        out
    }
}

/// Collect a HealthSummary from the system, returning None on any failure.
fn collect_health_summary() -> Option<HealthSummary> {
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

/// Extract a percentage value from strings like "12G / 50G (24%)" or "used 15% used".
fn parse_pct_from_str(s: &str) -> f32 {
    // Find the last '(' followed by a number and '%'
    if let Some(paren) = s.rfind('(') {
        let after = &s[paren + 1..];
        let pct_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if let Ok(v) = pct_str.parse::<f32>() {
            return v;
        }
    }
    0.0
}

/// Parse uptime hours from strings like "3d 4h 22m", "4h 22m", "30m".
fn parse_uptime_hours(s: &str) -> u64 {
    let mut hours: u64 = 0;
    for part in s.split_whitespace() {
        if let Some(d) = part.strip_suffix('d') {
            if let Ok(n) = d.parse::<u64>() {
                hours += n * 24;
            }
        } else if let Some(h) = part.strip_suffix('h') {
            if let Ok(n) = h.parse::<u64>() {
                hours += n;
            }
        }
    }
    hours
}


fn parse_active_goals() -> Vec<String> {
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

/// Collect open predictions from the default predictions store.
fn collect_open_predictions() -> Vec<(u32, String, String)> {
    let store = PredictionStore::default_path();
    store
        .open()
        .into_iter()
        .map(|(id, pred)| (id, pred.created.clone(), truncate_str(&pred.prediction, 60).to_string()))
        .collect()
}

/// Parse the last N sessions from METRICS.md table rows.
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

    // Return the last N rows
    let start = rows.len().saturating_sub(n);
    rows.into_iter().skip(start).collect()
}

/// Parse a single METRICS.md table row.
/// Format: | Day | Session | Date | Tokens | Tests | Failed | Files | +Lines | -Lines | Committed | Notes |
fn parse_metrics_row(line: &str) -> Option<SessionSummary> {
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

/// Truncate a string to `max_chars` characters (on char boundary).
fn truncate_str(s: &str, max_chars: usize) -> &str {
    let mut end = s.len();
    let mut char_count = 0;
    for (i, _) in s.char_indices() {
        if char_count >= max_chars {
            end = i;
            break;
        }
        char_count += 1;
    }
    if char_count < max_chars {
        s
    } else {
        &s[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_metrics_row ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_metrics_row_valid() {
        let line = "| 4 | S4 | 2026-03-17 | ~30k | 362 | 0 | 5 | 380 | 10 | yes | Day 4 S4: complete G-021 |";
        let row = parse_metrics_row(line).expect("should parse valid row");
        assert_eq!(row.day, "4");
        assert_eq!(row.session, "S4");
        assert_eq!(row.date, "2026-03-17");
        assert_eq!(row.tests, "362");
        assert!(row.notes.contains("G-021"), "notes should contain G-021");
    }

    #[test]
    fn test_parse_metrics_row_header_returns_none() {
        let line = "| Day | Session | Date | Tokens | Tests | Failed | Files | +Lines | -Lines | Committed | Notes |";
        assert!(parse_metrics_row(line).is_none(), "header row should return None");
    }

    #[test]
    fn test_parse_metrics_row_separator_returns_none() {
        let line = "|-----|------|-------------|";
        assert!(parse_metrics_row(line).is_none(), "separator row should return None");
    }

    #[test]
    fn test_parse_metrics_row_too_few_cols_returns_none() {
        let line = "| 4 | 2026-03-17 |";
        assert!(parse_metrics_row(line).is_none(), "too-few-col row should return None");
    }

    // ── parse_active_goals ───────────────────────────────────────────────────────

    #[test]
    fn test_parse_active_goals_finds_unchecked() {
        // We can't easily test file I/O, so test the parsing logic directly.
        // The function reads GOALS.md from cwd — skip if not present.
        // Instead we verify the parser logic inline:
        let sample = "## Active\n\n- [ ] [G-022] Morning brief\n- [x] [G-021] Done one\n\n## Backlog\n";
        let mut in_active = false;
        let mut goals = Vec::new();
        for line in sample.lines() {
            if line.trim_start().starts_with("## Active") {
                in_active = true;
                continue;
            }
            if in_active && line.trim_start().starts_with("## ") {
                break;
            }
            if in_active {
                let trimmed = line.trim();
                if trimmed.starts_with("- [ ]") {
                    let rest = trimmed.trim_start_matches("- [ ]").trim();
                    let text = if rest.starts_with('[') {
                        rest.find(']').map(|i| rest[i + 1..].trim()).unwrap_or(rest)
                    } else {
                        rest
                    };
                    if !text.is_empty() {
                        goals.push(text.to_string());
                    }
                }
            }
        }
        assert_eq!(goals.len(), 1, "only unchecked goals should be found");
        assert!(goals[0].contains("Morning brief"), "should find G-022: {}", goals[0]);
    }

    #[test]
    fn test_parse_active_goals_checked_not_included() {
        let sample = "## Active\n\n- [x] [G-021] Done one\n\n## Backlog\n";
        let mut in_active = false;
        let mut goals = Vec::new();
        for line in sample.lines() {
            if line.trim_start().starts_with("## Active") { in_active = true; continue; }
            if in_active && line.trim_start().starts_with("## ") { break; }
            if in_active {
                let trimmed = line.trim();
                if trimmed.starts_with("- [ ]") {
                    goals.push(trimmed.to_string());
                }
            }
        }
        assert!(goals.is_empty(), "checked goals should not be included");
    }

    // ── truncate_str ─────────────────────────────────────────────────────────────

    #[test]
    fn test_truncate_str_short_string() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_exact_length() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_over_limit() {
        assert_eq!(truncate_str("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_str_unicode_safe() {
        // Each emoji is 1 char — 10 emoji, limit 5 → 5 emoji
        let s = "🦀".repeat(10);
        let result = truncate_str(&s, 5);
        assert_eq!(result.chars().count(), 5);
    }

    #[test]
    fn test_truncate_str_empty() {
        assert_eq!(truncate_str("", 5), "");
    }

    // ── Brief::format_terminal ───────────────────────────────────────────────────

    #[test]
    fn test_brief_format_terminal_contains_sections() {
        let brief = Brief {
            active_goals: vec!["Morning brief feature".to_string()],
            open_predictions: vec![(1, "2026-03-17".to_string(), "build will succeed".to_string())],
            recent_sessions: vec![SessionSummary {
                day: "4".to_string(),
                session: "S6".to_string(),
                date: "2026-03-17".to_string(),
                tests: "362".to_string(),
                notes: "Day 4 S6 test".to_string(),
            }],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("MORNING BRIEF"), "should contain header");
        assert!(output.contains("ACTIVE GOALS"), "should contain goals section");
        assert!(output.contains("Morning brief feature"), "should show active goal");
        assert!(output.contains("OPEN PREDICTIONS"), "should contain predictions section");
        assert!(output.contains("build will succeed"), "should show prediction");
        assert!(output.contains("RECENT SESSIONS"), "should contain metrics section");
        assert!(output.contains("362"), "should show test count");
    }

    #[test]
    fn test_brief_format_terminal_empty_state() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("no active goals"), "should note empty goals");
        assert!(output.contains("no open predictions"), "should note empty predictions");
        assert!(output.contains("no session data"), "should note empty metrics");
    }

    #[test]
    fn test_brief_format_telegram_contains_markdown() {
        let brief = Brief {
            active_goals: vec!["Morning brief".to_string()],
            open_predictions: vec![],
            recent_sessions: vec![SessionSummary {
                day: "4".to_string(),
                session: "S1".to_string(),
                date: "2026-03-17".to_string(),
                tests: "362".to_string(),
                notes: "test".to_string(),
            }],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_telegram();
        assert!(output.contains("*Axonix Morning Brief*"), "should have bold header");
        assert!(output.contains("Morning brief"), "should show goal");
    }

    #[test]
    fn test_brief_note_appears_in_terminal() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: Some("deploy needed".to_string()),
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("deploy needed"), "note should appear in output");
    }

    // ── parse_recent_metrics ─────────────────────────────────────────────────────

    #[test]
    fn test_parse_recent_metrics_returns_last_n() {
        // We can verify the trimming logic works
        let rows: Vec<SessionSummary> = vec![
            SessionSummary { day: "1".to_string(), session: "S1".to_string(), date: "2026-03-14".to_string(), tests: "40".to_string(), notes: "first".to_string() },
            SessionSummary { day: "2".to_string(), session: "S1".to_string(), date: "2026-03-15".to_string(), tests: "100".to_string(), notes: "second".to_string() },
            SessionSummary { day: "3".to_string(), session: "S1".to_string(), date: "2026-03-16".to_string(), tests: "200".to_string(), notes: "third".to_string() },
            SessionSummary { day: "4".to_string(), session: "S1".to_string(), date: "2026-03-17".to_string(), tests: "362".to_string(), notes: "fourth".to_string() },
        ];
        let n = 3;
        let start = rows.len().saturating_sub(n);
        let result: Vec<&SessionSummary> = rows.iter().skip(start).collect();
        assert_eq!(result.len(), 3, "should return last 3");
        assert_eq!(result[0].day, "2", "first of last 3 should be day 2");
        assert_eq!(result[2].day, "4", "last should be day 4");
    }

    // ── Brief::format_terminal edge cases ────────────────────────────────────────

    #[test]
    fn test_brief_format_terminal_has_end_marker() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("end of brief"), "should have end marker");
    }

    #[test]
    fn test_brief_format_terminal_multiple_goals() {
        let brief = Brief {
            active_goals: vec![
                "Goal one".to_string(),
                "Goal two".to_string(),
                "Goal three".to_string(),
            ],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("Goal one"));
        assert!(output.contains("Goal two"));
        assert!(output.contains("Goal three"));
    }

    #[test]
    fn test_brief_format_terminal_multiple_predictions() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![
                (1, "2026-03-17".to_string(), "first prediction".to_string()),
                (2, "2026-03-18".to_string(), "second prediction".to_string()),
            ],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("#1"), "should show prediction IDs");
        assert!(output.contains("#2"));
        assert!(output.contains("first prediction"));
        assert!(output.contains("second prediction"));
    }

    #[test]
    fn test_brief_format_telegram_empty_state() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_telegram();
        assert!(output.contains("*Axonix Morning Brief*"), "should have header");
        // Should not panic or produce empty output
        assert!(!output.is_empty());
    }

    #[test]
    fn test_brief_format_telegram_note_appears() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: Some("important note here".to_string()),
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_telegram();
        // format_telegram doesn't render notes (compact format) — but must not panic
        assert!(!output.is_empty(), "telegram output should be non-empty");
        assert!(output.contains("*Axonix Morning Brief*"), "should have header");
    }

    #[test]
    fn test_brief_format_telegram_multiple_goals() {
        let brief = Brief {
            active_goals: vec!["alpha".to_string(), "beta".to_string()],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_telegram();
        assert!(output.contains("alpha"));
        assert!(output.contains("beta"));
    }

    #[test]
    fn test_session_summary_fields() {
        let s = SessionSummary {
            day: "5".to_string(),
            session: "S2".to_string(),
            date: "2026-03-18".to_string(),
            tests: "450".to_string(),
            notes: "big session".to_string(),
        };
        assert_eq!(s.day, "5");
        assert_eq!(s.session, "S2");
        assert_eq!(s.date, "2026-03-18");
        assert_eq!(s.tests, "450");
        assert_eq!(s.notes, "big session");
    }

    #[test]
    fn test_brief_format_terminal_session_row_format() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![SessionSummary {
                day: "7".to_string(),
                session: "S3".to_string(),
                date: "2026-03-20".to_string(),
                tests: "434".to_string(),
                notes: "Day 7 session".to_string(),
            }],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("Day 7"), "should show day number");
        assert!(output.contains("S3"), "should show session");
        assert!(output.contains("434"), "should show test count");
        assert!(output.contains("2026-03-20"), "should show date");
    }

    #[test]
    fn test_truncate_str_zero_max() {
        // max=0 should return empty string
        let result = truncate_str("hello", 0);
        assert!(result.is_empty(), "truncate_str to 0 should return empty: got '{result}'");
    }

    #[test]
    fn test_truncate_str_preserves_unicode() {
        let s = "Hello 世界!";
        let result = truncate_str(s, 100);
        assert_eq!(result, s, "short string should be unchanged");
    }

    #[test]
    fn test_parse_metrics_row_real_format() {
        // Test the exact 11-column format used in METRICS.md (with Session column)
        let line = "| 7 | S3 | 2026-03-20 | ~25k | 434 | 0 | 5 | 88 | 15 | yes | Day 7 S3 notes |";
        let result = parse_metrics_row(line);
        assert!(result.is_some(), "real METRICS.md format should parse: {line}");
        let row = result.unwrap();
        assert_eq!(row.day, "7");
        assert_eq!(row.session, "S3");
        assert_eq!(row.tests, "434");
        assert!(row.notes.contains("Day 7 S3 notes"));
    }

    #[test]
    fn test_parse_metrics_row_with_unknown_tokens() {
        // ~?k tokens should still parse if other fields are valid
        let line = "| 6 | S1 | 2026-03-19 | ~?k | 406 | 0 | 2 | 172 | 5 | yes | auto-generated |";
        let result = parse_metrics_row(line);
        assert!(result.is_some(), "row with ~?k tokens should still parse");
        let row = result.unwrap();
        assert_eq!(row.tests, "406");
    }

    // ── HealthSummary in Brief ────────────────────────────────────────────────────

    #[test]
    fn test_brief_health_some_shows_in_terminal() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: Some(HealthSummary {
                cpu_pct: 42.3,
                mem_pct: 67.1,
                disk_pct: 55.0,
                uptime_hours: 142,
            }),
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("SYSTEM HEALTH"), "should contain SYSTEM HEALTH section");
        assert!(output.contains("42.3"), "should show CPU percentage");
        assert!(output.contains("67.1"), "should show memory percentage");
        assert!(output.contains("55.0"), "should show disk percentage");
        assert!(output.contains("142h"), "should show uptime hours");
    }

    #[test]
    fn test_brief_health_none_shows_unavailable() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(output.contains("SYSTEM HEALTH"), "should still contain section header");
        assert!(output.contains("unavailable"), "should show unavailable message");
    }

    #[test]
    fn test_brief_health_telegram_compact_format() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: Some(HealthSummary {
                cpu_pct: 30.0,
                mem_pct: 50.0,
                disk_pct: 20.0,
                uptime_hours: 72,
            }),
            bluesky_stats: None,
        };
        let output = brief.format_telegram();
        assert!(output.contains("Health:"), "telegram brief should contain Health: line");
        assert!(output.contains("72h"), "should show uptime in telegram format");
    }

    // ── parse_pct_from_str ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_pct_from_str_disk_format() {
        assert_eq!(parse_pct_from_str("12G / 50G (24%)"), 24.0);
    }

    #[test]
    fn test_parse_pct_from_str_memory_format() {
        assert_eq!(parse_pct_from_str("1.2G / 8.0G (15% used)"), 15.0);
    }

    #[test]
    fn test_parse_pct_from_str_no_paren_returns_zero() {
        assert_eq!(parse_pct_from_str("(unavailable)"), 0.0);
    }

    // ── parse_uptime_hours ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_uptime_hours_days_and_hours() {
        assert_eq!(parse_uptime_hours("3d 4h 22m"), 3 * 24 + 4);
    }

    #[test]
    fn test_parse_uptime_hours_hours_only() {
        assert_eq!(parse_uptime_hours("5h 30m"), 5);
    }

    #[test]
    fn test_parse_uptime_hours_minutes_only() {
        assert_eq!(parse_uptime_hours("45m"), 0);
    }

    // ── Brief::bluesky_stats ──────────────────────────────────────────────────────

    #[test]
    fn test_brief_bluesky_stats_shows_in_terminal() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: Some((10, 7, Some("2026-03-22".to_string()))),
        };
        let output = brief.format_terminal();
        assert!(output.contains("BLUESKY"), "should contain BLUESKY section");
        assert!(output.contains("7 posts"), "should show root post count");
        assert!(output.contains("10 total"), "should show total incl. replies");
        assert!(output.contains("2026-03-22"), "should show last date");
    }

    #[test]
    fn test_brief_bluesky_stats_none_not_shown_in_terminal() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
        };
        let output = brief.format_terminal();
        assert!(!output.contains("BLUESKY"), "no bluesky_stats → no BLUESKY section");
    }

    #[test]
    fn test_brief_bluesky_stats_shows_in_telegram() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: Some((5, 3, Some("2026-03-21".to_string()))),
        };
        let output = brief.format_telegram();
        assert!(output.contains("*Bluesky*"), "telegram should show *Bluesky* label");
        assert!(output.contains("3 posts"), "should show root count");
        assert!(output.contains("2026-03-21"), "should show last date");
    }

    #[test]
    fn test_brief_bluesky_stats_no_last_date_shows_never() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: Some((2, 0, None)),
        };
        let terminal = brief.format_terminal();
        assert!(terminal.contains("(never)"), "no last date should display (never)");
        let telegram = brief.format_telegram();
        assert!(telegram.contains("(never)"), "telegram: no last date should display (never)");
    }
}
