//! Brief::log_to_db() and Brief::log_to_db_at() implementations.

use std::path::Path;
use crate::db::AxonixDb;
use super::types::Brief;
use super::helpers::today_date_utc;

impl Brief {
    /// Log this brief run to the axonix.db sessions table (G-079).
    ///
    /// Opens the default DB path (`.axonix/axonix.db`) and inserts a row.
    /// On failure, logs a warning to stderr and continues — never crashes the brief.
    ///
    /// Call this after displaying the brief so the DB write doesn't affect output timing.
    pub fn log_to_db(&self) {
        self.log_to_db_at(Path::new(".axonix/axonix.db"));
    }

    /// Log this brief run to the sessions table at the given DB path.
    ///
    /// Separated from `log_to_db()` so tests can pass a temp path.
    pub fn log_to_db_at(&self, db_path: &Path) {
        // Parse day number from DAY_COUNT env var (format: "N YYYY-MM-DD" or just "N").
        let day: i64 = std::env::var("DAY_COUNT")
            .ok()
            .and_then(|s| s.split_whitespace().next().and_then(|n| n.parse().ok()))
            .unwrap_or(0);

        // Today's date in YYYY-MM-DD format (derived from system time, no external deps).
        let date = today_date_utc();

        // Meaningful notes: goal count + open prediction count.
        let notes = format!(
            "brief: {} active goals, {} open predictions",
            self.active_goals.len(),
            self.open_predictions.len(),
        );

        let db = match AxonixDb::open(db_path) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("warning: brief DB open failed ({}): {}", db_path.display(), e);
                return;
            }
        };

        if let Err(e) = db.session_insert(day, "brief", &date, None, None, None, Some(&notes)) {
            eprintln!("warning: brief DB insert failed: {e}");
        }
    }
}
