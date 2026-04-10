//! Memory usage reading and byte formatting helpers.

use super::run_command;

/// Read memory usage from /proc/meminfo.
pub(super) fn read_memory() -> String {
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

/// Parse a /proc/meminfo line like "MemTotal:       8048756 kB" → kilobytes.
pub(super) fn parse_kb(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Format bytes into human-readable string.
pub(super) fn format_bytes(bytes: u64) -> String {
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

    #[test]
    fn test_read_memory_returns_string() {
        let result = read_memory();
        assert!(!result.is_empty(), "memory must not be empty");
    }
}
