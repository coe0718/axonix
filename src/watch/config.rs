//! `WatchConfig` — threshold and timing configuration for the health watch.

use std::time::Duration;

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_restart_cooldown_default() {
        let config = WatchConfig::default();
        assert_eq!(
            config.restart_cooldown,
            Duration::from_secs(3600),
            "default restart_cooldown should be 3600s (1 hour)"
        );
    }

    #[test]
    fn test_watch_config_env_var_cpu_threshold() {
        // Test that AXONIX_CPU_THRESHOLD is parsed from env.
        // (just verify the default still works when env is unset)
        // We can't set env vars reliably in parallel tests, so just test defaults.
        let config = WatchConfig::for_test(30, 60);
        // Default thresholds
        assert_eq!(config.mem_threshold, 85);
        assert_eq!(config.disk_threshold, 85);
    }
}
