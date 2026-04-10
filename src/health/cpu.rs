//! CPU load average reading.

use super::run_command;

/// Read load average from /proc/loadavg (Linux).
/// Falls back to `uptime` command output on failure.
pub(super) fn read_load_avg() -> String {
    // Try /proc/loadavg first (fast, no subprocess)
    if let Ok(content) = std::fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() >= 3 {
            return format!("{}, {}, {}", parts[0], parts[1], parts[2]);
        }
    }

    // Fall back to uptime command
    run_command("uptime")
        .and_then(|out| {
            // uptime output: " 10:30:01 up 3 days,  2:22,  1 user,  load average: 0.42, 0.38, 0.31"
            out.split("load average:").nth(1).map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "(unavailable)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_load_avg_returns_string() {
        // On Linux /proc/loadavg should be readable; in any case must not panic
        let result = read_load_avg();
        assert!(!result.is_empty(), "load avg must not be empty");
    }
}
