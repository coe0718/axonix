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
//!
//! Sub-modules:
//! - `types`    — struct definitions (HealthSummary, Brief, SessionSummary, etc.)
//! - `collect`  — Brief::collect() assembly from disk
//! - `format`   — Brief::format_terminal() and Brief::format_telegram()
//! - `db`       — Brief::log_to_db() and Brief::log_to_db_at()
//! - `parsers`  — parse_active_goals, parse_recent_metrics, etc.
//! - `helpers`  — utility functions (today_date_utc, truncate_str, etc.)
//! - `priority` — synthesize_priority()

pub mod types;
pub mod collect;
pub mod format;
pub mod format_telegram;
pub mod db;
pub mod parsers;
pub mod helpers;
pub mod priority;

// Re-export all public types and functions so callers use `brief::Brief`, etc.
pub use types::{Brief, HealthSummary, LastSessionSummary, SessionSummary};
pub use parsers::{
    parse_active_goals,
    parse_backlog_goals,
    parse_recent_metrics,
    parse_journal_entries_from_str,
    parse_recent_journal_entries,
    collect_memory_context,
};

#[cfg(test)]
mod tests;
