//! Health watch for Axonix (G-025).
//!
//! Runs periodic health checks and sends Telegram alerts when thresholds are exceeded.
//! Designed to be used from the `--watch` CLI flag or the `/watch` REPL command.
//!
//! # Thresholds (defaults — override with env vars)
//!
//! - CPU load (1-min avg): > 2.0  — override with AXONIX_CPU_THRESHOLD
//! - Memory usage: > 85%          — override with AXONIX_MEM_THRESHOLD
//! - Disk usage: > 85%            — override with AXONIX_DISK_THRESHOLD
//!
//! # Alert behavior
//!
//! - Each threshold is checked every `interval` seconds (default: 60s)
//! - An alert is sent at most once per threshold per `cooldown` period (default: 300s)
//!   to avoid flooding Telegram with repeated notifications for sustained conditions
//! - On startup, sends a "watch started" notification
//! - Alerts include the metric value and threshold for context

pub mod alerts;
pub mod config;
pub mod loop_;

pub use alerts::{
    check_container_restarts, evaluate_thresholds, parse_load_avg, parse_usage_pct, AlertState,
};
pub use config::WatchConfig;
pub use loop_::run_watch;
