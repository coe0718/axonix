use rusqlite::{Result, params};
use super::AxonixDb;
use super::helpers::now_utc;
use super::types::GoalRow;

impl AxonixDb {
    /// Upsert a goal (insert or replace). `status` must be one of
    /// `"active"`, `"backlog"`, or `"done"`.
    pub fn goal_upsert(&self, id: &str, title: &str, status: &str) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "INSERT OR REPLACE INTO goals (id, title, status, created_at, completed_at)
             VALUES (
                 ?1, ?2, ?3, ?4,
                 CASE WHEN ?3 = 'done' THEN ?4 ELSE NULL END
             )",
            params![id, title, status, now],
        )?;
        Ok(())
    }

    /// Get a goal by ID. Returns `None` if not found.
    pub fn goal_get(&self, id: &str) -> Result<Option<GoalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, created_at, completed_at FROM goals WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(GoalRow {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                completed_at: row.get(4)?,
            })),
            None => Ok(None),
        }
    }

    /// List goals filtered by `status`.
    pub fn goals_by_status(&self, status: &str) -> Result<Vec<GoalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, created_at, completed_at
             FROM goals WHERE status = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![status], |row| {
            Ok(GoalRow {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                completed_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Mark a goal as done, setting `completed_at` to now.
    /// Returns `true` if the goal existed and was updated.
    pub fn goal_complete(&self, id: &str) -> Result<bool> {
        let now = now_utc();
        let n = self.conn.execute(
            "UPDATE goals SET status = 'done', completed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(n > 0)
    }
}
