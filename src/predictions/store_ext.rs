//! Extension methods on `PredictionStore` — calibration and formatting.
//!
//! Extracted from `store.rs` to keep that file under 300 lines (G-166, Issue #110).

use super::types::{CalibrationScore, Prediction, parse_days_from_delta};
use super::helpers::extract_goal_ids;
use super::store::PredictionStore;

impl PredictionStore {
    /// Compute a calibration score across all resolved predictions.
    ///
    /// Returns a `CalibrationScore` with hit rate, avg_days_early, and direction_bias.
    /// If there are no resolved predictions, returns a zero-state score.
    pub fn calibration_score(&self) -> CalibrationScore {
        let resolved = self.resolved();
        let total_resolved = resolved.len();

        if total_resolved == 0 {
            return CalibrationScore {
                total_resolved: 0,
                correct: 0,
                hit_rate: 0.0,
                avg_days_early: 0.0,
                direction_bias: String::new(),
            };
        }

        let correct = resolved
            .iter()
            .filter(|(_, p)| {
                p.outcome.as_deref().map(|o| o.contains("TRUE")).unwrap_or(false)
            })
            .count();

        let hit_rate = correct as f64 / total_resolved as f64;

        // Parse avg_days_early from delta fields.
        // Looks for patterns like "N day(s) early" or "N day(s) late".
        let mut delta_sum: f64 = 0.0;
        let mut delta_count: usize = 0;

        for (_, pred) in &resolved {
            if let Some(delta_text) = &pred.delta {
                if let Some(days) = parse_days_from_delta(delta_text) {
                    delta_sum += days;
                    delta_count += 1;
                }
            }
        }

        let avg_days_early = if delta_count > 0 {
            delta_sum / delta_count as f64
        } else {
            0.0
        };

        let direction_bias = if hit_rate > 0.7 && avg_days_early > 0.5 {
            "optimistic".to_string()
        } else if hit_rate < 0.4 {
            "pessimistic".to_string()
        } else {
            "calibrated".to_string()
        };

        CalibrationScore {
            total_resolved,
            correct,
            hit_rate,
            avg_days_early,
            direction_bias,
        }
    }

    /// Format calibration data for injection into a system prompt.
    ///
    /// Returns `None` if there are no resolved predictions.
    pub fn format_calibration_for_system_prompt(&self) -> Option<String> {
        let score = self.calibration_score();
        score.format_for_system_prompt()
    }

    /// Auto-resolve predictions that mention a goal ID (e.g. "G-120") when that goal
    /// is found as completed in a goals file (GOALS_ARCHIVE.md or GOALS.md).
    ///
    /// Returns a list of (id, prediction_text) for each newly resolved prediction.
    pub fn auto_resolve_from_goals(&mut self, goals_archive_content: &str) -> Vec<(u32, String)> {
        // Collect open predictions and their IDs up front (avoid borrow conflict)
        let open_list: Vec<(u32, Prediction)> = self
            .open()
            .into_iter()
            .map(|(id, p)| (id, p.clone()))
            .collect();

        let mut resolved = Vec::new();

        for (id, pred) in open_list {
            let goal_ids = extract_goal_ids(&pred.prediction);
            for goal_id in &goal_ids {
                let marker_bracket = format!("[x] [{goal_id}]");
                if goals_archive_content.contains(&marker_bracket) {
                    let outcome = format!(
                        "TRUE. {} was completed (found as [x] in goals archive).",
                        goal_id
                    );
                    let delta = format!("Goal {} verified complete in GOALS_ARCHIVE.md.", goal_id);
                    if self.resolve(id, &outcome, Some(&delta)).is_ok() {
                        resolved.push((id, pred.prediction.clone()));
                    }
                    break; // one goal match is enough
                }
            }
        }

        resolved
    }

    /// Format open predictions as a block suitable for injection into a system prompt.
    ///
    /// Returns `None` if there are no open predictions.
    /// Format:
    ///   ## Open Predictions
    ///   These are predictions I made but haven't resolved yet.
    ///   #1 [2026-03-17]: I expect the build to succeed on first try
    ///
    /// Used by G-024: inject prediction context at agent startup so I remember
    /// what outcomes I committed to before each session starts.
    pub fn format_for_system_prompt(&self) -> Option<String> {
        let open = self.open();
        if open.is_empty() {
            return None;
        }
        let mut lines = vec!["## Open Predictions".to_string()];
        lines.push("These predictions were made but not yet resolved:".to_string());
        for (id, pred) in &open {
            lines.push(format!("- #{} [{}]: {}", id, pred.created, pred.prediction));
        }
        Some(lines.join("\n"))
    }
}
