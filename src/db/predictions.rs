use rusqlite::{Result, params};
use super::AxonixDb;

impl AxonixDb {
    /// Upsert a prediction row. Called on every predict/resolve.
    pub fn prediction_upsert(
        &self,
        id: &str,
        prediction: &str,
        created: &str,
        outcome: Option<&str>,
        delta: Option<&str>,
        resolved: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO predictions (id, prediction, created, outcome, delta, resolved)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               prediction = excluded.prediction,
               created    = excluded.created,
               outcome    = excluded.outcome,
               delta      = excluded.delta,
               resolved   = excluded.resolved",
            params![id, prediction, created, outcome, delta, resolved],
        )?;
        Ok(())
    }

    /// List all predictions ordered by id (numeric sort by casting to int).
    ///
    /// Returns raw tuples of `(id, prediction_text, created, outcome, delta, resolved)`
    /// to avoid a circular dependency between `db` and `predictions` modules.
    pub fn predictions_list(
        &self,
    ) -> Result<Vec<(String, String, String, Option<String>, Option<String>, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, prediction, created, outcome, delta, resolved
             FROM predictions
             ORDER BY CAST(id AS INTEGER)",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.collect()
    }
}
