//! REPL state and command dispatch.
//!
//! `ReplState` holds all mutable state for an interactive session.
//! `handle_command` processes a single user input against that state.
//!
//! Sub-modules:
//! - `types`       — ReplState, CommandResult, HISTORY_LIMIT
//! - `commands`    — handle_command main dispatch
//! - `help_cmd`    — /help text builder
//! - `watch_cmd`   — /watch handler
//! - `misc_cmds`   — /failures, /summary, /recap, /archive-journal handlers
//! - `memory`      — /memory command handler
//! - `predict`     — /predict command handler
//! - `ssh_cmd`     — /ssh command handler
//! - `github_cmds` — /comment and /respond handlers

pub mod types;
pub mod commands;
mod help_cmd;
mod watch_cmd;
mod misc_cmds;
mod memory;
mod predict;
mod ssh_cmd;
mod github_cmds;

// Re-export public API so callers use `repl::ReplState`, etc.
pub use types::{ReplState, CommandResult, HISTORY_LIMIT};
pub use commands::handle_command;

#[cfg(test)]
mod tests;
