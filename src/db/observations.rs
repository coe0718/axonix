use rusqlite::{Result, params};
use super::AxonixDb;
use super::helpers::{now_utc, tokenize};
use super::types::{ObservationRow, StructuredObservation};

impl AxonixDb {
    /// Store (insert or replace) an observation by key.
    /// `tags` is a comma-separated list of topic tags (e.g. `"repl,slash-command,bug"`).
    ///
    /// After storing, best-effort embeds the text via Ollama for semantic search.
    /// If Ollama is unavailable, the observation is still stored — embedding is skipped.
    pub fn observation_store(&self, key: &str, text: &str, tags: &str) -> Result<()> {
        let now = now_utc();
        self.conn.execute(
            "INSERT INTO observations (key, text, tags, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key) DO UPDATE SET
               text       = excluded.text,
               tags       = excluded.tags",
            // created_at intentionally NOT updated — preserve original timestamp
            params![key, text, tags, now],
        )?;
        // Best-effort: embed and store vector for semantic search
        if let Ok(vec) = crate::embeddings::embed(text) {
            let _ = self.embedding_store(key, &vec);
        }
        Ok(())
    }

    /// Return up to `limit` observations most relevant to `query`.
    ///
    /// Uses keyword overlap scoring:
    /// - tag token matches contribute 2.0 each
    /// - text token matches contribute 1.0 each
    ///
    /// Only rows with score > 0 are returned.  Tie-breaks by most-recently
    /// created first.
    pub fn search_memory(&self, query: &str, limit: usize) -> Result<Vec<ObservationRow>> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        // Fetch all observations
        let all = self.observations_list(usize::MAX)?;

        // Score each row
        let mut scored: Vec<ObservationRow> = all
            .into_iter()
            .filter_map(|mut row| {
                let text_tokens = tokenize(&row.text);
                let tag_tokens = tokenize(&row.tags);
                let mut score = 0.0f64;
                for qt in &query_tokens {
                    if text_tokens.contains(qt) {
                        score += 1.0;
                    }
                    if tag_tokens.contains(qt) {
                        score += 2.0;
                    }
                }
                if score > 0.0 {
                    row.score = score;
                    Some(row)
                } else {
                    None
                }
            })
            .collect();

        // Sort: score DESC, then created_at DESC
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.created_at.cmp(&a.created_at))
        });

        scored.truncate(limit);
        Ok(scored)
    }

    /// Parse `journal_path` (a `JOURNAL.md`-format file) and upsert each `## ` section
    /// as an observation.
    ///
    /// Key  = `journal:` + lowercase slug of the heading, truncated to 80 chars.
    /// Tags = `"journal"`.
    /// Text = heading line + `"\n"` + body, truncated to 800 chars.
    ///
    /// The operation is idempotent: calling it twice on the same file produces
    /// the same DB state (upsert by key).
    ///
    /// Returns the number of entries stored (including upserted duplicates).
    pub fn seed_from_journal(&self, journal_path: &std::path::Path) -> Result<usize> {
        let content = match std::fs::read_to_string(journal_path) {
            Ok(s) => s,
            Err(e) => {
                return Err(rusqlite::Error::InvalidParameterName(
                    format!("cannot read journal: {e}"),
                ));
            }
        };

        // Split on "\n## " to get sections (first chunk may be a preamble).
        // We also handle a file that starts with "## " directly.
        let raw_sections: Vec<&str> = content.split("\n## ").collect();

        let mut count = 0usize;
        for (i, section) in raw_sections.iter().enumerate() {
            // The very first chunk from split("\n## ") still has its "## " prefix
            // only if the file starts with "## " — otherwise the first chunk is
            // whatever precedes the first "## " heading (e.g. "# Journal\n\n").
            let section_trimmed = if i == 0 {
                // Strip a possible leading "## " if the file starts with it.
                section.strip_prefix("## ").unwrap_or(section)
            } else {
                section
            };

            // First line is the heading; the rest is the body.
            let mut lines = section_trimmed.splitn(2, '\n');
            let heading = lines.next().unwrap_or("").trim();
            let body = lines.next().unwrap_or("").trim();

            // Skip sections with an empty heading (e.g. the preamble before the first "## ")
            // or preamble sections starting with a title-level "#" heading.
            if heading.is_empty() || heading.starts_with('#') {
                continue;
            }

            // Build the key slug: lowercase, non-alphanumeric → '-', truncate to 80 chars.
            let slug: String = heading
                .chars()
                .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
                .collect::<String>()
                .chars()
                .take(80)
                .collect();
            let key = format!("journal:{slug}");

            // Build the text: heading + newline + body, truncated to 800 chars.
            let combined = format!("{heading}\n{body}");
            let text: String = combined.chars().take(800).collect();

            self.observation_store(&key, &text, "journal")?;
            count += 1;
        }

        Ok(count)
    }

    /// Return all observations, most recent first.
    pub fn observations_list(&self, limit: usize) -> Result<Vec<ObservationRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, key, text, tags, created_at
             FROM observations
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ObservationRow {
                id: row.get(0)?,
                key: row.get(1)?,
                text: row.get(2)?,
                tags: row.get(3)?,
                created_at: row.get(4)?,
                score: 0.0,
            })
        })?;
        rows.collect()
    }

    /// Valid categories for structured observations.
    /// If the supplied category is not in this list, "learned" is used.
    const VALID_CATEGORIES: &'static [&'static str] = &[
        "learned",
        "tried_and_failed",
        "blocked_by",
        "dependency_discovered",
        "pattern_noticed",
    ];

    /// Insert a structured observation and return its new row id.
    pub fn sobs_insert(
        &self,
        content: &str,
        category: &str,
        source_file: &str,
        goal_id: &str,
        session: &str,
        tags: &str,
    ) -> Result<i64> {
        let effective_category = if Self::VALID_CATEGORIES.contains(&category) {
            category
        } else {
            "learned"
        };
        let now = now_utc();
        self.conn.execute(
            "INSERT INTO structured_observations
             (content, category, source_file, goal_id, session, tags, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![content, effective_category, source_file, goal_id, session, tags, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Search structured observations by keyword match on content + tags.
    /// Returns up to `limit` results ordered by relevance then created_at DESC.
    /// Only rows with score > 0 are returned.
    pub fn sobs_search(&self, query: &str, limit: usize) -> Result<Vec<StructuredObservation>> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        let all = self.sobs_list(usize::MAX)?;
        let mut scored: Vec<(f64, StructuredObservation)> = all
            .into_iter()
            .filter_map(|row| {
                let content_tokens = tokenize(&row.content);
                let tag_tokens = tokenize(&row.tags);
                let mut score = 0.0f64;
                for qt in &query_tokens {
                    if content_tokens.contains(qt) {
                        score += 1.0;
                    }
                    if tag_tokens.contains(qt) {
                        score += 2.0;
                    }
                }
                if score > 0.0 { Some((score, row)) } else { None }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.1.created_at.cmp(&a.1.created_at))
        });
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, row)| row).collect())
    }

    /// List structured observations, most recent first, up to `limit`.
    pub fn sobs_list(&self, limit: usize) -> Result<Vec<StructuredObservation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, category, source_file, goal_id, session, tags, created_at
             FROM structured_observations
             ORDER BY created_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(StructuredObservation {
                id: row.get(0)?,
                content: row.get(1)?,
                category: row.get(2)?,
                source_file: row.get(3)?,
                goal_id: row.get(4)?,
                session: row.get(5)?,
                tags: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }
}
