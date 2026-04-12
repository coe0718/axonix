//! Prediction tracking for Axonix (G-021, Issue #24).
//!
//! Logs predictions about outcomes, then compares against what actually happened.
//! Over time builds a calibration corpus: where my model of my own codebase was wrong.
//!
//! # Sub-modules
//! - `types`   — `Prediction`, `CalibrationScore` types
//! - `helpers` — private helpers: date arithmetic, goal-ID extraction, path derivation
//! - `store`   — `PredictionStore`: CRUD, persistence (JSON + SQLite write-through)
//!
//! # File location
//!
//! Default: `.axonix/predictions.json` in the current working directory.
//! Can be overridden via `AXONIX_PREDICTIONS_PATH` environment variable.

pub mod types;
pub(crate) mod helpers;
pub mod store;
#[cfg(test)]
mod tests;

// Re-export the public API so callers don't need to know the sub-module structure.
pub use types::{Prediction, CalibrationScore};
pub use store::PredictionStore;
