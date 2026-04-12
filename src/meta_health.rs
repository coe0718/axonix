//! Meta-system health check for Axonix (G-070, Issue #83).
//!
//! Verifies that Axonix's own self-improvement infrastructure is working:
//! - predictions.json exists and has been written recently
//! - cycle_summary.json is fresh (updated in the last 2 sessions / 2 days)
//! - METRICS.md has no stale `~?k` placeholder rows
//!
//! Surfaced in the morning brief and via /brief on Telegram.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// Status of one meta-health item.
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// Everything looks good.
    Ok(String),
    /// Something is wrong — message describes the problem.
    Warn(String),
    /// File doesn't exist.
    Missing(String),
}

impl HealthStatus {
    pub fn is_ok(&self) -> bool {
        matches!(self, HealthStatus::Ok(_))
    }
    pub fn message(&self) -> &str {
        match self {
            HealthStatus::Ok(m) | HealthStatus::Warn(m) | HealthStatus::Missing(m) => m,
        }
    }
    pub fn emoji(&self) -> &str {
        match self {
            HealthStatus::Ok(_) => "✓",
            HealthStatus::Warn(_) => "⚠",
            HealthStatus::Missing(_) => "✗",
        }
    }
}

/// Result of a complete meta-health check.
#[derive(Debug, Clone)]
pub struct MetaHealthCheck {
    pub predictions: HealthStatus,
    pub cycle_summary: HealthStatus,
    pub metrics_clean: HealthStatus,
}

impl MetaHealthCheck {
    /// Run all checks. Paths use defaults (workspace root and .axonix/).
    pub fn run() -> Self {
        Self::run_with_paths(
            Path::new(".axonix/predictions.json"),
            Path::new(".axonix/cycle_summary.json"),
            Path::new("METRICS.md"),
        )
    }

    /// Run checks with explicit paths (for testing).
    pub fn run_with_paths(
        predictions_path: &Path,
        cycle_summary_path: &Path,
        metrics_path: &Path,
    ) -> Self {
        MetaHealthCheck {
            predictions: check_file_freshness(predictions_path, Duration::from_secs(7 * 24 * 3600), "predictions.json"),
            cycle_summary: check_file_freshness(cycle_summary_path, Duration::from_secs(2 * 24 * 3600), "cycle_summary.json"),
            metrics_clean: check_metrics_stale(metrics_path),
        }
    }

    /// Returns true if all checks passed.
    pub fn all_ok(&self) -> bool {
        self.predictions.is_ok() && self.cycle_summary.is_ok() && self.metrics_clean.is_ok()
    }

    /// Returns a list of warning/error messages (excludes OK items).
    pub fn issues(&self) -> Vec<String> {
        let mut out = Vec::new();
        for status in [&self.predictions, &self.cycle_summary, &self.metrics_clean] {
            if !status.is_ok() {
                out.push(format!("{} {}", status.emoji(), status.message()));
            }
        }
        out
    }

    /// Format for terminal display (1-3 lines).
    pub fn format_terminal(&self) -> String {
        let checks = [&self.predictions, &self.cycle_summary, &self.metrics_clean];
        checks.iter()
            .map(|s| format!("   {} {}", s.emoji(), s.message()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Format for terminal display — only shows non-OK checks.
    pub fn format_terminal_issues(&self) -> String {
        let checks = [&self.predictions, &self.cycle_summary, &self.metrics_clean];
        let issues: Vec<String> = checks.iter()
            .filter(|s| !s.is_ok())
            .map(|s| format!("   {} {}", s.emoji(), s.message()))
            .collect();
        issues.join("\n")
    }

    /// Format compact for Telegram (emojis only + issues).
    pub fn format_telegram(&self) -> String {
        if self.all_ok() {
            "✓ meta-system: ok".to_string()
        } else {
            let issues = self.issues();
            format!("⚠ meta-system issues:\n{}", issues.join("\n"))
        }
    }
}

/// Check if a file exists and was modified within `max_age`.
fn check_file_freshness(path: &Path, max_age: Duration, label: &str) -> HealthStatus {
    if !path.exists() {
        return HealthStatus::Missing(format!("{label}: not found"));
    }
    match path.metadata().and_then(|m| m.modified()) {
        Err(_) => HealthStatus::Warn(format!("{label}: exists but cannot read mtime")),
        Ok(modified) => {
            let age = SystemTime::now()
                .duration_since(modified)
                .unwrap_or(Duration::from_secs(u64::MAX));
            if age > max_age {
                let days = age.as_secs() / 86400;
                HealthStatus::Warn(format!("{label}: last updated {days} days ago (stale)"))
            } else {
                let hours = age.as_secs() / 3600;
                HealthStatus::Ok(format!("{label}: updated {hours}h ago"))
            }
        }
    }
}

/// Check METRICS.md for stale `~?k` token rows.
fn check_metrics_stale(path: &Path) -> HealthStatus {
    if !path.exists() {
        return HealthStatus::Missing("METRICS.md: not found".to_string());
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HealthStatus::Warn("METRICS.md: cannot read".to_string()),
    };
    let stale_count = content.lines()
        .filter(|line| line.contains("| ~?k |") || line.contains("|~?k|") || line.contains("| ~?k|") || line.contains("|~?k |"))
        .count();
    if stale_count > 0 {
        HealthStatus::Warn(format!("METRICS.md: {stale_count} row(s) with stale ~?k tokens"))
    } else {
        HealthStatus::Ok("METRICS.md: no stale rows".to_string())
    }
}

#[cfg(test)]
#[path = "meta_health_tests.rs"]
mod tests;
