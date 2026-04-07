use rusqlite::{Result, params};
use super::AxonixDb;
use super::helpers::{now_utc, add_days_to_utc, epoch_to_ymd_hms, tokenize};
use super::types::HotMemoryRow;

impl AxonixDb {
    /// Insert a hot memory. expires_at is set to 30 days from now.
    pub fn hot_memory_insert(
        &self,
        content: &str,
        summary: &str,
        entities: &str,
        topics: &str,
        importance: f64,
    ) -> Result<i64> {
        let now = now_utc();
        let expires_at = add_days_to_utc(&now, 30);
        self.conn.execute(
            "INSERT INTO hot_memories
             (content, summary, entities, topics, importance, created_at, last_accessed, access_count, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)",
            params![content, summary, entities, topics, importance, now, now, expires_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return up to `limit` hot memories, most recent first.
    pub fn hot_memories_list(&self, limit: usize) -> Result<Vec<HotMemoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, summary, entities, topics, importance,
                    created_at, last_accessed, access_count, expires_at
             FROM hot_memories
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(HotMemoryRow {
                id: row.get(0)?,
                content: row.get(1)?,
                summary: row.get(2)?,
                entities: row.get(3)?,
                topics: row.get(4)?,
                importance: row.get(5)?,
                created_at: row.get(6)?,
                last_accessed: row.get(7)?,
                access_count: row.get(8)?,
                expires_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    /// Keyword search over hot memories (content + summary + topics).
    pub fn hot_memory_search(&self, query: &str, limit: usize) -> Result<Vec<HotMemoryRow>> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || limit == 0 {
            return Ok(vec![]);
        }
        let all = self.hot_memories_list(usize::MAX)?;
        let mut scored: Vec<(f64, HotMemoryRow)> = all
            .into_iter()
            .filter_map(|row| {
                let content_tokens = tokenize(&row.content);
                let summary_tokens = tokenize(&row.summary);
                let topic_tokens = tokenize(&row.topics);
                let mut score = 0.0f64;
                for qt in &query_tokens {
                    if content_tokens.contains(qt) { score += 1.0; }
                    if summary_tokens.contains(qt) { score += 1.5; }
                    if topic_tokens.contains(qt) { score += 2.0; }
                }
                if score > 0.0 { Some((score, row)) } else { None }
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, row)| row).collect())
    }

    /// Update last_accessed to now and increment access_count for a hot memory.
    pub fn hot_memory_touch(&self, id: i64) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "UPDATE hot_memories SET last_accessed = ?1, access_count = access_count + 1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// Return hot memories created more than 7 days ago (candidates for consolidation).
    pub fn hot_memories_consolidatable(&self) -> Result<Vec<HotMemoryRow>> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cutoff_secs = secs.saturating_sub(7 * 86400);
        let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(cutoff_secs);
        let cutoff = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z");
        let mut stmt = self.conn.prepare(
            "SELECT id, content, summary, entities, topics, importance,
                    created_at, last_accessed, access_count, expires_at
             FROM hot_memories
             WHERE created_at < ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![cutoff], |row| {
            Ok(HotMemoryRow {
                id: row.get(0)?,
                content: row.get(1)?,
                summary: row.get(2)?,
                entities: row.get(3)?,
                topics: row.get(4)?,
                importance: row.get(5)?,
                created_at: row.get(6)?,
                last_accessed: row.get(7)?,
                access_count: row.get(8)?,
                expires_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    /// Delete a hot memory by id. Returns true if the row existed and was deleted.
    pub fn hot_memory_delete(&self, id: i64) -> Result<bool> {
        let n = self.conn.execute("DELETE FROM hot_memories WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }
}
