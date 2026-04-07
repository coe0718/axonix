use rusqlite::{Result, params};
use super::AxonixDb;
use super::helpers::now_utc;

impl AxonixDb {
    /// Store an embedding for an observation identified by its unique key.
    /// Uses INSERT OR REPLACE to update if already exists.
    pub fn embedding_store(&self, obs_key: &str, vector: &[f32]) -> Result<()> {
        let now = now_utc();
        let blob = crate::embeddings::serialize_vec(vector);
        self.conn.execute(
            "INSERT OR REPLACE INTO embeddings (obs_key, vector, created_at)
             VALUES (?1, ?2, ?3)",
            params![obs_key, blob, now],
        )?;
        Ok(())
    }

    /// Return all (obs_key, vector) rows from the embeddings table.
    pub fn embeddings_list_by_key(&self) -> Result<Vec<(String, Vec<f32>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT obs_key, vector FROM embeddings")?;
        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((key, blob))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (key, blob) = row?;
            let vec = crate::embeddings::deserialize_vec(&blob);
            result.push((key, vec));
        }
        Ok(result)
    }
}
