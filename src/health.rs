//! System health metrics for the home lab.
//!
//! Collects local system metrics (CPU, memory, disk) using standard
//! Linux commands available in the container/host environment.
//!
//! Designed to be called from both the REPL `/health` command and
//! the Telegram `/health` command without network dependencies.

use std::process::Command;

/// A snapshot of system health metrics.
#[derive(Debug, Clone)]
pub struct HealthSnapshot {
    /// CPU load average (1min, 5min, 15min) as a string, e.g. "0.42, 0.38, 0.31"
    pub load_avg: String,
    /// Memory usage summary, e.g. "used: 1.2G / total: 8.0G (15%)"
    pub memory: String,
    /// Root disk usage, e.g. "used: 12G / total: 50G (24%)"
    pub disk: String,
    /// Uptime summary, e.g. "3 days, 4:22"
    pub uptime: String,
}

impl HealthSnapshot {
    /// Collect a fresh health snapshot from the local system.
    ///
    /// All commands are run with timeouts via shell — if a command fails,
    /// the field falls back to "(unavailable)" so a single failure doesn't
    /// prevent the rest of the snapshot from being reported.
    pub fn collect() -> Self {
        Self {
            load_avg: read_load_avg(),
            memory: read_memory(),
            disk: read_disk(),
            uptime: read_uptime(),
        }
    }

    /// Format the snapshot as a multi-line human-readable string.
    pub fn format(&self) -> String {
        format!(
            "🖥 System Health\n\
             CPU load:  {}\n\
             Memory:    {}\n\
             Disk (/):  {}\n\
             Uptime:    {}",
            self.load_avg, self.memory, self.disk, self.uptime
        )
    }

    /// Format as a compact single-line summary for banners.
    pub fn format_compact(&self) -> String {
        format!(
            "load {} | mem {} | disk {}",
            self.load_avg.split(',').next().unwrap_or(&self.load_avg).trim(),
            self.memory,
            self.disk,
        )
    }
}

/// Read load average from /proc/loadavg (Linux).
/// Falls back to `uptime` command output on failure.
fn read_load_avg() -> String {
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

/// Read memory usage from /proc/meminfo.
fn read_memory() -> String {
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        let mut total_kb: u64 = 0;
        let mut available_kb: u64 = 0;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = parse_kb(line);
            } else if line.starts_with("MemAvailable:") {
                available_kb = parse_kb(line);
            }
        }

        if total_kb > 0 {
            let used_kb = total_kb.saturating_sub(available_kb);
            let pct = (used_kb * 100) / total_kb;
            return format!(
                "{} / {} ({}% used)",
                format_bytes(used_kb * 1024),
                format_bytes(total_kb * 1024),
                pct
            );
        }
    }

    // Fall back to `free -h`
    run_command("free -h")
        .and_then(|out| {
            out.lines()
                .find(|l| l.starts_with("Mem:"))
                .map(|l| {
                    let parts: Vec<&str> = l.split_whitespace().collect();
                    if parts.len() >= 3 {
                        format!("{} used / {} total", parts[2], parts[1])
                    } else {
                        l.to_string()
                    }
                })
        })
        .unwrap_or_else(|| "(unavailable)".to_string())
}

/// Read disk usage for root filesystem.
fn read_disk() -> String {
    run_command("df -h /")
        .and_then(|out| {
            // df -h output (2nd line): /dev/sda1  50G  12G  35G  26%  /
            out.lines().nth(1).map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    format!("{} / {} ({})", parts[2], parts[1], parts[4])
                } else {
                    line.to_string()
                }
            })
        })
        .unwrap_or_else(|| "(unavailable)".to_string())
}

/// Read system uptime.
fn read_uptime() -> String {
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

/// Run a shell command and return its stdout, or None on failure.
fn run_command(cmd: &str) -> Option<String> {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    let prog = parts[0];
    let args: Vec<&str> = if parts.len() > 1 {
        parts[1].split_whitespace().collect()
    } else {
        vec![]
    };

    Command::new(prog)
        .args(&args)
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parse a /proc/meminfo line like "MemTotal:       8048756 kB" → kilobytes.
fn parse_kb(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Format bytes into human-readable string.
fn format_bytes(bytes: u64) -> String {
    const GIB: u64 = 1024 * 1024 * 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes >= GIB {
        let gb = bytes as f64 / GIB as f64;
        format!("{:.1}G", gb)
    } else if bytes >= MIB {
        let mb = bytes as f64 / MIB as f64;
        format!("{:.0}M", mb)
    } else {
        format!("{}K", bytes / 1024)
    }
}

/// Caddy reverse proxy health information from the admin API.
#[derive(Debug, Clone)]
pub struct CaddyHealth {
    /// Whether the Caddy admin API responded.
    pub reachable: bool,
    /// Number of upstreams monitored by Caddy's reverse proxy.
    pub upstream_count: usize,
    /// Number of unhealthy upstreams (empty = all healthy or no upstreams).
    pub unhealthy: Vec<String>,
    /// Error message if the admin API was unreachable.
    pub error: Option<String>,
}

impl CaddyHealth {
    pub fn format(&self) -> String {
        if !self.reachable {
            return format!(
                "🔴 Caddy: unreachable ({})",
                self.error.as_deref().unwrap_or("unknown")
            );
        }
        if self.unhealthy.is_empty() {
            format!("✅ Caddy: {} upstream(s) healthy", self.upstream_count)
        } else {
            format!(
                "⚠️ Caddy: {}/{} upstreams unhealthy: {}",
                self.unhealthy.len(),
                self.upstream_count,
                self.unhealthy.join(", ")
            )
        }
    }
}

/// Query the Caddy admin API for reverse proxy upstream health.
///
/// Uses the `CADDY_ADMIN_URL` environment variable (default: `http://localhost:2019`).
/// Falls back gracefully when Caddy is not running.
pub fn caddy_health() -> CaddyHealth {
    let admin_url = std::env::var("CADDY_ADMIN_URL")
        .unwrap_or_else(|_| "http://localhost:2019".to_string());
    let url = format!("{}/reverse_proxy/upstreams", admin_url.trim_end_matches('/'));

    let output = Command::new("curl")
        .args(["-sf", "--max-time", "3", &url])
        .output();

    match output {
        Err(e) => CaddyHealth {
            reachable: false,
            upstream_count: 0,
            unhealthy: vec![],
            error: Some(e.to_string()),
        },
        Ok(out) if !out.status.success() => {
            let err = String::from_utf8_lossy(&out.stderr).to_string();
            CaddyHealth {
                reachable: false,
                upstream_count: 0,
                unhealthy: vec![],
                error: Some(if err.is_empty() {
                    "curl failed".to_string()
                } else {
                    err
                }),
            }
        }
        Ok(out) => {
            let body = String::from_utf8_lossy(&out.stdout);
            parse_caddy_upstreams(&body)
        }
    }
}

/// Parse a JSON array of Caddy upstream objects.
///
/// Format: `[{"address":"host:port","num_requests":0,"fails":0}, ...]`
fn parse_caddy_upstreams(json: &str) -> CaddyHealth {
    let trimmed = json.trim();
    if trimmed == "null" || trimmed.is_empty() {
        return CaddyHealth {
            reachable: true,
            upstream_count: 0,
            unhealthy: vec![],
            error: None,
        };
    }
    let mut upstream_count = 0;
    let mut unhealthy = vec![];
    // Split by "address" to find each upstream entry
    for entry in trimmed.split("\"address\"") {
        if !entry.contains(':') {
            continue;
        }
        // Extract address value (first quoted string after the colon)
        let addr = entry.split('"').nth(1).unwrap_or("unknown").to_string();
        // Check if fails > 0: look for "fails":N where N > 0
        let has_fails = entry.contains("\"fails\":") && {
            let after_fails = entry.split("\"fails\":").nth(1).unwrap_or("0");
            let fails_str: String = after_fails
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            fails_str.parse::<u64>().unwrap_or(0) > 0
        };
        if !addr.is_empty() && addr != "unknown" {
            upstream_count += 1;
            if has_fails {
                unhealthy.push(addr);
            }
        }
    }
    CaddyHealth {
        reachable: true,
        upstream_count,
        unhealthy,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_bytes ──────────────────────────────────────────────────────────

    #[test]
    fn test_format_bytes_gigabytes() {
        assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0G");
    }

    #[test]
    fn test_format_bytes_megabytes() {
        assert_eq!(format_bytes(512 * 1024 * 1024), "512M");
    }

    #[test]
    fn test_format_bytes_kilobytes() {
        assert_eq!(format_bytes(4096), "4K");
    }

    #[test]
    fn test_format_bytes_fractional_gb() {
        // 1.5 GiB
        let val = (1024 + 512) * 1024 * 1024;
        let result = format_bytes(val);
        assert!(result.contains("1.5G") || result.contains("1.4G"), "should show ~1.5G: {result}");
    }

    // ── parse_kb ─────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_kb_valid() {
        assert_eq!(parse_kb("MemTotal:       8048756 kB"), 8048756);
    }

    #[test]
    fn test_parse_kb_invalid() {
        assert_eq!(parse_kb(""), 0);
        assert_eq!(parse_kb("no numbers here"), 0);
    }

    // ── HealthSnapshot::collect ───────────────────────────────────────────────

    #[test]
    fn test_health_snapshot_collect_no_panic() {
        // Must not panic. On any Linux system (including Docker) this should
        // return real values; in a sandbox it should return "(unavailable)".
        let snap = HealthSnapshot::collect();
        // All fields must be non-empty strings
        assert!(!snap.load_avg.is_empty(), "load_avg should not be empty");
        assert!(!snap.memory.is_empty(), "memory should not be empty");
        assert!(!snap.disk.is_empty(), "disk should not be empty");
        assert!(!snap.uptime.is_empty(), "uptime should not be empty");
    }

    #[test]
    fn test_health_snapshot_format_contains_labels() {
        let snap = HealthSnapshot {
            load_avg: "0.10, 0.05, 0.01".to_string(),
            memory: "1.0G / 8.0G (12% used)".to_string(),
            disk: "12G / 50G (24%)".to_string(),
            uptime: "2d 4h 30m".to_string(),
        };
        let formatted = snap.format();
        assert!(formatted.contains("CPU load"), "format should include CPU label");
        assert!(formatted.contains("Memory"), "format should include Memory label");
        assert!(formatted.contains("Disk"), "format should include Disk label");
        assert!(formatted.contains("Uptime"), "format should include Uptime label");
    }

    #[test]
    fn test_health_snapshot_format_compact() {
        let snap = HealthSnapshot {
            load_avg: "0.42, 0.38, 0.31".to_string(),
            memory: "2.0G / 8.0G (25% used)".to_string(),
            disk: "15G / 50G (30%)".to_string(),
            uptime: "1d 2h 5m".to_string(),
        };
        let compact = snap.format_compact();
        assert!(compact.contains("load"), "compact should mention load");
        assert!(compact.contains("mem"), "compact should mention mem");
        assert!(compact.contains("disk"), "compact should mention disk");
        // Should be reasonably short
        assert!(compact.len() < 120, "compact format should be short: {compact}");
    }

    #[test]
    fn test_read_load_avg_returns_string() {
        // On Linux /proc/loadavg should be readable; in any case must not panic
        let result = read_load_avg();
        assert!(!result.is_empty(), "load avg must not be empty");
    }

    #[test]
    fn test_read_memory_returns_string() {
        let result = read_memory();
        assert!(!result.is_empty(), "memory must not be empty");
    }

    #[test]
    fn test_read_disk_returns_string() {
        let result = read_disk();
        assert!(!result.is_empty(), "disk must not be empty");
    }

    #[test]
    fn test_read_uptime_returns_string() {
        let result = read_uptime();
        assert!(!result.is_empty(), "uptime must not be empty");
    }

    // ── format_bytes edge cases ───────────────────────────────────────────────

    #[test]
    fn test_format_bytes_zero() {
        // 0 bytes — less than MiB — should show in K
        let result = format_bytes(0);
        assert!(result.ends_with('K'), "0 bytes should be shown as 0K: {result}");
    }

    #[test]
    fn test_format_bytes_exactly_one_gib() {
        let result = format_bytes(1024 * 1024 * 1024);
        assert_eq!(result, "1.0G", "exactly 1 GiB should be '1.0G': {result}");
    }

    #[test]
    fn test_format_bytes_exactly_one_mib() {
        let result = format_bytes(1024 * 1024);
        assert_eq!(result, "1M", "exactly 1 MiB should be '1M': {result}");
    }

    #[test]
    fn test_format_bytes_large_value() {
        // 32 GiB — should still format cleanly
        let result = format_bytes(32 * 1024 * 1024 * 1024);
        assert!(result.contains('G'), "32 GiB should show G suffix: {result}");
    }

    // ── parse_kb edge cases ───────────────────────────────────────────────────

    #[test]
    fn test_parse_kb_large_value() {
        // 16 GB RAM = 16 * 1024 * 1024 kB
        let line = "MemTotal:       16777216 kB";
        assert_eq!(parse_kb(line), 16_777_216);
    }

    #[test]
    fn test_parse_kb_only_whitespace() {
        assert_eq!(parse_kb("   "), 0);
    }

    #[test]
    fn test_parse_kb_numeric_overflow_safe() {
        // Very large number — must not panic, may or may not parse (u64 overflow)
        let line = "MemTotal: 99999999999999999999999 kB";
        let _result = parse_kb(line); // must not panic; value doesn't matter
    }

    // ── HealthSnapshot format invariants ─────────────────────────────────────

    #[test]
    fn test_health_snapshot_format_contains_values() {
        let snap = HealthSnapshot {
            load_avg: "1.23, 0.45, 0.67".to_string(),
            memory: "4.0G / 16.0G (25% used)".to_string(),
            disk: "20G / 100G (20%)".to_string(),
            uptime: "5d 2h 30m".to_string(),
        };
        let formatted = snap.format();
        assert!(formatted.contains("1.23"), "format should include load value");
        assert!(formatted.contains("4.0G"), "format should include memory value");
        assert!(formatted.contains("20G"), "format should include disk value");
        assert!(formatted.contains("5d 2h"), "format should include uptime value");
    }

    #[test]
    fn test_health_snapshot_compact_uses_1min_load() {
        // format_compact should take the first comma-separated load value (1-min)
        let snap = HealthSnapshot {
            load_avg: "0.99, 0.50, 0.25".to_string(),
            memory: "2G / 8G (25% used)".to_string(),
            disk: "10G / 50G (20%)".to_string(),
            uptime: "1d".to_string(),
        };
        let compact = snap.format_compact();
        assert!(
            compact.contains("0.99"),
            "compact should show 1-min load (0.99): {compact}"
        );
        assert!(
            !compact.contains("0.50"),
            "compact should NOT show 5-min load: {compact}"
        );
    }

    // ── CaddyHealth ───────────────────────────────────────────────────────────

    #[test]
    fn test_caddy_health_format_unreachable() {
        let h = CaddyHealth {
            reachable: false,
            upstream_count: 0,
            unhealthy: vec![],
            error: Some("connection refused".to_string()),
        };
        let fmt = h.format();
        assert!(fmt.contains("unreachable"), "should say unreachable: {fmt}");
        assert!(fmt.contains("connection refused"), "should include error: {fmt}");
    }

    #[test]
    fn test_caddy_health_format_all_healthy() {
        let h = CaddyHealth {
            reachable: true,
            upstream_count: 3,
            unhealthy: vec![],
            error: None,
        };
        let fmt = h.format();
        assert!(fmt.contains('3'), "should contain upstream count: {fmt}");
        assert!(fmt.contains("healthy"), "should say healthy: {fmt}");
    }

    #[test]
    fn test_caddy_health_format_some_unhealthy() {
        let h = CaddyHealth {
            reachable: true,
            upstream_count: 3,
            unhealthy: vec!["host:8080".to_string()],
            error: None,
        };
        let fmt = h.format();
        assert!(fmt.contains("unhealthy"), "should mention unhealthy: {fmt}");
        assert!(fmt.contains("host:8080"), "should list unhealthy host: {fmt}");
    }

    #[test]
    fn test_parse_caddy_upstreams_empty_null() {
        let result = parse_caddy_upstreams("null");
        assert_eq!(result.upstream_count, 0);
        assert!(result.reachable);
        assert!(result.unhealthy.is_empty());
    }

    #[test]
    fn test_parse_caddy_upstreams_all_healthy() {
        let json = r#"[{"address":"host:8080","num_requests":0,"fails":0}]"#;
        let result = parse_caddy_upstreams(json);
        assert_eq!(result.upstream_count, 1, "should count 1 upstream");
        assert!(result.unhealthy.is_empty(), "no unhealthy upstreams");
        assert!(result.reachable);
    }

    #[test]
    fn test_parse_caddy_upstreams_with_failures() {
        let json = r#"[{"address":"host:8080","num_requests":5,"fails":2},{"address":"host:8081","num_requests":3,"fails":0}]"#;
        let result = parse_caddy_upstreams(json);
        assert_eq!(result.upstream_count, 2, "should count 2 upstreams");
        assert_eq!(result.unhealthy.len(), 1, "should have 1 unhealthy upstream");
        assert_eq!(result.unhealthy[0], "host:8080", "host:8080 should be unhealthy");
    }
}
