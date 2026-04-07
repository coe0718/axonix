//! Utility/helper functions for the morning brief.

use std::path::Path;

/// Return today's date in YYYY-MM-DD format using only std (no external deps).
pub(crate) fn today_date_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Re-use the same manual calendar arithmetic as db.rs.
    let days = secs / 86400;
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let dy = if is_leap_year(y) { 366 } else { 365 };
        if remaining < dy { break; }
        remaining -= dy;
        y += 1;
    }
    let months = if is_leap_year(y) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 1u64;
    for &dm in &months {
        if remaining < dm { break; }
        remaining -= dm;
        mo += 1;
    }
    let d = remaining + 1;
    format!("{y:04}-{mo:02}-{d:02}")
}

pub(crate) fn is_leap_year(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Extract a percentage value from strings like "12G / 50G (24%)" or "used 15% used".
pub(crate) fn parse_pct_from_str(s: &str) -> f32 {
    // Find the last '(' followed by a number and '%'
    if let Some(paren) = s.rfind('(') {
        let after = &s[paren + 1..];
        let pct_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if let Ok(v) = pct_str.parse::<f32>() {
            return v;
        }
    }
    0.0
}

/// Parse uptime hours from strings like "3d 4h 22m", "4h 22m", "30m".
pub(crate) fn parse_uptime_hours(s: &str) -> u64 {
    let mut hours: u64 = 0;
    for part in s.split_whitespace() {
        if let Some(d) = part.strip_suffix('d') {
            if let Ok(n) = d.parse::<u64>() {
                hours += n * 24;
            }
        } else if let Some(h) = part.strip_suffix('h') {
            if let Ok(n) = h.parse::<u64>() {
                hours += n;
            }
        }
    }
    hours
}

/// Truncate a string to `max_chars` characters (on char boundary).
pub(crate) fn truncate_str(s: &str, max_chars: usize) -> &str {
    let mut end = s.len();
    let mut char_count = 0;
    for (i, _) in s.char_indices() {
        if char_count >= max_chars {
            end = i;
            break;
        }
        char_count += 1;
    }
    if char_count < max_chars {
        s
    } else {
        &s[..end]
    }
}

/// Compute the approximate number of days between `date_str` and `today`.
/// Both strings must be in "YYYY-MM-DD" format.
pub(crate) fn days_since(date_str: &str, today: &str) -> u64 {
    fn to_days(s: &str) -> u64 {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return 0;
        }
        let y: u64 = parts[0].parse().unwrap_or(2026);
        let m: u64 = parts[1].parse().unwrap_or(1);
        let d: u64 = parts[2].parse().unwrap_or(1);
        y * 365 + m * 30 + d
    }
    let t = to_days(today);
    let p = to_days(date_str);
    if t > p { t - p } else { 0 }
}

/// Count `###` headers inside the `## Backlog` section of GOALS.md.
pub(crate) fn count_backlog_goals() -> usize {
    let path = Path::new("GOALS.md");
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut in_backlog = false;
    let mut count = 0;
    for line in content.lines() {
        if line.starts_with("## Backlog") {
            in_backlog = true;
            continue;
        }
        if in_backlog && line.starts_with("## ") {
            break;
        }
        if in_backlog && line.starts_with("### ") {
            count += 1;
        }
    }
    count
}
