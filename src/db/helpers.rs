/// Return current UTC time formatted as `YYYY-MM-DDTHH:MM:SSZ` using only std.
pub(super) fn now_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert Unix timestamp (seconds) to (year, month, day, hour, min, sec).
/// Gregorian calendar, no leap-second handling — good enough for timestamps.
pub(super) fn epoch_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;

    // Days since 1970-01-01
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if remaining < dy {
            break;
        }
        remaining -= dy;
        y += 1;
    }
    let months = if is_leap(y) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 1u64;
    for &dm in &months {
        if remaining < dm {
            break;
        }
        remaining -= dm;
        mo += 1;
    }
    let d = remaining + 1;
    (y, mo, d, h, m, s)
}

pub(super) fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Add `days` to a UTC timestamp string (YYYY-MM-DDTHH:MM:SSZ).
///
/// Parses the timestamp, adds days * 86400 seconds, re-formats.
/// Falls back to `now_utc()` if parsing fails.
pub(super) fn add_days_to_utc(ts: &str, days: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let base_secs = if ts.len() >= 19 {
        let y: u64 = ts[0..4].parse().unwrap_or(0);
        let mo: u64 = ts[5..7].parse().unwrap_or(0);
        let d: u64 = ts[8..10].parse().unwrap_or(0);
        let h: u64 = ts[11..13].parse().unwrap_or(0);
        let mi: u64 = ts[14..16].parse().unwrap_or(0);
        let s: u64 = ts[17..19].parse().unwrap_or(0);
        if y > 0 && mo > 0 && d > 0 {
            ymd_hms_to_epoch(y, mo, d, h, mi, s)
        } else {
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        }
    } else {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
    };
    let target_secs = base_secs + days * 86400;
    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(target_secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert (year, month, day, hour, min, sec) to Unix timestamp (seconds).
pub(super) fn ymd_hms_to_epoch(y: u64, mo: u64, d: u64, h: u64, mi: u64, s: u64) -> u64 {
    let days_to_year: u64 = (1970..y).map(|yr| if is_leap(yr) { 366 } else { 365 }).sum();
    let month_days: [u64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let days_to_month: u64 = month_days[..(mo as usize).saturating_sub(1)].iter().sum();
    let total_days = days_to_year + days_to_month + d.saturating_sub(1);
    total_days * 86400 + h * 3600 + mi * 60 + s
}

/// Tokenize a string for keyword search.
///
/// Lowercases, splits on whitespace and non-alphanumeric characters,
/// filters out stop words, deduplicates, and returns the result.
pub(super) fn tokenize(s: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "the", "is", "it", "in", "on", "at", "to", "of",
        "for", "and", "or", "but", "not", "was", "has", "be",
    ];

    let lower = s.to_lowercase();
    let mut tokens: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .filter(|t| !STOP_WORDS.contains(t))
        .map(|t| t.to_string())
        .collect();

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    tokens.retain(|t| seen.insert(t.clone()));
    tokens
}
