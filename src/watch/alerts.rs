//! Alert state tracking and threshold evaluation for the health watch.

use crate::health::{DockerHealth, HealthSnapshot};
use crate::watch::config::WatchConfig;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tracks the last time an alert was sent for each metric.
/// Used to enforce cooldown periods and prevent alert floods.
#[derive(Debug, Default)]
pub struct AlertState {
    pub(super) last_cpu_alert: Option<Instant>,
    pub(super) last_mem_alert: Option<Instant>,
    pub(super) last_disk_alert: Option<Instant>,
    /// Maps container name → last restart alert time.
    pub last_restart_alert: HashMap<String, Instant>,
}

impl AlertState {
    /// Create a fresh AlertState (no previous alerts — all metrics can alert).
    /// Used by the REPL `/watch` command for point-in-time checks.
    pub fn default_for_repl() -> Self {
        Self::default()
    }

    /// Returns true if a CPU alert can be sent (not in cooldown).
    pub(super) fn can_alert_cpu(&self, cooldown: Duration) -> bool {
        self.last_cpu_alert
            .map(|t| t.elapsed() >= cooldown)
            .unwrap_or(true)
    }

    /// Returns true if a memory alert can be sent.
    pub(super) fn can_alert_mem(&self, cooldown: Duration) -> bool {
        self.last_mem_alert
            .map(|t| t.elapsed() >= cooldown)
            .unwrap_or(true)
    }

    /// Returns true if a disk alert can be sent.
    pub(super) fn can_alert_disk(&self, cooldown: Duration) -> bool {
        self.last_disk_alert
            .map(|t| t.elapsed() >= cooldown)
            .unwrap_or(true)
    }

    /// Returns true if a restart alert can be sent for the named container.
    pub fn can_alert_restart(&self, name: &str, cooldown: Duration) -> bool {
        self.last_restart_alert
            .get(name)
            .map(|t| t.elapsed() >= cooldown)
            .unwrap_or(true)
    }
}

/// Parse a CPU load string (e.g. "0.42, 0.38, 0.31") and return the 1-min average.
///
/// Returns `None` if the string cannot be parsed.
pub fn parse_load_avg(s: &str) -> Option<f64> {
    s.split(',')
        .next()
        .and_then(|first| first.trim().parse().ok())
}

/// Parse a memory/disk usage string for the percentage used.
///
/// Accepts strings like "1.2G / 8.0G (15% used)" or "12G / 50G (24%)".
/// Returns the percentage value (0–100), or `None` if unparseable.
pub fn parse_usage_pct(s: &str) -> Option<u8> {
    // Find the last occurrence of a number before '%'
    // Format: "... (N% ...)" or "... (N%)"
    let paren_start = s.rfind('(')?;
    let paren_content = &s[paren_start + 1..];
    let pct_pos = paren_content.find('%')?;
    let num_str = paren_content[..pct_pos].trim();
    num_str.parse().ok()
}

/// Evaluate a health snapshot against thresholds and return a list of alert messages.
///
/// Each alert message is a human-readable string suitable for sending via Telegram.
/// Returns an empty vec if all metrics are within thresholds.
pub fn evaluate_thresholds(snapshot: &HealthSnapshot, config: &WatchConfig, state: &AlertState) -> Vec<String> {
    let mut alerts = Vec::new();

    // CPU load check
    if let Some(load) = parse_load_avg(&snapshot.load_avg) {
        if load > config.cpu_threshold && state.can_alert_cpu(config.cooldown) {
            alerts.push(format!(
                "⚠️ *High CPU load*: {:.2} (threshold: {:.1})\n\
                 Full: CPU {}",
                load, config.cpu_threshold, snapshot.load_avg
            ));
        }
    }

    // Memory check
    if let Some(pct) = parse_usage_pct(&snapshot.memory) {
        if pct >= config.mem_threshold && state.can_alert_mem(config.cooldown) {
            alerts.push(format!(
                "⚠️ *High memory usage*: {}% (threshold: {}%)\n\
                 Full: {}",
                pct, config.mem_threshold, snapshot.memory
            ));
        }
    }

    // Disk check
    if let Some(pct) = parse_usage_pct(&snapshot.disk) {
        if pct >= config.disk_threshold && state.can_alert_disk(config.cooldown) {
            alerts.push(format!(
                "⚠️ *High disk usage*: {}% (threshold: {}%)\n\
                 Full: {}",
                pct, config.disk_threshold, snapshot.disk
            ));
        }
    }

    alerts
}

/// Check for containers in the "restarting" state and return alert messages.
///
/// Takes a `DockerHealth` snapshot (passed in for testability).
/// Rate-limited by `config.restart_cooldown` per container.
/// Returns an empty vec if Docker is unavailable or no containers are restarting.
pub fn check_container_restarts(docker: &DockerHealth, config: &WatchConfig, state: &AlertState) -> Vec<String> {
    if docker.error.is_some() {
        return vec![];
    }

    let mut alerts = Vec::new();
    for c in &docker.containers {
        if c.state == "restarting" && state.can_alert_restart(&c.name, config.restart_cooldown) {
            alerts.push(format!(
                "⚠️ *Container restarting*: {}\n  Status: {}",
                c.name, c.status
            ));
        }
    }
    alerts
}

#[cfg(test)]
#[path = "alerts_tests.rs"]
mod tests;
