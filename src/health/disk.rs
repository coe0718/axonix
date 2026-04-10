//! Disk usage reading.

use super::run_command;

/// Read disk usage for root filesystem.
pub(super) fn read_disk() -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_disk_returns_string() {
        let result = read_disk();
        assert!(!result.is_empty(), "disk must not be empty");
    }
}
