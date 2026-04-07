use rusqlite::{Result, params};
use super::AxonixDb;
use super::helpers::{now_utc, add_days_to_utc, tokenize};
use super::types::{ColdMemoryRow, ContradictionRow, ObservationRow};

impl AxonixDb {
    /// Insert a cold memory. expires_at is set to 90 days from now.
    pub fn cold_memory_insert(&self, content: &str, topics: &str, importance: f64) -> Result<i64> {
        let now = now_utc();
        let expires_at = add_days_to_utc(&now, 90);
        self.conn.execute(
            "INSERT INTO cold_memories
             (content, topics, importance, created_at, reinforcement_count, last_reinforced, expires_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
            params![content, topics, importance, now, now, expires_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return up to `limit` cold memories, most recent first.
    pub fn cold_memories_list(&self, limit: usize) -> Result<Vec<ColdMemoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, topics, importance, created_at,
                    reinforcement_count, last_reinforced, expires_at
             FROM cold_memories
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ColdMemoryRow {
                id: row.get(0)?,
                content: row.get(1)?,
                topics: row.get(2)?,
                importance: row.get(3)?,
                created_at: row.get(4)?,
                reinforcement_count: row.get(5)?,
                last_reinforced: row.get(6)?,
                expires_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    /// Keyword search over cold memories (content + topics).
    pub fn cold_memory_search(&self, query: &str, limit: usize) -> Result<Vec<ColdMemoryRow>> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || limit == 0 {
            return Ok(vec![]);
        }
        let all = self.cold_memories_list(usize::MAX)?;
        let mut scored: Vec<(f64, ColdMemoryRow)> = all
            .into_iter()
            .filter_map(|row| {
                let content_tokens = tokenize(&row.content);
                let topic_tokens = tokenize(&row.topics);
                let mut score = 0.0f64;
                for qt in &query_tokens {
                    if content_tokens.contains(qt) { score += 1.0; }
                    if topic_tokens.contains(qt) { score += 2.0; }
                }
                if score > 0.0 { Some((score, row)) } else { None }
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, row)| row).collect())
    }

    /// Increment reinforcement_count, update last_reinforced, and extend expires_at by 90 days.
    pub fn cold_memory_reinforce(&self, id: i64) -> Result<()> {
        let now = now_utc();
        let new_expires = add_days_to_utc(&now, 90);
        self.conn.execute(
            "UPDATE cold_memories
             SET reinforcement_count = reinforcement_count + 1,
                 last_reinforced = ?1,
                 expires_at = ?2
             WHERE id = ?3",
            params![now, new_expires, id],
        )?;
        Ok(())
    }

    /// Delete a cold memory by id. Returns true if the row existed and was deleted.
    pub fn cold_memory_delete(&self, id: i64) -> Result<bool> {
        let n = self.conn.execute("DELETE FROM cold_memories WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    // ─── Contradiction helpers ────────────────────────────────────────────────

    /// Record a new contradiction between an existing cold memory and a new memory.
    pub fn contradiction_insert(&self, cold_memory_id: i64, new_memory: &str) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "INSERT INTO memory_contradictions (cold_memory_id, new_memory, created_at, resolved)
             VALUES (?1, ?2, ?3, 0)",
            params![cold_memory_id, new_memory, now],
        )?;
        Ok(())
    }

    /// Return all unresolved contradictions.
    pub fn contradictions_open(&self) -> Result<Vec<ContradictionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, cold_memory_id, new_memory, created_at, resolved
             FROM memory_contradictions
             WHERE resolved = 0
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let resolved_int: i64 = row.get(4)?;
            Ok(ContradictionRow {
                id: row.get(0)?,
                cold_memory_id: row.get(1)?,
                new_memory: row.get(2)?,
                created_at: row.get(3)?,
                resolved: resolved_int != 0,
            })
        })?;
        rows.collect()
    }

    // ─── Semantic search ──────────────────────────────────────────────────────

    /// Semantic search using Ollama embeddings. Returns top-`limit` observations
    /// ordered by cosine similarity to `query`.
    ///
    /// Gracefully returns an empty vec if Ollama is unavailable or no embeddings
    /// are stored yet — never panics.
    pub fn semantic_search_memory(&self, query: &str, limit: usize) -> Result<Vec<ObservationRow>> {
        if limit == 0 || query.is_empty() {
            return Ok(vec![]);
        }
        // Get query embedding — graceful fallback if Ollama is down
        let query_vec = match crate::embeddings::embed(query) {
            Ok(v) => v,
            Err(_) => return Ok(vec![]),
        };
        // Get all stored embeddings
        let all_embeddings = self.embeddings_list_by_key()?;
        if all_embeddings.is_empty() {
            return Ok(vec![]);
        }
        // Score each stored embedding against the query vector
        let mut scored: Vec<(String, f32)> = all_embeddings
            .into_iter()
            .map(|(key, vec)| {
                let sim = crate::embeddings::cosine_similarity(&query_vec, &vec);
                (key, sim)
            })
            .filter(|(_, sim)| *sim > 0.01)
            .collect();
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);

        // Fetch matching observations from DB
        let mut result = Vec::new();
        for (key, sim) in scored {
            let mut stmt = self.conn.prepare(
                "SELECT id, key, text, tags, created_at FROM observations WHERE key = ?1",
            )?;
            let rows: Vec<ObservationRow> = stmt
                .query_map(params![key], |row| {
                    Ok(ObservationRow {
                        id: row.get(0)?,
                        key: row.get(1)?,
                        text: row.get(2)?,
                        tags: row.get(3)?,
                        created_at: row.get(4)?,
                        score: 0.0,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            for mut row in rows {
                row.score = sim as f64;
                result.push(row);
            }
        }
        Ok(result)
    }
}
