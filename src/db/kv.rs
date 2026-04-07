use rusqlite::{Result, params};
use super::AxonixDb;
use super::helpers::now_utc;

impl AxonixDb {
    /// Upsert a key-value pair.
    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "INSERT OR REPLACE INTO kv (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, now],
        )?;
        Ok(())
    }

    /// Get a value by key. Returns `None` if the key does not exist.
    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM kv WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Delete a key. Returns `true` if the key existed and was removed.
    pub fn kv_delete(&self, key: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM kv WHERE key = ?1", params![key])?;
        Ok(n > 0)
    }

    /// List all key-value pairs as `(key, value)` tuples.
    pub fn kv_list(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM kv ORDER BY key")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }
}
