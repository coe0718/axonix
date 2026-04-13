//! Private check functions for MetaHealthCheck — split from meta_health.rs.

use std::path::Path;
use std::time::{Duration, SystemTime};
use super::HealthStatus;

/// Check if a file exists and was modified within `max_age`.
pub(super) fn check_file_freshness(path: &Path, max_age: Duration, label: &str) -> HealthStatus {
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
pub(super) fn check_metrics_stale(path: &Path) -> HealthStatus {
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
