//! System health metrics for the home lab.
//!
//! Collects local system metrics (CPU, memory, disk) using standard
//! Linux commands available in the container/host environment.
//!
//! Designed to be called from both the REPL `/health` command and
//! the Telegram `/health` command without network dependencies.

use std::process::Command;

mod cpu;
mod memory;
mod disk;
mod uptime;

pub mod caddy;
pub mod docker;

pub use caddy::{CaddyHealth, caddy_health};
pub use docker::{ContainerStatus, DockerHealth, docker_health};

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
            load_avg: cpu::read_load_avg(),
            memory: memory::read_memory(),
            disk: disk::read_disk(),
            uptime: uptime::read_uptime(),
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

/// Run a shell command and return its stdout, or None on failure.
pub(super) fn run_command(cmd: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── HealthSnapshot::collect ───────────────────────────────────────────────

    #[test]
    fn test_health_snapshot_collect_no_panic() {
        let snap = HealthSnapshot::collect();
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
        assert!(compact.len() < 120, "compact format should be short: {compact}");
    }

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
        let snap = HealthSnapshot {
            load_avg: "0.99, 0.50, 0.25".to_string(),
            memory: "2G / 8G (25% used)".to_string(),
            disk: "10G / 50G (20%)".to_string(),
            uptime: "1d".to_string(),
        };
        let compact = snap.format_compact();
        assert!(compact.contains("0.99"), "compact should show 1-min load (0.99): {compact}");
        assert!(!compact.contains("0.50"), "compact should NOT show 5-min load: {compact}");
    }
}
