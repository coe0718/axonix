//! Timestamp helpers for conversation memory.

/// Return the current UTC time as an ISO 8601 string (seconds precision).
pub(super) fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    unix_to_iso8601(secs)
}

/// Convert a Unix timestamp (seconds) to an ISO 8601 UTC string.
///
/// Example: `1773619200` → `"2026-03-16T00:00:00Z"`
pub(super) fn unix_to_iso8601(secs: u64) -> String {
    let (year, month, day) = unix_to_ymd(secs);
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    )
}

/// Convert Unix timestamp (seconds) to `(year, month, day)` UTC.
fn unix_to_ymd(secs: u64) -> (u32, u32, u32) {
    let days_total = secs / 86400;
    let z = days_total + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (if month <= 2 { y + 1 } else { y }) as u32;
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_to_iso8601_known_date() {
        // 2026-03-16T00:00:00Z = 1773619200
        let result = unix_to_iso8601(1773619200);
        assert_eq!(result, "2026-03-16T00:00:00Z");
    }

    #[test]
    fn test_unix_to_iso8601_epoch() {
        let result = unix_to_iso8601(0);
        assert_eq!(result, "1970-01-01T00:00:00Z");
    }
}
