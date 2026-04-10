//! System uptime reading.

use super::run_command;

/// Read system uptime.
pub(super) fn read_uptime() -> String {
    // /proc/uptime contains seconds since boot as a float
    if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
        if let Some(secs_str) = content.split_whitespace().next() {
            if let Ok(secs) = secs_str.parse::<f64>() {
                let secs = secs as u64;
                let days = secs / 86400;
                let hours = (secs % 86400) / 3600;
                let mins = (secs % 3600) / 60;
                return if days > 0 {
                    format!("{days}d {hours}h {mins}m")
                } else {
                    format!("{hours}h {mins}m")
                };
            }
        }
    }

    run_command("uptime -p")
        .unwrap_or_else(|| "(unavailable)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_uptime_returns_string() {
        let result = read_uptime();
        assert!(!result.is_empty(), "uptime must not be empty");
    }
}
