//! Struct definitions for the morning brief.

use crate::predictions::CalibrationScore;

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
    /// Infrastructure anomalies: containers in unhealthy/restarting state.
    pub infrastructure_anomalies: Vec<String>,
}

/// One session row from METRICS.md.
pub struct SessionSummary {
    pub day: String,
    pub session: String, // e.g. "S1", "S2"
    pub date: String,
    pub tests: String,
    pub notes: String,
}
