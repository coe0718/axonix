use rusqlite::{Result, params};
use super::AxonixDb;
use super::helpers::now_utc;
use super::types::SessionRow;

impl AxonixDb {
    /// Insert a session record and return its new row id.
    #[allow(clippy::too_many_arguments)]
    pub fn session_insert(
        &self,
        day: i64,
        session: &str,
        date: &str,
        tokens: Option<&str>,
        tests: Option<i64>,
        failed: Option<i64>,
        notes: Option<&str>,
    ) -> Result<i64> {
        let now = now_utc();
        self.conn.execute(
            "INSERT INTO sessions (day, session, date, tokens, tests, failed, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![day, session, date, tokens, tests, failed, notes, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return the most recent `limit` sessions, ordered by (day DESC, session DESC).
    pub fn sessions_recent(&self, limit: usize) -> Result<Vec<SessionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, day, session, date, tokens, tests, failed, notes
             FROM sessions
             ORDER BY day DESC, session DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                day: row.get(1)?,
                session: row.get(2)?,
                date: row.get(3)?,
                tokens: row.get(4)?,
                tests: row.get(5)?,
                failed: row.get(6)?,
                notes: row.get(7)?,
            })
        })?;
        rows.collect()
    }
}
