//! Time helper functions for Bluesky ISO 8601 timestamps.

/// Generate a current ISO 8601 UTC timestamp string.
///
/// Bluesky requires `createdAt` in the record. Format: `2026-03-16T12:34:56.000Z`
pub(super) fn current_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, min, sec) = unix_to_ymd_hms(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.000Z")
}

/// Convert a Unix timestamp (seconds since epoch) to (year, month, day, hour, min, sec).
///
/// Implements the proleptic Gregorian calendar algorithm used for UTC conversion.
/// Accurate for dates from 1970 through roughly 2100.
pub(super) fn unix_to_ymd_hms(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec = (secs % 60) as u32;
    let mins_total = secs / 60;
    let min = (mins_total % 60) as u32;
    let hours_total = mins_total / 60;
    let hour = (hours_total % 24) as u32;
    let days_total = hours_total / 24; // days since 1970-01-01

    // Algorithm: Civil date from days since epoch (Julian Day Number method)
    // Reference: https://howardhinnant.github.io/date_algorithms.html
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

    (year, month, day, hour, min, sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_iso8601_format() {
        let ts = current_iso8601();
        assert!(ts.ends_with(".000Z"), "must end with .000Z: {ts}");
        assert_eq!(ts.len(), 24, "must be 24 chars: {ts}");
        assert_eq!(&ts[4..5], "-", "year-month separator: {ts}");
        assert_eq!(&ts[7..8], "-", "month-day separator: {ts}");
        assert_eq!(&ts[10..11], "T", "date-time separator: {ts}");
    }

    #[test]
    fn test_current_iso8601_year_is_reasonable() {
        let ts = current_iso8601();
        let year: u32 = ts[..4].parse().expect("year should be numeric");
        assert!(year >= 2024 && year <= 2100, "year should be reasonable: {year}");
    }

    #[test]
    fn test_unix_epoch_is_1970_01_01() {
        let (y, mo, d, h, mi, s) = unix_to_ymd_hms(0);
        assert_eq!((y, mo, d, h, mi, s), (1970, 1, 1, 0, 0, 0), "epoch should be 1970-01-01T00:00:00");
    }

    #[test]
    fn test_known_date_2026_03_16() {
        let (y, mo, d, h, mi, s) = unix_to_ymd_hms(1773619200);
        assert_eq!(y, 2026, "year should be 2026");
        assert_eq!(mo, 3, "month should be 3 (March)");
        assert_eq!(d, 16, "day should be 16");
        assert_eq!(h, 0, "hour should be 0");
        assert_eq!(mi, 0, "minute should be 0");
        assert_eq!(s, 0, "second should be 0");
    }

    #[test]
    fn test_known_date_with_time_components() {
        let secs = 1773671445u64;
        let (y, mo, d, h, mi, s) = unix_to_ymd_hms(secs);
        assert_eq!(y, 2026);
        assert_eq!(mo, 3);
        assert_eq!(d, 16);
        assert_eq!(h, 14);
        assert_eq!(mi, 30);
        assert_eq!(s, 45);
    }

    #[test]
    fn test_unix_to_ymd_month_boundaries() {
        let (y, mo, d, ..) = unix_to_ymd_hms(1767225600);
        assert_eq!(y, 2026, "should be year 2026");
        assert_eq!(mo, 1, "should be January");
        assert_eq!(d, 1, "should be day 1");
    }
}
