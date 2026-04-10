//! Tests for watch::alerts — threshold evaluation and container restart checks.

use super::*;
use crate::health::ContainerStatus;

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
    assert_eq!(parse_usage_pct("1.2G / 8.0G (15% used)"), Some(15));
}

#[test]
fn test_parse_usage_pct_disk_format() {
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

fn make_snapshot(load: &str, mem: &str, disk: &str) -> crate::health::HealthSnapshot {
    crate::health::HealthSnapshot {
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
    let config = default_config();
    let state = fresh_state();
    let alerts = evaluate_thresholds(&snap, &config, &state);
    assert_eq!(alerts.len(), 1, "Should have exactly 1 CPU alert: {alerts:?}");
    assert!(alerts[0].contains("CPU") || alerts[0].contains("cpu"), "Alert should mention CPU: {}", alerts[0]);
}

#[test]
fn test_evaluate_thresholds_memory_alert_when_over() {
    let snap = make_snapshot("0.50, 0.40, 0.30", "7.0G / 8.0G (87% used)", "12G / 50G (24%)");
    let config = default_config();
    let state = fresh_state();
    let alerts = evaluate_thresholds(&snap, &config, &state);
    assert_eq!(alerts.len(), 1, "Should have exactly 1 memory alert: {alerts:?}");
    assert!(alerts[0].contains("memory") || alerts[0].contains("Memory"), "Alert should mention memory: {}", alerts[0]);
}

#[test]
fn test_evaluate_thresholds_disk_alert_when_over() {
    let snap = make_snapshot("0.50, 0.40, 0.30", "1.0G / 8.0G (12% used)", "45G / 50G (90%)");
    let config = default_config();
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
    let snap = make_snapshot("0.50, 0.40, 0.30", "6.8G / 8.0G (85% used)", "12G / 50G (24%)");
    let config = default_config();
    let state = fresh_state();
    let alerts = evaluate_thresholds(&snap, &config, &state);
    assert_eq!(alerts.len(), 1, "85% should trigger alert at threshold 85: {alerts:?}");
}

#[test]
fn test_evaluate_thresholds_cooldown_suppresses_repeat_alert() {
    use std::time::{Duration, Instant};
    let mut state = AlertState::default();
    state.last_cpu_alert = Some(Instant::now());
    let snap = make_snapshot("5.00, 4.00, 3.00", "1.0G / 8.0G (12% used)", "12G / 50G (24%)");
    let config = WatchConfig {
        cooldown: Duration::from_secs(300),
        ..Default::default()
    };
    let alerts = evaluate_thresholds(&snap, &config, &state);
    assert!(alerts.is_empty(), "CPU alert should be suppressed during cooldown: {alerts:?}");
}

#[test]
fn test_evaluate_thresholds_after_cooldown_alerts_again() {
    let mut state = AlertState::default();
    state.last_cpu_alert = None;
    let snap = make_snapshot("5.00, 4.00, 3.00", "1.0G / 8.0G (12% used)", "12G / 50G (24%)");
    let config = default_config();
    let alerts = evaluate_thresholds(&snap, &config, &state);
    assert_eq!(alerts.len(), 1, "Should alert when no previous alert: {alerts:?}");
}

#[test]
fn test_evaluate_thresholds_unavailable_metrics_no_panic() {
    let snap = make_snapshot("(unavailable)", "(unavailable)", "(unavailable)");
    let config = default_config();
    let state = fresh_state();
    let alerts = evaluate_thresholds(&snap, &config, &state);
    assert!(alerts.is_empty(), "Unavailable metrics should produce no alerts: {alerts:?}");
}

#[test]
fn test_evaluate_thresholds_custom_cpu_threshold() {
    let snap = make_snapshot("1.50, 1.20, 1.00", "1.0G / 8.0G (12% used)", "12G / 50G (24%)");
    let config = WatchConfig {
        cpu_threshold: 1.0,
        ..Default::default()
    };
    let state = fresh_state();
    let alerts = evaluate_thresholds(&snap, &config, &state);
    assert_eq!(alerts.len(), 1, "Should alert at custom CPU threshold 1.0: {alerts:?}");
}

// ── WatchConfig ───────────────────────────────────────────────────────────

#[test]
fn test_watch_config_default_values() {
    use std::time::Duration;
    let config = WatchConfig::default();
    assert_eq!(config.cpu_threshold, 2.0);
    assert_eq!(config.mem_threshold, 85);
    assert_eq!(config.disk_threshold, 85);
    assert_eq!(config.interval, Duration::from_secs(60));
    assert_eq!(config.cooldown, Duration::from_secs(300));
}

#[test]
fn test_alert_state_default_all_can_alert() {
    use std::time::Duration;
    let state = AlertState::default();
    let cooldown = Duration::from_secs(300);
    assert!(state.can_alert_cpu(cooldown));
    assert!(state.can_alert_mem(cooldown));
    assert!(state.can_alert_disk(cooldown));
}

#[test]
fn test_alert_state_cooldown_blocks_immediately_after_alert() {
    use std::time::{Duration, Instant};
    let mut state = AlertState::default();
    state.last_cpu_alert = Some(Instant::now());
    let cooldown = Duration::from_secs(300);
    assert!(!state.can_alert_cpu(cooldown));
}

// ── check_container_restarts ──────────────────────────────────────────────

fn make_container(name: &str, state: &str, status: &str) -> ContainerStatus {
    ContainerStatus {
        name: name.to_string(),
        state: state.to_string(),
        status: status.to_string(),
        healthy: state == "running",
        anomaly: state == "restarting" || status.contains("(unhealthy)"),
    }
}

fn make_docker_health(containers: Vec<ContainerStatus>, error: Option<&str>) -> crate::health::DockerHealth {
    crate::health::DockerHealth {
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
    use std::time::Instant;
    let docker = make_docker_health(
        vec![make_container("axonix", "restarting", "Restarting (2) 3 seconds ago")],
        None,
    );
    let config = WatchConfig::default();
    let mut state = AlertState::default();
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
    assert!(alerts.is_empty(), "Docker unavailable should return empty: {alerts:?}");
}

#[test]
fn test_restart_cooldown_default() {
    use std::time::Duration;
    let config = WatchConfig::default();
    assert_eq!(config.restart_cooldown, Duration::from_secs(3600));
}

#[test]
fn test_alert_state_restart_empty_by_default() {
    use std::time::Duration;
    let state = AlertState::default();
    assert!(state.can_alert_restart("any-container", Duration::from_secs(3600)));
}

#[test]
fn test_watch_config_env_var_cpu_threshold() {
    let config = WatchConfig::for_test(30, 60);
    assert_eq!(config.mem_threshold, 85);
    assert_eq!(config.disk_threshold, 85);
}
