//! Cycle summary: compact session state persisted across agent restarts.
//!
//! Addresses context window exhaustion (Issue #38): each session writes a
//! compact summary of what was done, what changed, and what's pending.
//! The next session loads this summary and injects it into the system prompt,
//! giving the agent enough context to continue intelligently without replaying
//! the full message history.
//!
//! # Design
//!
//! - Flat JSON file at `.axonix/cycle_summary.json`
//! - Written at session end by the evolve.sh orchestrator (or via `/summary` REPL command)
//! - Loaded at startup alongside memory and predictions
//! - Injected into system prompt as a compact "## Last Session" context block
//! - Bounded size: never grows beyond a fixed number of entries
//!
//! # Format
//!
//! ```json
//! {
//!   "session": "Day 7, Session 5",
//!   "date": "2026-03-20",
//!   "completed": ["Implemented cycle_summary module", "Fixed Issue #38"],
//!   "changed_files": ["src/cycle_summary.rs", "src/lib.rs", "src/main.rs"],
//!   "pending": ["G-031: morning brief on schedule", "G-033: context window fix"],
//!   "learnings": ["cycle_summary.json keeps context bounded across sessions"]
//! }
//! ```
//!
//! # Example
//!
//! ```
//! use axonix::cycle_summary::CycleSummary;
//!
//! let mut summary = CycleSummary::new("/tmp/test-cycle-summary.json");
//! summary.set_session("Day 7, Session 5", "2026-03-20");
//! summary.add_completed("Implemented cycle_summary module");
//! summary.add_changed_file("src/cycle_summary.rs");
//! summary.add_pending("G-031: morning brief on schedule");
//! summary.add_learning("compact summary keeps context bounded");
//! let _ = summary.save();
//!
//! let loaded = CycleSummary::load("/tmp/test-cycle-summary.json");
//! assert!(loaded.format_for_system_prompt().is_some());
//! ```

mod types;
mod io;
mod format;
mod collect;

pub use types::{CycleSummary, CycleSummaryData};

#[cfg(test)]
mod tests;
