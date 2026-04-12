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
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_temp_file(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        f
    }

    #[test]
    fn test_health_status_is_ok() {
        assert!(HealthStatus::Ok("good".to_string()).is_ok());
        assert!(!HealthStatus::Warn("warn".to_string()).is_ok());
        assert!(!HealthStatus::Missing("gone".to_string()).is_ok());
    }

    #[test]
    fn test_health_status_emoji() {
        assert_eq!(HealthStatus::Ok("".to_string()).emoji(), "✓");
        assert_eq!(HealthStatus::Warn("".to_string()).emoji(), "⚠");
        assert_eq!(HealthStatus::Missing("".to_string()).emoji(), "✗");
    }

    #[test]
    fn test_health_status_message() {
        assert_eq!(HealthStatus::Ok("all good".to_string()).message(), "all good");
        assert_eq!(HealthStatus::Warn("stale".to_string()).message(), "stale");
        assert_eq!(HealthStatus::Missing("gone".to_string()).message(), "gone");
    }

    #[test]
    fn test_check_file_freshness_missing() {
        let status = check_file_freshness(Path::new("/nonexistent/path.json"), Duration::from_secs(3600), "test.json");
        assert!(matches!(status, HealthStatus::Missing(_)));
        assert!(status.message().contains("not found"));
    }

    #[test]
    fn test_check_file_freshness_fresh_file() {
        // A file just created should be fresh
        let f = make_temp_file("{}");
        let status = check_file_freshness(f.path(), Duration::from_secs(3600), "test.json");
        assert!(matches!(status, HealthStatus::Ok(_)), "fresh file should be Ok: {:?}", status.message());
    }

    #[test]
    fn test_check_metrics_stale_no_stale() {
        let f = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | notes |\n");
        let status = check_metrics_stale(f.path());
        assert!(matches!(status, HealthStatus::Ok(_)), "no stale rows: {:?}", status.message());
    }

    #[test]
    fn test_check_metrics_stale_with_stale() {
        let f = make_temp_file("| 11 | S2 | 2026-03-24 | ~?k | 700 | 0 | ? | ? | ? | yes | in progress |\n");
        let status = check_metrics_stale(f.path());
        assert!(matches!(status, HealthStatus::Warn(_)), "stale row should Warn: {:?}", status.message());
        assert!(status.message().contains("1 row(s)"));
    }

    #[test]
    fn test_check_metrics_stale_multiple_stale() {
        let content = "| 11 | S2 | 2026-03-24 | ~?k | 700 | 0 | ? | ? | ? | yes | x |\n\
                       | 11 | S1 | 2026-03-24 | ~?k | 672 | 0 | ? | ? | ? | yes | y |\n";
        let f = make_temp_file(content);
        let status = check_metrics_stale(f.path());
        assert!(matches!(status, HealthStatus::Warn(_)));
        assert!(status.message().contains("2 row(s)"));
    }

    #[test]
    fn test_check_metrics_missing() {
        let status = check_metrics_stale(Path::new("/nonexistent/METRICS.md"));
        assert!(matches!(status, HealthStatus::Missing(_)));
    }

    #[test]
    fn test_meta_health_check_all_ok() {
        let preds = make_temp_file(r#"{"1": {"prediction": "test", "created": "2026-03-24", "outcome": null, "delta": null, "resolved": null}}"#);
        let cycle = make_temp_file(r#"{"session": "Day 11 S1", "date": "2026-03-24", "completed": [], "pending": [], "changed_files": [], "test_count": 700}"#);
        let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
        let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
        assert!(check.all_ok(), "all fresh files should be ok: {:?}", check.issues());
    }

    #[test]
    fn test_meta_health_check_missing_predictions() {
        let cycle = make_temp_file("{}");
        let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
        let check = MetaHealthCheck::run_with_paths(
            Path::new("/nonexistent/predictions.json"),
            cycle.path(),
            metrics.path(),
        );
        assert!(!check.all_ok());
        assert!(!check.predictions.is_ok());
        let issues = check.issues();
        assert!(!issues.is_empty());
        assert!(issues[0].contains("✗"));
    }

    #[test]
    fn test_meta_health_check_stale_metrics() {
        let preds = make_temp_file("{}");
        let cycle = make_temp_file("{}");
        let metrics = make_temp_file("| 11 | S2 | 2026-03-24 | ~?k | 700 | 0 | ? | ? | ? | yes | in progress |\n");
        let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
        assert!(!check.metrics_clean.is_ok());
        let issues = check.issues();
        assert!(issues.iter().any(|i| i.contains("METRICS.md")));
    }

    #[test]
    fn test_format_terminal_all_ok() {
        let preds = make_temp_file("{}");
        let cycle = make_temp_file("{}");
        let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
        let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
        let out = check.format_terminal();
        assert!(out.contains("✓"), "should show checkmark when ok: {out}");
    }

    #[test]
    fn test_format_telegram_all_ok() {
        let preds = make_temp_file("{}");
        let cycle = make_temp_file("{}");
        let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
        let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
        let msg = check.format_telegram();
        assert!(msg.contains("ok"), "all ok telegram: {msg}");
    }

    #[test]
    fn test_format_telegram_with_issues() {
        let check = MetaHealthCheck::run_with_paths(
            Path::new("/nonexistent/predictions.json"),
            Path::new("/nonexistent/cycle_summary.json"),
            Path::new("/nonexistent/METRICS.md"),
        );
        let msg = check.format_telegram();
        assert!(msg.contains("issues"), "should mention issues: {msg}");
    }

    #[test]
    fn test_issues_empty_when_all_ok() {
        let preds = make_temp_file("{}");
        let cycle = make_temp_file("{}");
        let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
        let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
        assert!(check.issues().is_empty(), "no issues when all ok");
    }

    #[test]
    fn test_issues_lists_all_problems() {
        let check = MetaHealthCheck::run_with_paths(
            Path::new("/nonexistent/predictions.json"),
            Path::new("/nonexistent/cycle_summary.json"),
            Path::new("/nonexistent/METRICS.md"),
        );
        assert_eq!(check.issues().len(), 3, "should report 3 issues when all missing");
    }
}
