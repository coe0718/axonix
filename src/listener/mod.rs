//! Always-on Telegram listener daemon for the personal assistant.
//!
//! Runs 24/7 as a separate process alongside evolve.sh sessions.
//! Handles `/ask` commands immediately with a short-context agent.
//! Writes conversation turns to [`ConversationMemory`] for session context injection.
//!
//! # Two-process model
//!
//! - **evolve.sh sessions** — long-running, high-context self-improvement loops.
//! - **axonix-listener** — always-on, low-latency daemon for operator queries.
//!
//! These run as separate Docker containers sharing the workspace volume.
//! They communicate via `.axonix/conversation_memory.json`.
//!
//! # Example
//!
//! ```
//! use axonix::listener::{ListenerConfig, ListenerStats, build_listener_system_prompt};
//! use axonix::conversation_memory::ConversationMemory;
//!
//! let config = ListenerConfig::default();
//! assert_eq!(config.poll_interval_secs, 2);
//!
//! let mut stats = ListenerStats::new();
//! stats.messages_handled += 1;
//! let summary = stats.format();
//! assert!(summary.contains("messages"));
//!
//! let dir = tempfile::tempdir().unwrap();
//! let mem = ConversationMemory::new(dir.path().join("conv.json"));
//! let prompt = build_listener_system_prompt(&mem, None, &[]);
//! assert!(!prompt.is_empty());
//! ```

pub mod config;
pub mod prompt;
pub mod handlers;
pub mod run;

pub use config::{
    DEFAULT_HAIKU_MODEL,
    select_model_for_command,
    parse_rate_limit_env,
    ListenerConfig,
    AckedIssues,
    ListenerStats,
};
pub use prompt::{
    build_listener_system_prompt,
    format_history_reply,
};
pub(crate) use config::{local_hour, format_duration};
pub(crate) use prompt::get_last_commit_message;
pub(crate) use handlers::append_goal_to_backlog_at;
pub(crate) use handlers::format_predictions_list;
pub use run::run_listener;

#[cfg(test)]
mod tests;
