//! Data collection: from_real_data — builds a CycleSummaryData from git/GOALS.md.

use super::types::{CycleSummary, CycleSummaryData};

impl CycleSummary {
    /// Build a CycleSummary from real data: recent git commits, changed files,
    /// and active goals from GOALS.md. Used by `--write-summary` CLI flag (G-035).
    pub fn from_real_data(label: &str) -> CycleSummaryData {
        // Get current date via `date` command (no chrono dependency needed)
        let date = std::process::Command::new("date")
            .arg("+%Y-%m-%d")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string());

        // Recent git commit subjects (last 10)
        let completed: Vec<String> = std::process::Command::new("git")
            .args(["log", "--format=%s", "-10"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| {
                s.lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // Changed files in recent history (HEAD~5..HEAD)
        let changed_files: Vec<String> = std::process::Command::new("git")
            .args(["diff", "--name-only", "HEAD~5", "HEAD"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| {
                s.lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // Active (unchecked) goals from GOALS.md
        let pending: Vec<String> = std::fs::read_to_string("GOALS.md")
            .unwrap_or_default()
            .lines()
            .filter(|l| l.trim_start().starts_with("- [ ]"))
            .map(|l| {
                l.trim()
                    .trim_start_matches("- [ ]")
                    .trim()
                    .to_string()
            })
            .filter(|l| !l.is_empty())
            .collect();

        CycleSummaryData {
            session: label.to_string(),
            date,
            completed,
            changed_files,
            pending,
            learnings: Vec::new(),
            test_count: None,
        }
    }
}
