//! Brief::format_terminal() implementation.
//!
//! format_telegram() lives in format_telegram.rs.

use super::types::Brief;
use super::helpers::truncate_str;

impl Brief {
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
                if let Some(n) = ls.test_count {
                    out.push_str(&format!("   🧪 {n} tests\n"));
                }
            }
            None => {
                out.push_str("   (no cycle summary found)\n");
            }
        }
        out.push('\n');

        // Meta-system health — only show if there are warnings (mirrors Telegram filter)
        if let Some(ref mh) = self.meta_health {
            if !mh.all_ok() {
                out.push_str("🔧 META-SYSTEM\n");
                out.push_str(&mh.format_terminal_issues());
                out.push('\n');
                out.push('\n');
            }
        }

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
}
