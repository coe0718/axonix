//! Data types: `FailureType` enum and `FailureEvent` struct.

/// Known categories of agent failure for self-monitoring.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum FailureType {
    /// Marked a goal done without verifying in code.
    FalseCompletion,
    /// Claimed an infra blocker that didn't exist.
    InfraBlindness,
    /// Claimed something was implemented that wasn't.
    FalseClaim,
    /// Committed code that had to be reverted.
    RevertRequired,
    /// Marked a goal complete then had to reopen it.
    GoalReopened,
    /// Session ended without updating GOALS.md/METRICS.md.
    MissedWrapUp,
    /// Any other failure type with a free-form description.
    Other(String),
}

impl FailureType {
    /// Return a human-readable label for this failure type.
    pub fn label(&self) -> String {
        match self {
            FailureType::FalseCompletion => "FalseCompletion".to_string(),
            FailureType::InfraBlindness  => "InfraBlindness".to_string(),
            FailureType::FalseClaim      => "FalseClaim".to_string(),
            FailureType::RevertRequired  => "RevertRequired".to_string(),
            FailureType::GoalReopened    => "GoalReopened".to_string(),
            FailureType::MissedWrapUp    => "MissedWrapUp".to_string(),
            FailureType::Other(s)        => format!("Other({})", s),
        }
    }

    /// Return the canonical string key used for counting / bucketing.
    pub(crate) fn key(&self) -> String {
        match self {
            FailureType::Other(s) => format!("Other:{}", s),
            _ => self.label(),
        }
    }
}

/// A single logged failure event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FailureEvent {
    /// The category of failure.
    pub failure_type: FailureType,
    /// Human-readable description of what went wrong.
    pub description: String,
    /// Session label, e.g. "Day 10, Session 5".
    pub session: String,
    /// ISO date string, e.g. "2026-03-23".
    pub date: String,
}
