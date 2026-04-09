//! Private helpers for prediction tracking.

use std::path::PathBuf;

/// Extract all "G-NNN" goal IDs mentioned in a prediction text.
///
/// Finds every occurrence of `G-` followed by one or more ASCII digits.
/// Returns them in order of appearance, without deduplication.
pub(super) fn extract_goal_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with("G-") {
            let start = i + 2; // skip "G-"
            let num_len = text[start..]
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(text.len() - start);
            let end = start + num_len;
            if end > start {
                ids.push(format!("G-{}", &text[start..end]));
            }
            i = if end > i { end } else { i + 1 };
        } else {
            i += 1;
        }
    }
    ids
}

/// Get today's date as YYYY-MM-DD.
pub(super) fn today_str() -> String {
    // Use the same approach as memory.rs — compute from system time
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = unix_to_ymd(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Convert Unix timestamp to (year, month, day).
/// Matches the implementation in memory.rs.
pub(super) fn unix_to_ymd(secs: u64) -> (u32, u32, u32) {
    let days = (secs / 86400) as u32;
    let mut y = 1970u32;
    let mut remaining = days;
    loop {
        let year_days = if is_leap(y) { 366 } else { 365 };
        if remaining < year_days {
            break;
        }
        remaining -= year_days;
        y += 1;
    }
    let month_days: [u32; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1u32;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        m += 1;
    }
    let d = remaining + 1;
    (y, m, d)
}

pub(super) fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

/// Derive the SQLite DB path from a JSON predictions path.
///
/// Returns `<parent_dir>/axonix.db` alongside the JSON file.
/// Falls back to `.axonix/axonix.db` relative to CWD if the path has no parent.
pub(super) fn db_path_for(json_path: &std::path::Path) -> PathBuf {
    if let Some(parent) = json_path.parent() {
        if !parent.as_os_str().is_empty() {
            return parent.join("axonix.db");
        }
    }
    PathBuf::from(".axonix/axonix.db")
}
