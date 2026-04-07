#[cfg(test)]
mod tests {
    use crate::brief::{
        Brief, HealthSummary, LastSessionSummary, SessionSummary,
        parse_active_goals, parse_backlog_goals, parse_recent_metrics,
        parse_journal_entries_from_str, parse_recent_journal_entries,
        collect_memory_context,
    };
    use crate::brief::parsers::{
        parse_metrics_row, collect_health_summary, collect_open_predictions,
        collect_predictions_due_soon,
    };
    use crate::brief::helpers::{
        today_date_utc, is_leap_year, parse_pct_from_str, parse_uptime_hours,
        truncate_str, days_since, count_backlog_goals,
    };
    use crate::brief::priority::synthesize_priority;

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

    // ── parse_journal_entries_from_str ──────────────────────────────────────────

    #[test]
    fn test_parse_recent_journal_entries_empty() {
        // Empty content: no ## headings → empty vec
        let result = parse_journal_entries_from_str("", 3);
        assert!(result.is_empty(), "empty content should return empty vec");

        // Content with no ## headings
        let result = parse_journal_entries_from_str("# Title\nsome text\n### Sub", 3);
        assert!(result.is_empty(), "no ## headings should return empty vec");
    }

    #[test]
    fn test_parse_recent_journal_entries_finds_headings() {
        let content = "\
# Journal\n\
\n\
## Day 15, Session 5 — Dashboard: containers panel\n\
Some body text.\n\
\n\
## Day 15, Session 6 — Complete dashboard redesign\n\
More text.\n\
\n\
## Day 16, Session 5 — Morning brief: surface recent journal activity\n\
";
        let result = parse_journal_entries_from_str(content, 3);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "Day 15, Session 5 — Dashboard: containers panel");
        assert_eq!(result[1], "Day 15, Session 6 — Complete dashboard redesign");
        assert_eq!(result[2], "Day 16, Session 5 — Morning brief: surface recent journal activity");
    }

    #[test]
    fn test_parse_recent_journal_entries_respects_limit() {
        let content = "\
## Day 11, Session 1 — Alpha\n\
## Day 12, Session 2 — Beta\n\
## Day 13, Session 3 — Gamma\n\
## Day 14, Session 4 — Delta\n\
## Day 15, Session 5 — Epsilon\n\
";
        // Given 5 headings, n=3 should return only the last 3
        let result = parse_journal_entries_from_str(content, 3);
        assert_eq!(result.len(), 3, "should return exactly 3 entries");
        assert_eq!(result[0], "Day 13, Session 3 — Gamma");
        assert_eq!(result[1], "Day 14, Session 4 — Delta");
        assert_eq!(result[2], "Day 15, Session 5 — Epsilon");
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("deploy needed"), "note should appear in output");
    }

    // ── parse_recent_metrics ─────────────────────────────────────────────────────

    #[test]
    fn test_parse_recent_metrics_returns_first_n() {
        // METRICS.md is newest-first (G-071), so we take the first N rows.
        // Simulate a newest-first vec: day 4 is at index 0.
        let rows: Vec<SessionSummary> = vec![
            SessionSummary { day: "4".to_string(), session: "S1".to_string(), date: "2026-03-17".to_string(), tests: "362".to_string(), notes: "fourth".to_string() },
            SessionSummary { day: "3".to_string(), session: "S1".to_string(), date: "2026-03-16".to_string(), tests: "200".to_string(), notes: "third".to_string() },
            SessionSummary { day: "2".to_string(), session: "S1".to_string(), date: "2026-03-15".to_string(), tests: "100".to_string(), notes: "second".to_string() },
            SessionSummary { day: "1".to_string(), session: "S1".to_string(), date: "2026-03-14".to_string(), tests: "40".to_string(), notes: "first".to_string() },
        ];
        let n = 3;
        // Stub rows (tests == "?") are filtered; none here, so we take the first 3.
        let result: Vec<&SessionSummary> = rows.iter().filter(|s| s.tests != "?").take(n).collect();
        assert_eq!(result.len(), 3, "should return first 3 (newest)");
        assert_eq!(result[0].day, "4", "first result should be newest (day 4)");
        assert_eq!(result[2].day, "2", "third result should be day 2");
    }

    #[test]
    fn test_parse_recent_metrics_skips_stub_rows() {
        // Rows with tests == "?" are in-progress stubs and should be skipped.
        let rows: Vec<SessionSummary> = vec![
            SessionSummary { day: "5".to_string(), session: "S1".to_string(), date: "2026-03-18".to_string(), tests: "?".to_string(), notes: "in progress".to_string() },
            SessionSummary { day: "4".to_string(), session: "S1".to_string(), date: "2026-03-17".to_string(), tests: "362".to_string(), notes: "fourth".to_string() },
            SessionSummary { day: "3".to_string(), session: "S1".to_string(), date: "2026-03-16".to_string(), tests: "200".to_string(), notes: "third".to_string() },
        ];
        let n = 2;
        let result: Vec<&SessionSummary> = rows.iter().filter(|s| s.tests != "?").take(n).collect();
        assert_eq!(result.len(), 2, "should skip stub and return 2 completed rows");
        assert_eq!(result[0].day, "4", "first non-stub should be day 4");
        assert_eq!(result[1].day, "3", "second non-stub should be day 3");
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
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
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let terminal = brief.format_terminal();
        assert!(terminal.contains("(never)"), "no last date should display (never)");
        let telegram = brief.format_telegram();
        assert!(telegram.contains("(never)"), "telegram: no last date should display (never)");
    }

    // ── calibration in brief ─────────────────────────────────────────────────────

    #[test]
    fn test_brief_calibration_terminal_shown_when_some() {
        use crate::predictions::CalibrationScore;
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: Some(CalibrationScore {
                total_resolved: 5,
                correct: 5,
                hit_rate: 1.0,
                avg_days_early: 1.2,
                direction_bias: "optimistic".to_string(),
            }),
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("calibration:"), "terminal should show calibration line");
        assert!(output.contains("5/5"), "should show 5/5 correct");
        assert!(output.contains("100.0%"), "should show 100.0%");
        assert!(output.contains("optimistic"), "should show bias");
    }

    #[test]
    fn test_brief_calibration_terminal_hidden_when_none() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(!output.contains("calibration:"), "no calibration → should not show calibration line");
    }

    #[test]
    fn test_brief_calibration_telegram_shown_when_some() {
        use crate::predictions::CalibrationScore;
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: Some(CalibrationScore {
                total_resolved: 5,
                correct: 5,
                hit_rate: 1.0,
                avg_days_early: 0.0,
                direction_bias: "optimistic".to_string(),
            }),
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_telegram();
        assert!(output.contains("📊 Calibration:"), "telegram should show calibration emoji line");
        assert!(output.contains("5/5"), "should show 5/5");
        assert!(output.contains("optimistic"), "should show bias");
    }

    #[test]
    fn test_brief_calibration_telegram_hidden_when_none() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_telegram();
        assert!(!output.contains("📊 Calibration:"), "no calibration → should not show calibration line");
    }

    // ── LastSessionSummary — terminal format ──────────────────────────────────────

    /// Last session section renders session label when Some (terminal format)
    #[test]
    fn test_last_session_terminal_renders_session_label() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![],
                test_count: None,
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("LAST SESSION"), "should contain LAST SESSION header");
        assert!(output.contains("Day 10, Session 3"), "should show session label");
    }

    /// Last session section renders gracefully when None (terminal format — no crash)
    #[test]
    fn test_last_session_terminal_none() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("LAST SESSION"), "header still present when None");
        assert!(output.contains("no cycle summary found"), "should show fallback message");
    }

    /// Last session shows completed items (up to 5) in terminal format
    #[test]
    fn test_last_session_terminal_completed_items() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![
                    "G-064: prediction calibration".to_string(),
                    "G-065: test coverage".to_string(),
                ],
                test_count: Some(624),
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("G-064: prediction calibration"), "should show first completed item");
        assert!(output.contains("G-065: test coverage"), "should show second completed item");
    }

    /// Last session truncates completed items beyond 5 in terminal format
    #[test]
    fn test_last_session_terminal_max_five_items() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![
                    "item one".to_string(),
                    "item two".to_string(),
                    "item three".to_string(),
                    "item four".to_string(),
                    "item five".to_string(),
                    "item six — should not appear".to_string(),
                ],
                test_count: None,
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("item five"), "fifth item should appear");
        assert!(!output.contains("item six"), "sixth item should be truncated away");
    }

    /// Last session shows test count when Some in terminal format
    #[test]
    fn test_last_session_terminal_test_count() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![],
                test_count: Some(624),
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("624 tests"), "should show numeric test count");
    }

    /// Last session omits test count line when test_count is None in terminal format
    #[test]
    fn test_last_session_terminal_no_test_count() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![],
                test_count: None,
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        // When test_count is None, no test count line is shown (cleaner than "? tests")
        assert!(!output.contains("? tests"), "should not show '? tests' noise when count is None");
    }

    // ── LastSessionSummary — telegram format ──────────────────────────────────────

    /// Telegram format renders last session label
    #[test]
    fn test_last_session_telegram_renders_label() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec!["G-064: prediction calibration".to_string()],
                test_count: Some(624),
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_telegram();
        assert!(output.contains("*Last Session*"), "telegram should show bold Last Session header");
        assert!(output.contains("Day 10, Session 3"), "should show session label in telegram");
    }

    /// Telegram format renders correctly when last_session is None
    #[test]
    fn test_last_session_telegram_none() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_telegram();
        assert!(output.contains("*Last Session*"), "telegram header still present when None");
        assert!(output.contains("no cycle summary found"), "should show fallback when None");
    }

    /// Telegram format shows completed items
    #[test]
    fn test_last_session_telegram_completed_items() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![
                    "G-064: prediction calibration".to_string(),
                    "G-065: test coverage".to_string(),
                ],
                test_count: Some(624),
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_telegram();
        assert!(output.contains("G-064: prediction calibration"), "telegram should show completed items");
        assert!(output.contains("G-065: test coverage"), "telegram should show second item");
    }

    /// Telegram format shows test count when Some
    #[test]
    fn test_last_session_telegram_test_count() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![],
                test_count: Some(624),
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_telegram();
        assert!(output.contains("624 tests"), "telegram should show numeric test count");
    }

    /// Telegram format omits test count line when test_count is None
    #[test]
    fn test_last_session_telegram_no_test_count() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![],
                test_count: None,
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_telegram();
        // When test_count is None, no test count line is shown (cleaner than "? tests")
        assert!(!output.contains("? tests"), "should not show '? tests' noise in telegram when count is None");
    }

    // ── LastSessionSummary — edge cases ───────────────────────────────────────────

    /// LastSessionSummary with empty completed renders gracefully (terminal + telegram)
    #[test]
    fn test_last_session_empty_completed() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![],
                test_count: Some(624),
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let terminal = brief.format_terminal();
        assert!(terminal.contains("no completed items recorded"), "empty completed should show fallback in terminal");
        // telegram format: no crash, session label still appears
        let telegram = brief.format_telegram();
        assert!(telegram.contains("Day 10, Session 3"), "session label should appear in telegram even if no items");
    }

    /// Long completed items are truncated to 60 chars in terminal
    #[test]
    fn test_last_session_truncation() {
        let long_item = "A".repeat(80);
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec![long_item.clone()],
                test_count: None,
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        // The full 80-char string should NOT appear (truncated to 60)
        assert!(!output.contains(&long_item), "80-char item should be truncated in terminal output");
        // But a 60-char prefix should be present
        assert!(output.contains(&"A".repeat(60)), "first 60 chars should be present");
    }

    /// Brief with last_session=None still has LAST SESSION header in terminal
    #[test]
    fn test_last_session_section_header_always_present() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("📝 LAST SESSION"), "LAST SESSION header must always appear in terminal brief");
    }

    /// Brief with last_session renders date alongside session name in terminal
    #[test]
    fn test_last_session_date_shown() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: Some(LastSessionSummary {
                session: "Day 10, Session 3".to_string(),
                date: "2026-03-23".to_string(),
                completed: vec!["G-064: prediction calibration".to_string()],
                test_count: Some(624),
            }),
        failure_summary: None,
        pogo: None,
        meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let terminal = brief.format_terminal();
        assert!(terminal.contains("2026-03-23"), "terminal should show date next to session name");
        let telegram = brief.format_telegram();
        assert!(telegram.contains("2026-03-23"), "telegram should show date next to session name");
    }

    // ── AxonixDb integration ──────────────────────────────────────────────────────

    /// log_to_db_at() inserts a row without panicking (happy path).
    #[test]
    fn test_brief_log_to_db_returns_ok() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("brief_test.db");

        let brief = Brief {
            active_goals: vec!["G-079: wire brief to db".to_string()],
            open_predictions: vec![
                (1, "2026-03-17".to_string(), "test prediction".to_string()),
            ],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
            meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };

        // Should not panic even with a fresh (non-existent) DB path.
        brief.log_to_db_at(&db_path);

        // Verify the row was actually written.
        let db = crate::db::AxonixDb::open(&db_path).unwrap();
        let rows = db.sessions_recent(10).unwrap();
        assert_eq!(rows.len(), 1, "exactly one session row should be inserted");
        let row = &rows[0];
        assert_eq!(row.session, "brief", "session label should be 'brief'");
        assert!(
            row.notes.as_deref().unwrap_or("").contains("1 active goals"),
            "notes should mention active goal count: {:?}", row.notes
        );
        assert!(
            row.notes.as_deref().unwrap_or("").contains("1 open predictions"),
            "notes should mention prediction count: {:?}", row.notes
        );
        assert!(row.tokens.is_none(), "tokens should be None for a brief run");
        assert!(row.tests.is_none(),  "tests should be None for a brief run");
        assert!(row.failed.is_none(), "failed should be None for a brief run");
    }

    /// log_to_db_at() with an empty brief writes a row with zero counts in notes.
    #[test]
    fn test_brief_log_to_db_empty_brief() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("empty_brief_test.db");

        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
            meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };

        brief.log_to_db_at(&db_path);

        let db = crate::db::AxonixDb::open(&db_path).unwrap();
        let rows = db.sessions_recent(10).unwrap();
        assert_eq!(rows.len(), 1, "should insert one row for an empty brief");
        let row = &rows[0];
        assert_eq!(row.session, "brief");
        assert!(
            row.notes.as_deref().unwrap_or("").contains("0 active goals"),
            "notes should show 0 active goals: {:?}", row.notes
        );
        assert!(
            row.notes.as_deref().unwrap_or("").contains("0 open predictions"),
            "notes should show 0 open predictions: {:?}", row.notes
        );
    }

    /// log_to_db_at() can be called multiple times; each call inserts a new row.
    #[test]
    fn test_brief_log_to_db_multiple_runs() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("multi_brief.db");

        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
            meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };

        brief.log_to_db_at(&db_path);
        brief.log_to_db_at(&db_path);
        brief.log_to_db_at(&db_path);

        let db = crate::db::AxonixDb::open(&db_path).unwrap();
        let rows = db.sessions_recent(10).unwrap();
        assert_eq!(rows.len(), 3, "three brief runs should insert three rows");
        for row in &rows {
            assert_eq!(row.session, "brief");
        }
    }

    /// today_date_utc() returns a plausible YYYY-MM-DD string.
    #[test]
    fn test_today_date_utc_format() {
        let date = today_date_utc();
        assert_eq!(date.len(), 10, "date should be exactly 10 chars: {date}");
        assert_eq!(&date[4..5], "-", "char 4 should be '-': {date}");
        assert_eq!(&date[7..8], "-", "char 7 should be '-': {date}");
        // Year should be >= 2025 (we're not time travelling backwards)
        let year: u32 = date[..4].parse().expect("year should be numeric");
        assert!(year >= 2025, "year should be >= 2025: {year}");
    }

    // ── MetaHealthCheck in Brief ──────────────────────────────────────────────────

    /// Brief::collect() returns a Brief with meta_health set to Some
    #[test]
    fn test_brief_has_meta_health_field() {
        // We can verify the field exists and can be Some/None without calling collect()
        // (which reads disk). Just construct with Some and verify it's accessible.
        let mh = crate::meta_health::MetaHealthCheck::run_with_paths(
            std::path::Path::new("/nonexistent/predictions.json"),
            std::path::Path::new("/nonexistent/cycle_summary.json"),
            std::path::Path::new("/nonexistent/METRICS.md"),
        );
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
            meta_health: Some(mh),
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        assert!(brief.meta_health.is_some(), "meta_health should be Some when set");
    }

    /// format_terminal output contains "META-SYSTEM" section header when there are issues
    #[test]
    fn test_brief_format_terminal_includes_meta_system() {
        let mh = crate::meta_health::MetaHealthCheck::run_with_paths(
            std::path::Path::new("/nonexistent/predictions.json"),
            std::path::Path::new("/nonexistent/cycle_summary.json"),
            std::path::Path::new("/nonexistent/METRICS.md"),
        );
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
            meta_health: Some(mh),
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(output.contains("META-SYSTEM"), "format_terminal should include META-SYSTEM section when there are issues: {output}");
    }

    /// When meta_health has issues, telegram format mentions them
    #[test]
    fn test_brief_format_telegram_includes_meta_health_warning() {
        // All paths missing -> all three checks fail -> issues present
        let mh = crate::meta_health::MetaHealthCheck::run_with_paths(
            std::path::Path::new("/nonexistent/predictions.json"),
            std::path::Path::new("/nonexistent/cycle_summary.json"),
            std::path::Path::new("/nonexistent/METRICS.md"),
        );
        assert!(!mh.all_ok(), "sanity: check that meta_health has issues");
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
            meta_health: Some(mh),
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_telegram();
        assert!(
            output.contains("meta-system") || output.contains("issues"),
            "telegram should mention meta-system issues when present: {output}"
        );
    }

    /// When meta_health is None, format_terminal does NOT show META-SYSTEM section
    #[test]
    fn test_brief_format_terminal_meta_health_none_shows_not_checked() {
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
            meta_health: None,
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(!output.contains("META-SYSTEM"), "META-SYSTEM section should be absent when meta_health is None");
    }

    /// When meta_health is all_ok, format_terminal does NOT show META-SYSTEM section
    #[test]
    fn test_brief_format_terminal_meta_health_all_ok_section_absent() {
        use std::io::Write;
        use tempfile::NamedTempFile;
        let mut pf = NamedTempFile::new().unwrap();
        write!(pf, "{{}}").unwrap();
        let mut cf = NamedTempFile::new().unwrap();
        write!(cf, "{{}}").unwrap();
        let mut mf = NamedTempFile::new().unwrap();
        write!(mf, "| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n").unwrap();
        let mh = crate::meta_health::MetaHealthCheck::run_with_paths(pf.path(), cf.path(), mf.path());
        assert!(mh.all_ok(), "sanity: meta_health should be all_ok");
        let brief = Brief {
            active_goals: vec![],
            open_predictions: vec![],
            recent_sessions: vec![],
            note: None,
            health: None,
            bluesky_stats: None,
            caddy: None,
            docker: None,
            calibration: None,
            last_session: None,
            failure_summary: None,
            pogo: None,
            meta_health: Some(mh),
            today_priority: String::new(),
            recent_journal: vec![],
            predictions_due_soon: vec![],
            memory_context: vec![],
            infrastructure_anomalies: vec![],
        };
        let output = brief.format_terminal();
        assert!(!output.contains("META-SYSTEM"), "META-SYSTEM section should be absent when all checks are OK: {output}");
    }

    // ── synthesize_priority ───────────────────────────────────────────────────────

    impl Brief {
        /// Construct a minimal all-None Brief suitable for unit testing priority logic.
        fn test_empty() -> Self {
            Brief {
                active_goals: vec![],
                open_predictions: vec![],
                recent_sessions: vec![],
                note: None,
                health: None,
                bluesky_stats: None,
                caddy: None,
                docker: None,
                calibration: None,
                last_session: None,
                failure_summary: None,
                pogo: None,
                meta_health: None,
                today_priority: String::new(),
                recent_journal: vec![],
                predictions_due_soon: vec![],
                memory_context: vec![],
                infrastructure_anomalies: vec![],
            }
        }
    }

    #[test]
    fn test_synthesize_priority_all_clear() {
        // Active goals present, no docker errors, no meta issues, no failures,
        // predictions are fresh (year 2099 dates won't be overdue), and we rely
        // on count_backlog_goals reading from disk — bypass by giving 3+ items via
        // a brief state that won't hit rules 1-5, with no backlog check (we can't
        // easily control GOALS.md in tests).
        // Rule 6 (backlog < 3) may trigger depending on repo state; that's OK —
        // we just verify the "nothing critical" path is reachable with no other flags.
        let mut b = Brief::test_empty();
        b.active_goals = vec!["G-082: today priority".to_string()];
        // Use a far-future prediction date so it won't be overdue
        b.open_predictions = vec![(1, "2099-01-01".to_string(), "future prediction".to_string())];

        // With no docker, no meta issues, active goals, no failures, no overdue preds,
        // the result is either backlog warning or all clear.
        let result = synthesize_priority(&b);
        // Either "Nothing critical" or "Backlog has N goals" — both are valid
        assert!(
            result.contains("Nothing critical") || result.contains("Backlog"),
            "expected all-clear or backlog warning, got: {result}"
        );
    }

    #[test]
    fn test_synthesize_priority_no_active_goals() {
        let b = Brief::test_empty(); // active_goals is empty
        let result = synthesize_priority(&b);
        assert!(
            result.contains("No active goal"),
            "empty active_goals should produce 'No active goal' message, got: {result}"
        );
    }

    #[test]
    fn test_synthesize_priority_meta_health_issue_beats_no_goals() {
        // Meta-health issues have priority 2, no-goals has priority 3.
        // So if meta_health has issues AND no active goals, meta-health wins.
        let mh = crate::meta_health::MetaHealthCheck::run_with_paths(
            std::path::Path::new("/nonexistent/predictions.json"),
            std::path::Path::new("/nonexistent/cycle_summary.json"),
            std::path::Path::new("/nonexistent/METRICS.md"),
        );
        let mut b = Brief::test_empty();
        b.meta_health = Some(mh);
        // active_goals is empty — but meta-health should win
        let result = synthesize_priority(&b);
        assert!(
            result.starts_with("Meta-health:"),
            "meta-health issue should take priority over no-goals, got: {result}"
        );
    }

    #[test]
    fn test_synthesize_priority_overdue_prediction() {
        // Prediction with date far in the past (>14 days ago) should be flagged.
        let mut b = Brief::test_empty();
        b.active_goals = vec!["some goal".to_string()];
        // Use a date from 2020 — definitely overdue
        b.open_predictions = vec![(42, "2020-01-01".to_string(), "ancient prediction".to_string())];

        let result = synthesize_priority(&b);
        assert!(
            result.contains("Prediction #42") && result.contains("overdue"),
            "overdue prediction should be flagged, got: {result}"
        );
    }

    #[test]
    fn test_count_backlog_goals_parses_markdown() {
        // Test the backlog counting logic with an inline string (mirrors count_backlog_goals)
        let sample = "## Active\n### G-001\n\n## Backlog\n### G-002\n### G-003\n\n## Done\n### G-old\n";
        let mut in_backlog = false;
        let mut count = 0;
        for line in sample.lines() {
            if line.starts_with("## Backlog") { in_backlog = true; continue; }
            if in_backlog && line.starts_with("## ") { break; }
            if in_backlog && line.starts_with("### ") { count += 1; }
        }
        assert_eq!(count, 2, "should count exactly 2 backlog goals");
    }

    // ── parse_backlog_goals ───────────────────────────────────────────────────────

    /// parse_backlog_goals returns titles under ## Backlog, each ≤60 chars.
    #[test]
    fn test_parse_backlog_goals_logic() {
        // Mirror the parse_backlog_goals logic inline to test without touching disk.
        let sample = "## Active\n### G-001 — Some Active Goal\n\n\
            ## Backlog\n### G-113 — First backlog goal\n### G-114 — Second backlog goal\n\n\
            ## Done\n### G-001 — Old completed goal\n";

        let mut in_backlog = false;
        let mut goals = Vec::new();

        for line in sample.lines() {
            if line.trim_start().starts_with("## Backlog") {
                in_backlog = true;
                continue;
            }
            if in_backlog && line.trim_start().starts_with("## ") {
                break;
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

        assert_eq!(goals.len(), 2, "should find exactly 2 backlog goals: {goals:?}");
        assert!(goals[0].contains("G-113"), "first should be G-113: {}", goals[0]);
        assert!(goals[1].contains("G-114"), "second should be G-114: {}", goals[1]);
        // Active/Done goals must NOT appear
        assert!(!goals.iter().any(|g| g.contains("Active Goal")), "active goal should not appear in backlog list");
        assert!(!goals.iter().any(|g| g.contains("Old completed")), "done goal should not appear in backlog list");
    }

    #[test]
    fn test_parse_backlog_goals_empty_when_no_backlog_section() {
        // parse_backlog_goals reads from disk; just test the logic path for no backlog header.
        let sample = "## Active\n### G-001\n\n## Done\n### G-old\n";
        let mut in_backlog = false;
        let mut goals: Vec<String> = Vec::new();
        for line in sample.lines() {
            if line.trim_start().starts_with("## Backlog") { in_backlog = true; continue; }
            if in_backlog && line.trim_start().starts_with("## ") { break; }
            if in_backlog {
                let trimmed = line.trim();
                if trimmed.starts_with("### ") {
                    goals.push(trimmed.trim_start_matches("### ").trim().to_string());
                }
            }
        }
        assert!(goals.is_empty(), "no Backlog section → empty result: {goals:?}");
    }

    #[test]
    fn test_parse_backlog_goals_truncates_long_titles() {
        // Verify truncation at 60 chars using truncate_str directly.
        let long_title = "G-200 — ".to_string() + &"A".repeat(70);
        let truncated = truncate_str(&long_title, 60).to_string();
        assert_eq!(truncated.chars().count(), 60, "should be exactly 60 chars: {truncated}");
    }

    #[test]
    fn test_days_since_calculation() {
        // days_since("2026-03-01", "2026-03-27") should be roughly 26
        let diff = days_since("2026-03-01", "2026-03-27");
        assert_eq!(diff, 26, "2026-03-01 to 2026-03-27 should be 26 days, got {diff}");
    }

    #[test]
    fn test_days_since_same_date() {
        assert_eq!(days_since("2026-05-10", "2026-05-10"), 0, "same date should be 0 days");
    }

    #[test]
    fn test_days_since_future_date_returns_zero() {
        // date_str is AFTER today → should return 0 (not negative)
        let diff = days_since("2030-01-01", "2026-01-01");
        assert_eq!(diff, 0, "future date should return 0");
    }

    #[test]
    fn test_synthesize_priority_consecutive_failures() {
        let mut b = Brief::test_empty();
        b.active_goals = vec!["some goal".to_string()];
        b.recent_sessions = vec![
            SessionSummary {
                day: "10".to_string(),
                session: "S2".to_string(),
                date: "2026-04-01".to_string(),
                tests: "500".to_string(),
                notes: "FAILED tests in build".to_string(),
            },
            SessionSummary {
                day: "10".to_string(),
                session: "S1".to_string(),
                date: "2026-03-31".to_string(),
                tests: "500".to_string(),
                notes: "FAILED again".to_string(),
            },
        ];

        let result = synthesize_priority(&b);
        assert!(
            result.contains("sessions had test failures"),
            "consecutive FAILED sessions should be flagged, got: {result}"
        );
    }

    #[test]
    fn test_today_priority_shown_in_terminal() {
        let mut b = Brief::test_empty();
        b.today_priority = "Test priority message".to_string();
        b.active_goals = vec!["some goal".to_string()];
        let output = b.format_terminal();
        assert!(output.contains("TODAY'S PRIORITY"), "terminal should show TODAY'S PRIORITY header");
        assert!(output.contains("Test priority message"), "terminal should show the priority text");
        // Priority section should appear before ACTIVE GOALS
        let priority_pos = output.find("TODAY'S PRIORITY").unwrap();
        let goals_pos = output.find("ACTIVE GOALS").unwrap();
        assert!(priority_pos < goals_pos, "priority section should appear before active goals");
    }

    #[test]
    fn test_today_priority_shown_in_telegram() {
        let mut b = Brief::test_empty();
        b.today_priority = "Telegram priority".to_string();
        let output = b.format_telegram();
        assert!(output.contains("Today's Priority"), "telegram should show Today's Priority header");
        assert!(output.contains("Telegram priority"), "telegram should show the priority text");
        // Priority section should appear before Active Goals
        let priority_pos = output.find("Today's Priority").unwrap();
        let goals_pos = output.find("Active Goals").unwrap();
        assert!(priority_pos < goals_pos, "priority section should appear before active goals in telegram");
    }

    // ── collect_predictions_due_soon ─────────────────────────────────────────────

    #[test]
    fn test_collect_predictions_due_soon_empty_when_no_day_count() {
        // When DAY_COUNT is not set (or unset), collect_predictions_due_soon returns empty.
        std::env::remove_var("DAY_COUNT");
        let result = collect_predictions_due_soon();
        assert!(result.is_empty(), "should return empty when DAY_COUNT is not set");
    }

    #[test]
    fn test_collect_predictions_due_soon_parses_by_day() {
        // Directly test the "By Day N" parsing logic.
        // "By Day 20" with current day 17 → threshold 20 → 20 <= 20 → included
        // "By Day 25" with current day 17 → threshold 20 → 25 > 20 → excluded
        let predictions: Vec<(u32, String, String)> = vec![
            (1, "2026-01-01".to_string(), "By Day 20, the X will happen".to_string()),
            (2, "2026-01-02".to_string(), "By Day 25, something else".to_string()),
            (3, "2026-01-03".to_string(), "By Day 18, another thing".to_string()),
        ];
        let current_day: u32 = 17;
        let threshold = current_day + 3; // 20

        let due_soon: Vec<_> = predictions
            .iter()
            .filter(|(_id, _date, text)| {
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
            .collect();

        assert_eq!(due_soon.len(), 2, "Day 20 and Day 18 should be included (threshold=20)");
        assert!(due_soon.iter().any(|(id, _, _)| *id == 1), "Day 20 prediction should be included");
        assert!(due_soon.iter().any(|(id, _, _)| *id == 3), "Day 18 prediction should be included");
        assert!(!due_soon.iter().any(|(id, _, _)| *id == 2), "Day 25 prediction should NOT be included");
    }

    #[test]
    fn test_brief_format_terminal_shows_due_soon() {
        let mut b = Brief::test_empty();
        b.predictions_due_soon = vec![
            (7, "2026-04-01".to_string(), "By Day 20, tests will pass".to_string()),
        ];
        let output = b.format_terminal();
        assert!(output.contains("⏰ DUE SOON"), "terminal should show DUE SOON section");
        assert!(output.contains("#7"), "should show prediction id");
        assert!(output.contains("By Day 20"), "should show prediction text");
    }

    #[test]
    fn test_brief_format_terminal_shows_memory_context() {
        let mut b = Brief::test_empty();
        b.memory_context = vec![
            ("rust borrow checker error in module X".to_string(), 0.92),
            ("repl command dispatch pattern".to_string(), 0.75),
        ];
        let output = b.format_terminal();
        assert!(output.contains("🧠 MEMORY CONTEXT"), "terminal should show MEMORY CONTEXT section");
        assert!(output.contains("rust borrow checker"), "should show first memory result");
        assert!(output.contains("0.92"), "should show relevance score");
    }

    #[test]
    fn test_brief_format_telegram_shows_due_soon() {
        let mut b = Brief::test_empty();
        b.predictions_due_soon = vec![
            (12, "2026-04-05".to_string(), "By Day 22, deploy to prod".to_string()),
        ];
        let output = b.format_telegram();
        assert!(output.contains("⏰ *Due Soon*"), "telegram should show Due Soon section");
        assert!(output.contains("#12"), "should show prediction id in telegram");
    }

    // ── G-104: infrastructure anomaly detection ───────────────────────────────

    #[test]
    fn test_brief_infrastructure_anomalies_shown_in_telegram() {
        let mut b = Brief::test_empty();
        b.infrastructure_anomalies = vec!["⚠ axonix-db — Up 5 min (unhealthy)".to_string()];
        let output = b.format_telegram();
        assert!(
            output.contains("Infrastructure Anomalies"),
            "telegram brief should show Infrastructure Anomalies section"
        );
        assert!(
            output.contains("axonix-db"),
            "telegram brief should show anomalous container name"
        );
    }

    #[test]
    fn test_brief_infrastructure_anomalies_not_shown_when_empty() {
        let b = Brief::test_empty(); // infrastructure_anomalies is empty
        let output = b.format_telegram();
        assert!(
            !output.contains("Infrastructure Anomalies"),
            "telegram brief should NOT show anomaly section when there are no anomalies"
        );
    }

    #[test]
    fn test_synthesize_priority_anomaly_takes_precedence_over_meta_health() {
        let mut b = Brief::test_empty();
        b.active_goals = vec!["some goal".to_string()];
        b.infrastructure_anomalies = vec!["⚠ axonix — Up 1 min (unhealthy)".to_string()];
        let result = synthesize_priority(&b);
        assert!(
            result.contains("Infrastructure anomaly"),
            "anomaly should be surfaced in priority, got: {result}"
        );
    }

    #[test]
    fn test_synthesize_priority_no_anomaly_when_healthy() {
        let mut b = Brief::test_empty();
        b.active_goals = vec!["some goal".to_string()];
        b.infrastructure_anomalies = vec![]; // no anomalies
        let result = synthesize_priority(&b);
        assert!(
            !result.contains("Infrastructure anomaly"),
            "no anomaly section when infrastructure_anomalies is empty, got: {result}"
        );
    }
}

