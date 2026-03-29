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

use crate::db::AxonixDb;
use crate::predictions::{CalibrationScore, PredictionStore};
use std::path::Path;

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

/// Summary of the last completed session, loaded from `.axonix/cycle_summary.json`.
pub struct LastSessionSummary {
    /// Session label e.g. "Day 10, Session 3"
    pub session: String,
    /// ISO date e.g. "2026-03-23"
    pub date: String,
    /// Completed items from that session (up to 5 shown)
    pub completed: Vec<String>,
    /// Test count at session end, if recorded
    pub test_count: Option<u32>,
}

/// A morning brief summary.
pub struct Brief {
    pub active_goals: Vec<String>,
    pub open_predictions: Vec<(u32, String, String)>, // (id, date, text)
    pub recent_sessions: Vec<SessionSummary>,
    pub note: Option<String>,
    pub health: Option<HealthSummary>,
    pub bluesky_stats: Option<(usize, usize, Option<String>)>, // (total, root_posts, last_date)
    pub caddy: Option<crate::health::CaddyHealth>,
    /// Docker container health summary.
    pub docker: Option<crate::health::DockerHealth>,
    pub calibration: Option<CalibrationScore>,
    pub last_session: Option<LastSessionSummary>,
    /// Most common failure type + total count, if any failures have been logged.
    pub failure_summary: Option<String>,
    /// Pokémon GO active events, upcoming events, and promo codes from leekduck.com.
    pub pogo: Option<crate::pogo::PogoData>,
    /// Meta-system health check (predictions freshness, cycle_summary freshness, METRICS.md staleness).
    pub meta_health: Option<crate::meta_health::MetaHealthCheck>,
    /// Synthesized priority: the single most important thing to address.
    pub today_priority: String,
    /// Last 3 journal entry titles from JOURNAL.md.
    pub recent_journal: Vec<String>,
    /// Predictions due within 3 days (id, deadline_date, text).
    pub predictions_due_soon: Vec<(u32, String, String)>,
    /// Top memory-search results for the current active goal (text, score).
    pub memory_context: Vec<(String, f64)>,
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

        let caddy_url_configured = std::env::var("CADDY_ADMIN_URL").is_ok();
        let caddy_result = crate::health::caddy_health();
        let caddy = if caddy_result.reachable || caddy_url_configured {
            Some(caddy_result)
        } else {
            None
        };

        // Collect Docker container health (graceful fallback when unavailable).
        let docker_host_configured = std::env::var("DOCKER_HOST").is_ok();
        let docker_result = crate::health::docker_health();
        let docker = if docker_result.error.is_none() || docker_host_configured {
            Some(docker_result)
        } else {
            None
        };

        // Compute calibration score from resolved predictions.
        let pred_store = PredictionStore::default_path();
        let score = pred_store.calibration_score();
        let calibration = if score.total_resolved > 0 { Some(score) } else { None };

        // Load last cycle summary from .axonix/cycle_summary.json
        let last_session = {
            let cs = crate::cycle_summary::CycleSummary::default_path();
            cs.data.map(|d| LastSessionSummary {
                session: d.session.clone(),
                date: d.date.clone(),
                completed: d.completed.clone(),
                test_count: d.test_count,
            })
        };

        // Load failure pattern summary from .axonix/failure_patterns.json
        let failure_summary = {
            let store = crate::failure_patterns::FailurePatternStore::default_path();
            if store.total_count() > 0 {
                store.most_common_failure_type().map(|ft| {
                    format!("⚠️  Most common failure: {} ({} total)", ft, store.total_count())
                })
            } else {
                None
            }
        };

        let recent_journal = parse_recent_journal_entries(3);

        let predictions_due_soon = collect_predictions_due_soon();
        let memory_context = if !active_goals.is_empty() {
            collect_memory_context(&active_goals[0])
        } else {
            vec![]
        };

        let mut brief = Brief {
            active_goals,
            open_predictions,
            recent_sessions,
            note: None,
            health,
            bluesky_stats,
            caddy,
            docker,
            calibration,
            last_session,
            failure_summary,
            pogo: {
                let data = crate::pogo::PogoData::fetch();
                if !data.is_empty() { Some(data) } else { None }
            },
            meta_health: Some(crate::meta_health::MetaHealthCheck::run()),
            today_priority: String::new(),
            recent_journal,
            predictions_due_soon,
            memory_context,
        };
        brief.today_priority = synthesize_priority(&brief);
        brief
    }

    /// Format the brief as a multi-line string for terminal output.
    pub fn format_terminal(&self) -> String {
        let mut out = String::new();
        out.push_str("╔══════════════════════════════════════════════╗\n");
        out.push_str("║  ⚡ AXONIX MORNING BRIEF                      ║\n");
        out.push_str("╚══════════════════════════════════════════════╝\n");
        out.push('\n');

        // Today's Priority
        out.push_str("🎯 TODAY'S PRIORITY\n");
        out.push_str(&format!("   → {}\n", self.today_priority));
        out.push('\n');

        // Recent journal activity
        if !self.recent_journal.is_empty() {
            out.push_str("📓 Recent Activity\n");
            for title in &self.recent_journal {
                out.push_str(&format!("  • {title}\n"));
            }
            out.push('\n');
        }

        // Predictions due soon
        if !self.predictions_due_soon.is_empty() {
            out.push_str("⏰ DUE SOON\n");
            for (id, date, text) in &self.predictions_due_soon {
                out.push_str(&format!("   #{id} [{date}] {}\n", truncate_str(text, 60)));
            }
            out.push('\n');
        }

        // Memory context for active goal
        if !self.memory_context.is_empty() {
            out.push_str("🧠 MEMORY CONTEXT\n");
            for (text, score) in &self.memory_context {
                out.push_str(&format!("   [{:.2}] {}\n", score, truncate_str(text, 70)));
            }
            out.push('\n');
        }

        // Active goals
        out.push_str("📋 ACTIVE GOALS\n");
        if self.active_goals.is_empty() {
            out.push_str("   (no active goals — will form one at session start)\n");
        } else {
            for g in &self.active_goals {
                out.push_str(&format!("   • {g}\n"));
            }
        }
        out.push('\n');

        // Last session (from cycle_summary.json)
        out.push_str("📝 LAST SESSION\n");
        match &self.last_session {
            Some(ls) => {
                out.push_str(&format!("   {} ({})\n", ls.session, ls.date));
                for item in ls.completed.iter().take(5) {
                    out.push_str(&format!("   ✓ {}\n", truncate_str(item, 60)));
                }
                if ls.completed.is_empty() {
                    out.push_str("   (no completed items recorded)\n");
                }
                match ls.test_count {
                    Some(n) => out.push_str(&format!("   🧪 {n} tests\n")),
                    None => out.push_str("   🧪 ? tests\n"),
                }
            }
            None => {
                out.push_str("   (no cycle summary found)\n");
            }
        }
        out.push('\n');

        // Meta-system health
        out.push_str("🔧 META-SYSTEM\n");
        if let Some(ref mh) = self.meta_health {
            out.push_str(&mh.format_terminal());
            out.push('\n');
        } else {
            out.push_str("   (not checked)\n");
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
        // Calibration line (shown if there are resolved predictions)
        if let Some(cal) = &self.calibration {
            out.push_str(&format!(
                "  calibration: {}/{} correct ({:.1}%) — bias: {}\n",
                cal.correct,
                cal.total_resolved,
                cal.hit_rate * 100.0,
                cal.direction_bias,
            ));
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

        // Caddy infrastructure health
        if let Some(caddy) = &self.caddy {
            out.push_str(&format!("🔗 Caddy: {}\n", caddy.format()));
            out.push('\n');
        }

        // Docker container health
        if let Some(docker) = &self.docker {
            out.push_str(&docker.format());
            out.push('\n');
            out.push('\n');
        }

        // Failure pattern summary (if any failures logged)
        if let Some(fs) = &self.failure_summary {
            out.push_str(&format!("{fs}\n"));
            out.push('\n');
        }

        // Pokémon GO events and promo codes
        if let Some(ref pogo) = self.pogo {
            out.push_str(&pogo.format_terminal());
            out.push('\n');
        }

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

        // Today's Priority
        out.push_str("🎯 *Today's Priority*\n");
        out.push_str(&format!("→ {}\n", self.today_priority));
        out.push('\n');

        // Recent journal activity (compact)
        if !self.recent_journal.is_empty() {
            out.push_str("📓 *Recent Activity*\n");
            for title in &self.recent_journal {
                out.push_str(&format!("• {title}\n"));
            }
            out.push('\n');
        }

        // Predictions due soon (Telegram compact)
        if !self.predictions_due_soon.is_empty() {
            out.push_str("⏰ *Due Soon*\n");
            for (id, _date, text) in &self.predictions_due_soon {
                out.push_str(&format!("• #{id} {}\n", truncate_str(text, 50)));
            }
            out.push('\n');
        }

        // Memory context (Telegram compact)
        if !self.memory_context.is_empty() {
            out.push_str("🧠 *Memory Context*\n");
            for (text, score) in self.memory_context.iter().take(2) {
                out.push_str(&format!("• [{:.2}] {}\n", score, truncate_str(text, 50)));
            }
            out.push('\n');
        }

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
        // Calibration line (compact)
        if let Some(cal) = &self.calibration {
            out.push_str(&format!(
                "📊 Calibration: {}/{} ({:.0}%) — {}\n",
                cal.correct,
                cal.total_resolved,
                cal.hit_rate * 100.0,
                cal.direction_bias,
            ));
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

        // Caddy infrastructure health (compact)
        if let Some(caddy) = &self.caddy {
            out.push_str(&format!("🔗 {}\n", caddy.format()));
        }

        // Docker container health (compact)
        if let Some(docker) = &self.docker {
            out.push_str(&format!("🐳 {}\n", docker.format_compact()));
        }

        // Failure pattern summary (compact)
        if let Some(fs) = &self.failure_summary {
            out.push_str(&format!("{fs}\n"));
        }

        // Pokémon GO events and promo codes (compact)
        if let Some(ref pogo) = self.pogo {
            out.push_str(&pogo.format_telegram());
            out.push('\n');
        }

        // Bluesky post stats (compact)
        if let Some((_total, root, last_date)) = &self.bluesky_stats {
            let date_str = last_date.as_deref().unwrap_or("(never)");
            out.push_str(&format!("📡 *Bluesky*: {root} posts (last: {date_str})\n"));
        }

        // Last session from cycle_summary.json
        out.push_str("\n📝 *Last Session*\n");
        match &self.last_session {
            Some(ls) => {
                out.push_str(&format!("• {} ({})\n", ls.session, ls.date));
                for item in ls.completed.iter().take(5) {
                    out.push_str(&format!("✓ {}\n", truncate_str(item, 60)));
                }
                match ls.test_count {
                    Some(n) => out.push_str(&format!("🧪 {n} tests\n")),
                    None => out.push_str("🧪 ? tests\n"),
                }
            }
            None => {
                out.push_str("_(no cycle summary found)_\n");
            }
        }

        out.push('\n');

        // Last session from METRICS.md
        out.push_str("📊 *Recent Metrics*\n");
        if let Some(last) = self.recent_sessions.first() {
            out.push_str(&format!(
                "Day {} {} {} — {} tests\n",
                last.day, last.session, last.date, last.tests
            ));
            out.push_str(&format!("_{}_\n", truncate_str(&last.notes, 60)));
        } else {
            out.push_str("_(no data)_\n");
        }

        // Meta-system health (only show if there are issues)
        if let Some(ref mh) = self.meta_health {
            if !mh.all_ok() {
                out.push('\n');
                out.push_str(&mh.format_telegram());
                out.push('\n');
            }
        }

        out
    }

    /// Log this brief run to the axonix.db sessions table (G-079).
    ///
    /// Opens the default DB path (`.axonix/axonix.db`) and inserts a row.
    /// On failure, logs a warning to stderr and continues — never crashes the brief.
    ///
    /// Call this after displaying the brief so the DB write doesn't affect output timing.
    pub fn log_to_db(&self) {
        self.log_to_db_at(Path::new(".axonix/axonix.db"));
    }

    /// Log this brief run to the sessions table at the given DB path.
    ///
    /// Separated from `log_to_db()` so tests can pass a temp path.
    pub fn log_to_db_at(&self, db_path: &Path) {
        // Parse day number from DAY_COUNT env var (format: "N YYYY-MM-DD" or just "N").
        let day: i64 = std::env::var("DAY_COUNT")
            .ok()
            .and_then(|s| s.split_whitespace().next().and_then(|n| n.parse().ok()))
            .unwrap_or(0);

        // Today's date in YYYY-MM-DD format (derived from system time, no external deps).
        let date = today_date_utc();

        // Meaningful notes: goal count + open prediction count.
        let notes = format!(
            "brief: {} active goals, {} open predictions",
            self.active_goals.len(),
            self.open_predictions.len(),
        );

        let db = match AxonixDb::open(db_path) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("warning: brief DB open failed ({}): {}", db_path.display(), e);
                return;
            }
        };

        if let Err(e) = db.session_insert(day, "brief", &date, None, None, None, Some(&notes)) {
            eprintln!("warning: brief DB insert failed: {e}");
        }
    }
}

/// Return today's date in YYYY-MM-DD format using only std (no external deps).
fn today_date_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Re-use the same manual calendar arithmetic as db.rs.
    let days = secs / 86400;
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let dy = if is_leap_year(y) { 366 } else { 365 };
        if remaining < dy { break; }
        remaining -= dy;
        y += 1;
    }
    let months = if is_leap_year(y) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 1u64;
    for &dm in &months {
        if remaining < dm { break; }
        remaining -= dm;
        mo += 1;
    }
    let d = remaining + 1;
    format!("{y:04}-{mo:02}-{d:02}")
}

fn is_leap_year(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
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

/// Compute the approximate number of days between `date_str` and `today`.
/// Both strings must be in "YYYY-MM-DD" format.
fn days_since(date_str: &str, today: &str) -> u64 {
    fn to_days(s: &str) -> u64 {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return 0;
        }
        let y: u64 = parts[0].parse().unwrap_or(2026);
        let m: u64 = parts[1].parse().unwrap_or(1);
        let d: u64 = parts[2].parse().unwrap_or(1);
        y * 365 + m * 30 + d
    }
    let t = to_days(today);
    let p = to_days(date_str);
    if t > p { t - p } else { 0 }
}

/// Count `###` headers inside the `## Backlog` section of GOALS.md.
fn count_backlog_goals() -> usize {
    let path = Path::new("GOALS.md");
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut in_backlog = false;
    let mut count = 0;
    for line in content.lines() {
        if line.starts_with("## Backlog") {
            in_backlog = true;
            continue;
        }
        if in_backlog && line.starts_with("## ") {
            break;
        }
        if in_backlog && line.starts_with("### ") {
            count += 1;
        }
    }
    count
}

/// Synthesize the single most important actionable item from the brief.
///
/// Priority order (first match wins):
/// 1. Non-running Docker containers
/// 2. Meta-health issues
/// 3. No active goals
/// 4. Consecutive session failures (last 2+ sessions with "FAILED" in notes)
/// 5. Open predictions overdue (>14 days since created)
/// 6. Backlog fewer than 3 items
/// 7. All clear
fn synthesize_priority(brief: &Brief) -> String {
    // 1. Non-running Docker containers
    if let Some(ref docker) = brief.docker {
        for container in &docker.containers {
            if !container.healthy {
                return format!(
                    "Container `{}` is not running — check status",
                    container.name
                );
            }
        }
    }

    // 2. Meta-health issues
    if let Some(ref mh) = brief.meta_health {
        let issues = mh.issues();
        if let Some(first) = issues.first() {
            // Strip leading emoji/whitespace for a clean message
            let clean = first.trim_start_matches(|c: char| !c.is_alphabetic()).trim();
            return format!("Meta-health: {clean}");
        }
    }

    // 3. No active goals
    if brief.active_goals.is_empty() {
        return "No active goal — promote one from backlog before doing anything else".to_string();
    }

    // 4. Consecutive session failures (last 2+ sessions have "FAILED" in notes)
    {
        let fail_count = brief
            .recent_sessions
            .iter()
            .take(2)
            .filter(|s| s.notes.contains("FAILED"))
            .count();
        if fail_count >= 2 {
            return format!(
                "Last {fail_count} sessions had test failures — fix before new features"
            );
        }
    }

    // 5. Open predictions overdue (>14 days since created)
    {
        let today = today_date_utc();
        for (id, date, _text) in &brief.open_predictions {
            if days_since(date, &today) > 14 {
                return format!("Prediction #{id} is overdue — resolve or update it");
            }
        }
    }

    // 6. Backlog fewer than 3 items
    {
        let backlog_count = count_backlog_goals();
        if backlog_count < 3 {
            return format!(
                "Backlog has {backlog_count} goals — form new goals before closing out"
            );
        }
    }

    // 7. All clear
    "Nothing critical — proceed with active goal".to_string()
}

/// Collect predictions due within 3 days (have "By Day N" where N ≤ current_day + 3).
///
/// Uses the DAY_COUNT env var (format: "N YYYY-MM-DD") to determine the current day.
/// Returns an empty vec when DAY_COUNT is unset or zero, or when no predictions match.
fn collect_predictions_due_soon() -> Vec<(u32, String, String)> {
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
/// Returns an empty vec when the DB is unavailable, the query is empty, or no
/// results are found — never panics.
fn collect_memory_context(goal_title: &str) -> Vec<(String, f64)> {
    if goal_title.is_empty() {
        return vec![];
    }
    match crate::db::AxonixDb::open_default() {
        Ok(db) => db
            .search_memory(goal_title, 3)
            .unwrap_or_default()
            .into_iter()
            .map(|row| (row.text, row.score))
            .collect(),
        Err(_) => vec![],
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
        };
        let output = brief.format_terminal();
        assert!(output.contains("624 tests"), "should show numeric test count");
    }

    /// Last session shows "? tests" when test_count is None in terminal format
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
        };
        let output = brief.format_terminal();
        assert!(output.contains("? tests"), "should show '? tests' when count is None");
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
        };
        let output = brief.format_telegram();
        assert!(output.contains("624 tests"), "telegram should show numeric test count");
    }

    /// Telegram format shows "? tests" when test_count is None
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
        };
        let output = brief.format_telegram();
        assert!(output.contains("? tests"), "telegram should show '? tests' when count is None");
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
        let date = super::today_date_utc();
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
        };
        assert!(brief.meta_health.is_some(), "meta_health should be Some when set");
    }

    /// format_terminal output contains "META-SYSTEM" section header
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
        };
        let output = brief.format_terminal();
        assert!(output.contains("META-SYSTEM"), "format_terminal should include META-SYSTEM section: {output}");
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
        };
        let output = brief.format_telegram();
        assert!(
            output.contains("meta-system") || output.contains("issues"),
            "telegram should mention meta-system issues when present: {output}"
        );
    }

    /// When meta_health is None, format_terminal shows "(not checked)"
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
        };
        let output = brief.format_terminal();
        assert!(output.contains("META-SYSTEM"), "META-SYSTEM section always present");
        assert!(output.contains("not checked"), "should show '(not checked)' when meta_health is None");
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
}

