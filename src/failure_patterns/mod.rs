//! Failure pattern tracking for Axonix self-monitoring (G-066).
//!
//! Persists a log of known failure types to `.axonix/failure_patterns.json`
//! so that patterns can be reviewed across sessions and surfaced in the
//! morning brief.
//!
//! # Design
//!
//! - Flat JSON array: transparent, human-readable, diffable in git
//! - Each event records type, description, session label, and date
//! - Load-on-read, save-on-write: minimal complexity, no background threads
//! - `most_common_failure_type()` drives the morning brief summary
//!
//! # Example
//!
//! ```
//! use axonix::failure_patterns::{FailurePatternStore, FailureType};
//!
//! let mut store = FailurePatternStore::new("/tmp/test_fp_doctest.json");
//! store.log_failure(
//!     FailureType::FalseCompletion,
//!     "marked goal done without verifying in code",
//!     "Day 10, Session 3",
//!     "2026-03-23",
//! );
//! assert_eq!(store.total_count(), 1);
//! ```

mod types;
mod store;
mod detect;
mod format;

pub use types::{FailureType, FailureEvent};
pub use store::FailurePatternStore;

use std::path::PathBuf;

/// Return the default path for the failure patterns store.
///
/// Uses `AXONIX_FAILURE_PATTERNS_PATH` env var if set, otherwise
/// `.axonix/failure_patterns.json` in the current working directory.
pub fn default_failure_patterns_path() -> PathBuf {
    if let Ok(path) = std::env::var("AXONIX_FAILURE_PATTERNS_PATH") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    PathBuf::from(".axonix/failure_patterns.json")
}

#[cfg(test)]
mod tests;
