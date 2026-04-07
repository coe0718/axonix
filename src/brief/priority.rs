//! Priority synthesis for the morning brief.

use super::types::Brief;
use super::helpers::{today_date_utc, days_since, count_backlog_goals};

/// Synthesize the single most important actionable item from the brief.
///
/// Priority order (first match wins):
/// 1. Non-running Docker containers
/// 1b. Running containers with health anomalies (unhealthy check / restarting)
/// 2. Meta-health issues
/// 3. No active goals
/// 4. Consecutive session failures (last 2+ sessions with "FAILED" in notes)
/// 5. Open predictions overdue (>14 days since created)
/// 6. Backlog fewer than 3 items
/// 7. All clear
pub(crate) fn synthesize_priority(brief: &Brief) -> String {
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

    // 1b. Running containers with health anomalies (unhealthy health check or restarting)
    if !brief.infrastructure_anomalies.is_empty() {
        return format!("⚠ Infrastructure anomaly: {}", brief.infrastructure_anomalies[0]);
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
