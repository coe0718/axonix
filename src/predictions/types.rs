//! Types and private helpers for prediction tracking.

use serde::{Deserialize, Serialize};

/// A single prediction entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Human-readable prediction text.
    pub prediction: String,
    /// Date the prediction was made (YYYY-MM-DD).
    pub created: String,
    /// Resolution: what actually happened (None if still open).
    pub outcome: Option<String>,
    /// What was different from the prediction (None if unresolved).
    pub delta: Option<String>,
    /// Date resolved (YYYY-MM-DD), if resolved.
    pub resolved: Option<String>,
}

impl Prediction {
    /// Whether this prediction has been resolved.
    pub fn is_resolved(&self) -> bool {
        self.outcome.is_some()
    }
}

/// Calibration score across all resolved predictions.
#[derive(Debug, Clone)]
pub struct CalibrationScore {
    /// Total number of resolved predictions.
    pub total_resolved: usize,
    /// Number of predictions whose outcome contains "TRUE".
    pub correct: usize,
    /// correct / total_resolved (0.0 if no resolved predictions).
    pub hit_rate: f64,
    /// Average days early (positive = early, negative = late). 0.0 if no delta data.
    pub avg_days_early: f64,
    /// "optimistic", "pessimistic", or "calibrated".
    pub direction_bias: String,
}

impl CalibrationScore {
    /// Format for injection into a system prompt.
    ///
    /// Returns `None` if `total_resolved == 0`.
    pub fn format_for_system_prompt(&self) -> Option<String> {
        if self.total_resolved == 0 {
            return None;
        }
        let hit_pct = self.hit_rate * 100.0;
        let timing_line = if self.avg_days_early > 0.0 {
            format!("Average resolution: {:.1} days early", self.avg_days_early)
        } else if self.avg_days_early < 0.0 {
            format!("Average resolution: {:.1} days late", self.avg_days_early.abs())
        } else {
            "Average resolution: on time".to_string()
        };
        let bias_advice = match self.direction_bias.as_str() {
            "optimistic" => "make bold, specific predictions",
            "pessimistic" => "be more confident — you may be underestimating",
            _ => "keep making precise, verifiable predictions",
        };
        Some(format!(
            "## Prediction Calibration\n\
             {} resolved predictions: {}/{} correct ({:.1}% hit rate)\n\
             {}\n\
             Bias: {} — {}",
            self.total_resolved,
            self.correct,
            self.total_resolved,
            hit_pct,
            timing_line,
            self.direction_bias,
            bias_advice,
        ))
    }
}

/// Parse days from a delta string.
///
/// Looks for patterns like:
/// - "1 day(s) early" → +1.0
/// - "3 days early"   → +3.0
/// - "2 day(s) late"  → -2.0
/// - "1 days late"    → -1.0
///
/// Returns `None` if no parseable pattern found.
pub(super) fn parse_days_from_delta(delta: &str) -> Option<f64> {
    let lower = delta.to_lowercase();
    // Find a number followed by "day"
    let day_pos = lower.find("day")?;
    // Work backwards from "day" to find the number
    let before_day = lower[..day_pos].trim_end();
    let num_str: String = before_day.chars().rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars().rev().collect();
    let n: f64 = num_str.parse().ok()?;
    // Now determine direction: look after "day..." for "early" or "late"
    let after_day = &lower[day_pos..];
    if after_day.contains("early") {
        Some(n)
    } else if after_day.contains("late") {
        Some(-n)
    } else {
        None
    }
}
