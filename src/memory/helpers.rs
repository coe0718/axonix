//! Path and date helpers for the memory module.

use std::path::{Path, PathBuf};

/// Return the default path for the memory store.
///
/// Uses `AXONIX_MEMORY_PATH` env var if set, otherwise `.axonix/memory.json`
/// in the current working directory.
pub fn default_memory_path() -> PathBuf {
    if let Ok(path) = std::env::var("AXONIX_MEMORY_PATH") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    PathBuf::from(".axonix/memory.json")
}

/// Derive the default SQLite DB path given a JSON memory path.
///
/// Looks for `.axonix/` as the parent directory; if the JSON file lives there,
/// the DB lives alongside it as `axonix.db`. Otherwise uses `.axonix/axonix.db`
/// relative to the current working directory.
pub(super) fn default_db_path_for(json_path: &Path) -> PathBuf {
    if let Some(parent) = json_path.parent() {
        if !parent.as_os_str().is_empty() {
            return parent.join("axonix.db");
        }
    }
    PathBuf::from(".axonix/axonix.db")
}

/// Return today's date as a compact string (YYYY-MM-DD).
///
/// Used to timestamp memory writes.
pub(crate) fn current_date() -> String {
    // Use the same unix_to_ymd algorithm from bluesky.rs (no chrono dep)
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day) = unix_to_ymd(secs);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Convert Unix timestamp to (year, month, day) UTC.
pub(crate) fn unix_to_ymd(secs: u64) -> (u32, u32, u32) {
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
