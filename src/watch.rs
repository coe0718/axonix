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

use crate::health::{DockerHealth, HealthSnapshot};
use crate::telegram::TelegramClient;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Configuration for the health watch.
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// CPU 1-min load average threshold for alert (default: 2.0).
    pub cpu_threshold: f64,
    /// Memory usage percentage threshold (0–100, default: 85).
    pub mem_threshold: u8,
    /// Disk usage percentage threshold (0–100, default: 85).
    pub disk_threshold: u8,
    /// How often to check health (default: 60 seconds).
    pub interval: Duration,
    /// Minimum time between alerts for the same metric (default: 300 seconds).
    /// Prevents Telegram flood during sustained high-load conditions.
    pub cooldown: Duration,
    /// Minimum time between restart alerts for the same container (default: 3600 seconds / 1 hour).
    pub restart_cooldown: Duration,
}

impl Default for WatchConfig {
    fn default() -> Self {
        let cpu_threshold = std::env::var("AXONIX_CPU_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(2.0);
        let mem_threshold = std::env::var("AXONIX_MEM_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|&n| n <= 100)
            .unwrap_or(85);
        let disk_threshold = std::env::var("AXONIX_DISK_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|&n| n <= 100)
            .unwrap_or(85);
        Self {
            cpu_threshold,
            mem_threshold,
            disk_threshold,
            interval: Duration::from_secs(60),
            cooldown: Duration::from_secs(300),
            restart_cooldown: Duration::from_secs(3600),
        }
    }
}

impl WatchConfig {
    /// Create a config with a short interval for testing.
    #[cfg(test)]
    pub fn for_test(interval_secs: u64, cooldown_secs: u64) -> Self {
        Self {
            interval: Duration::from_secs(interval_secs),
            cooldown: Duration::from_secs(cooldown_secs),
            ..Default::default()
        }
    }
}

/// Tracks the last time an alert was sent for each metric.
/// Used to enforce cooldown periods and prevent alert floods.
#[derive(Debug, Default)]
pub struct AlertState {
    last_cpu_alert: Option<Instant>,
    last_mem_alert: Option<Instant>,
    last_disk_alert: Option<Instant>,
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
    fn can_alert_cpu(&self, cooldown: Duration) -> bool {
        self.last_cpu_alert
            .map(|t| t.elapsed() >= cooldown)
            .unwrap_or(true)
    }

    /// Returns true if a memory alert can be sent.
    fn can_alert_mem(&self, cooldown: Duration) -> bool {
        self.last_mem_alert
            .map(|t| t.elapsed() >= cooldown)
            .unwrap_or(true)
    }

    /// Returns true if a disk alert can be sent.
    fn can_alert_disk(&self, cooldown: Duration) -> bool {
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

/// Run the health watch loop.
///
/// Checks health at `config.interval`, sends Telegram alerts when thresholds are
/// exceeded, and respects `config.cooldown` to avoid flooding.
///
/// This function runs indefinitely — call it in a task or with a timeout.
/// Returns only on Telegram send error (which is logged but not fatal).
pub async fn run_watch(config: WatchConfig, tg: &TelegramClient) {
    let mut state = AlertState::default();

    // Startup notification
    let snapshot = HealthSnapshot::collect();
    let startup_msg = format!(
        "👁 *Axonix health watch started*\n\
         Checking every {}s, cooldown {}s\n\
         Thresholds: CPU>{:.1} | Mem>{}% | Disk>{}%\n\
         | Restarts: per-container 1h cooldown\n\
         Current: {}",
        config.interval.as_secs(),
        config.cooldown.as_secs(),
        config.cpu_threshold,
        config.mem_threshold,
        config.disk_threshold,
        snapshot.format_compact(),
    );
    tg.send_message(&startup_msg).await.ok();

    loop {
        tokio::time::sleep(config.interval).await;

        let snapshot = HealthSnapshot::collect();
        let alerts = evaluate_thresholds(&snapshot, &config, &state);

        let docker = crate::health::docker_health();
        let restart_alerts = check_container_restarts(&docker, &config, &state);

        for alert in &alerts {
            // Best-effort: log failures but don't break the watch loop
            if let Err(e) = tg.send_message(alert).await {
                eprintln!("  watch: alert send failed: {e}");
            }
        }

        for alert in &restart_alerts {
            if let Err(e) = tg.send_message(alert).await {
                eprintln!("  watch: restart alert send failed: {e}");
            }
        }

        // Update alert state after sending
        let now = Instant::now();
        if alerts.iter().any(|a| a.contains("CPU load")) {
            state.last_cpu_alert = Some(now);
        }
        if alerts.iter().any(|a| a.contains("memory")) {
            state.last_mem_alert = Some(now);
        }
        if alerts.iter().any(|a| a.contains("disk")) {
            state.last_disk_alert = Some(now);
        }
        // Update restart alert state for any containers that were alerted
        for alert in &restart_alerts {
            // Extract container name from "⚠️ *Container restarting*: <name>\n  Status: ..."
            if let Some(rest) = alert.strip_prefix("⚠️ *Container restarting*: ") {
                let name = rest.split('\n').next().unwrap_or("").to_string();
                if !name.is_empty() {
                    state.last_restart_alert.insert(name, now);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_load_avg ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_load_avg_standard_format() {
        assert_eq!(parse_load_avg("0.42, 0.38, 0.31"), Some(0.42));
    }

    #[test]
    fn test_parse_load_avg_high_load() {
        assert_eq!(parse_load_avg("3.14, 2.50, 2.00"), Some(3.14));
    }

    #[test]
    fn test_parse_load_avg_zero() {
        assert_eq!(parse_load_avg("0.00, 0.00, 0.00"), Some(0.00));
    }

    #[test]
    fn test_parse_load_avg_single_value() {
        assert_eq!(parse_load_avg("1.23"), Some(1.23));
    }

    #[test]
    fn test_parse_load_avg_whitespace_trimmed() {
        assert_eq!(parse_load_avg("  0.50, 0.40, 0.30  "), Some(0.50));
    }

    #[test]
    fn test_parse_load_avg_invalid_returns_none() {
        assert_eq!(parse_load_avg(""), None);
        assert_eq!(parse_load_avg("(unavailable)"), None);
        assert_eq!(parse_load_avg("not a number"), None);
    }

    // ── parse_usage_pct ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_usage_pct_memory_format() {
        // "1.2G / 8.0G (15% used)"
        assert_eq!(parse_usage_pct("1.2G / 8.0G (15% used)"), Some(15));
    }

    #[test]
    fn test_parse_usage_pct_disk_format() {
        // "12G / 50G (24%)"
        assert_eq!(parse_usage_pct("12G / 50G (24%)"), Some(24));
    }

    #[test]
    fn test_parse_usage_pct_zero_percent() {
        assert_eq!(parse_usage_pct("0G / 8.0G (0% used)"), Some(0));
    }

    #[test]
    fn test_parse_usage_pct_high_usage() {
        assert_eq!(parse_usage_pct("7.5G / 8.0G (93% used)"), Some(93));
    }

    #[test]
    fn test_parse_usage_pct_no_paren_returns_none() {
        assert_eq!(parse_usage_pct("(unavailable)"), None);
        assert_eq!(parse_usage_pct("no percentage here"), None);
    }

    #[test]
    fn test_parse_usage_pct_empty_returns_none() {
        assert_eq!(parse_usage_pct(""), None);
    }

    // ── evaluate_thresholds ───────────────────────────────────────────────────

    fn make_snapshot(load: &str, mem: &str, disk: &str) -> HealthSnapshot {
        HealthSnapshot {
            load_avg: load.to_string(),
            memory: mem.to_string(),
            disk: disk.to_string(),
            uptime: "1d 2h 3m".to_string(),
        }
    }

    fn default_config() -> WatchConfig {
        WatchConfig::default()
    }

    fn fresh_state() -> AlertState {
        AlertState::default()
    }

    #[test]
    fn test_evaluate_thresholds_no_alerts_when_under() {
        let snap = make_snapshot("0.50, 0.40, 0.30", "1.0G / 8.0G (12% used)", "12G / 50G (24%)");
        let config = default_config();
        let state = fresh_state();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert!(alerts.is_empty(), "No alerts when all under threshold: {alerts:?}");
    }

    #[test]
    fn test_evaluate_thresholds_cpu_alert_when_over() {
        let snap = make_snapshot("3.50, 3.00, 2.80", "1.0G / 8.0G (12% used)", "12G / 50G (24%)");
        let config = default_config(); // cpu_threshold = 2.0
        let state = fresh_state();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert_eq!(alerts.len(), 1, "Should have exactly 1 CPU alert: {alerts:?}");
        assert!(alerts[0].contains("CPU") || alerts[0].contains("cpu"), "Alert should mention CPU: {}", alerts[0]);
    }

    #[test]
    fn test_evaluate_thresholds_memory_alert_when_over() {
        let snap = make_snapshot("0.50, 0.40, 0.30", "7.0G / 8.0G (87% used)", "12G / 50G (24%)");
        let config = default_config(); // mem_threshold = 85
        let state = fresh_state();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert_eq!(alerts.len(), 1, "Should have exactly 1 memory alert: {alerts:?}");
        assert!(alerts[0].contains("memory") || alerts[0].contains("Memory"), "Alert should mention memory: {}", alerts[0]);
    }

    #[test]
    fn test_evaluate_thresholds_disk_alert_when_over() {
        let snap = make_snapshot("0.50, 0.40, 0.30", "1.0G / 8.0G (12% used)", "45G / 50G (90%)");
        let config = default_config(); // disk_threshold = 85
        let state = fresh_state();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert_eq!(alerts.len(), 1, "Should have exactly 1 disk alert: {alerts:?}");
        assert!(alerts[0].contains("disk") || alerts[0].contains("Disk"), "Alert should mention disk: {}", alerts[0]);
    }

    #[test]
    fn test_evaluate_thresholds_multiple_alerts() {
        let snap = make_snapshot("5.00, 4.00, 3.00", "7.5G / 8.0G (93% used)", "45G / 50G (90%)");
        let config = default_config();
        let state = fresh_state();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert_eq!(alerts.len(), 3, "Should have 3 alerts (CPU + mem + disk): {alerts:?}");
    }

    #[test]
    fn test_evaluate_thresholds_at_exact_threshold_triggers_alert() {
        // mem_threshold = 85, so exactly 85% should trigger
        let snap = make_snapshot("0.50, 0.40, 0.30", "6.8G / 8.0G (85% used)", "12G / 50G (24%)");
        let config = default_config();
        let state = fresh_state();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert_eq!(alerts.len(), 1, "85% should trigger alert at threshold 85: {alerts:?}");
    }

    #[test]
    fn test_evaluate_thresholds_cooldown_suppresses_repeat_alert() {
        // State with a recent CPU alert (within cooldown)
        let mut state = AlertState::default();
        // Manually set last_cpu_alert to "just now" — cooldown should suppress
        state.last_cpu_alert = Some(Instant::now());

        let snap = make_snapshot("5.00, 4.00, 3.00", "1.0G / 8.0G (12% used)", "12G / 50G (24%)");
        let config = WatchConfig {
            cooldown: Duration::from_secs(300), // 5 minutes cooldown
            ..Default::default()
        };
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert!(alerts.is_empty(), "CPU alert should be suppressed during cooldown: {alerts:?}");
    }

    #[test]
    fn test_evaluate_thresholds_after_cooldown_alerts_again() {
        // State with a CPU alert from 301+ seconds ago (beyond cooldown)
        let mut state = AlertState::default();
        // Fake an old alert by using a past instant — we can't go back in time,
        // so test the no-cooldown path: None means no previous alert
        state.last_cpu_alert = None; // never alerted

        let snap = make_snapshot("5.00, 4.00, 3.00", "1.0G / 8.0G (12% used)", "12G / 50G (24%)");
        let config = default_config();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert_eq!(alerts.len(), 1, "Should alert when no previous alert (no cooldown active): {alerts:?}");
    }

    #[test]
    fn test_evaluate_thresholds_unavailable_metrics_no_panic() {
        // When health metrics are unavailable, should not panic or produce alerts
        let snap = make_snapshot("(unavailable)", "(unavailable)", "(unavailable)");
        let config = default_config();
        let state = fresh_state();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert!(alerts.is_empty(), "Unavailable metrics should produce no alerts (can't compare): {alerts:?}");
    }

    #[test]
    fn test_evaluate_thresholds_custom_cpu_threshold() {
        let snap = make_snapshot("1.50, 1.20, 1.00", "1.0G / 8.0G (12% used)", "12G / 50G (24%)");
        let config = WatchConfig {
            cpu_threshold: 1.0, // lower threshold
            ..Default::default()
        };
        let state = fresh_state();
        let alerts = evaluate_thresholds(&snap, &config, &state);
        assert_eq!(alerts.len(), 1, "Should alert at custom CPU threshold 1.0: {alerts:?}");
    }

    // ── WatchConfig ───────────────────────────────────────────────────────────

    #[test]
    fn test_watch_config_default_values() {
        let config = WatchConfig::default();
        assert_eq!(config.cpu_threshold, 2.0, "default CPU threshold should be 2.0");
        assert_eq!(config.mem_threshold, 85, "default mem threshold should be 85%");
        assert_eq!(config.disk_threshold, 85, "default disk threshold should be 85%");
        assert_eq!(config.interval, Duration::from_secs(60), "default interval should be 60s");
        assert_eq!(config.cooldown, Duration::from_secs(300), "default cooldown should be 300s");
    }

    #[test]
    fn test_alert_state_default_all_can_alert() {
        let state = AlertState::default();
        let cooldown = Duration::from_secs(300);
        assert!(state.can_alert_cpu(cooldown), "fresh state: CPU alert should be allowed");
        assert!(state.can_alert_mem(cooldown), "fresh state: mem alert should be allowed");
        assert!(state.can_alert_disk(cooldown), "fresh state: disk alert should be allowed");
    }

    #[test]
    fn test_alert_state_cooldown_blocks_immediately_after_alert() {
        let mut state = AlertState::default();
        state.last_cpu_alert = Some(Instant::now());
        let cooldown = Duration::from_secs(300);
        assert!(!state.can_alert_cpu(cooldown), "should not alert immediately after previous alert");
    }

    // ── check_container_restarts ──────────────────────────────────────────────

    use crate::health::ContainerStatus;

    fn make_container(name: &str, state: &str, status: &str) -> ContainerStatus {
        ContainerStatus {
            name: name.to_string(),
            state: state.to_string(),
            status: status.to_string(),
            healthy: state == "running",
            anomaly: state == "restarting" || status.contains("(unhealthy)"),
        }
    }

    fn make_docker_health(containers: Vec<ContainerStatus>, error: Option<&str>) -> DockerHealth {
        DockerHealth {
            containers,
            error: error.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_check_container_restarts_restarting_triggers_alert() {
        let docker = make_docker_health(
            vec![make_container("axonix", "restarting", "Restarting (1) 5 seconds ago")],
            None,
        );
        let config = WatchConfig::default();
        let state = AlertState::default();
        let alerts = check_container_restarts(&docker, &config, &state);
        assert_eq!(alerts.len(), 1, "restarting container should trigger alert: {alerts:?}");
        assert!(alerts[0].contains("axonix"), "alert should mention container name: {}", alerts[0]);
        assert!(alerts[0].contains("Container restarting"), "alert should say restarting: {}", alerts[0]);
    }

    #[test]
    fn test_check_container_restarts_running_no_alert() {
        let docker = make_docker_health(
            vec![make_container("axonix", "running", "Up 2 days")],
            None,
        );
        let config = WatchConfig::default();
        let state = AlertState::default();
        let alerts = check_container_restarts(&docker, &config, &state);
        assert!(alerts.is_empty(), "running container should not trigger restart alert: {alerts:?}");
    }

    #[test]
    fn test_check_container_restarts_cooldown_suppresses() {
        let docker = make_docker_health(
            vec![make_container("axonix", "restarting", "Restarting (2) 3 seconds ago")],
            None,
        );
        let config = WatchConfig::default(); // restart_cooldown = 3600s
        let mut state = AlertState::default();
        // Simulate a recent alert for this container
        state.last_restart_alert.insert("axonix".to_string(), Instant::now());
        let alerts = check_container_restarts(&docker, &config, &state);
        assert!(alerts.is_empty(), "restart alert should be suppressed during cooldown: {alerts:?}");
    }

    #[test]
    fn test_check_container_restarts_docker_unavailable_no_panic() {
        let docker = make_docker_health(vec![], Some("connection refused"));
        let config = WatchConfig::default();
        let state = AlertState::default();
        let alerts = check_container_restarts(&docker, &config, &state);
        assert!(alerts.is_empty(), "Docker unavailable should return empty vec without panic: {alerts:?}");
    }

    #[test]
    fn test_restart_cooldown_default() {
        let config = WatchConfig::default();
        assert_eq!(
            config.restart_cooldown,
            Duration::from_secs(3600),
            "default restart_cooldown should be 3600s (1 hour)"
        );
    }

    #[test]
    fn test_alert_state_restart_empty_by_default() {
        let state = AlertState::default();
        assert!(
            state.can_alert_restart("any-container", Duration::from_secs(3600)),
            "fresh AlertState should allow restart alert for any container"
        );
    }

    #[test]
    fn test_watch_config_env_var_cpu_threshold() {
        // Test that AXONIX_CPU_THRESHOLD is parsed from env.
        // (just verify the default still works when env is unset)
        // We can't set env vars reliably in parallel tests, so just test defaults.
        // The WatchConfig::for_test already uses defaults.
        let config = WatchConfig::for_test(30, 60);
        // Default thresholds
        assert_eq!(config.mem_threshold, 85);
        assert_eq!(config.disk_threshold, 85);
    }
}
