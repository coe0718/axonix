//! Formatting and calibration helpers for PredictionStore — split from store.rs.

use super::types::{CalibrationScore, parse_days_from_delta};
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
