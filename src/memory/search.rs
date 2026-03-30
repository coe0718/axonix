//! Unified memory search (Layer 4 — Search).
//!
//! Runs keyword search over hot_memories, cold_memories, and observations
//! tables, combines results with importance weighting, returns top-N.

use crate::db::AxonixDb;
use std::path::Path;

/// A single search result from any memory tier.
#[derive(Debug, Clone)]
pub struct MemoryResult {
    pub content: String,
    pub source: MemorySource,
    pub importance: f64,
    pub score: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemorySource {
    Hot,
    Cold,
    Observation,
}

impl MemorySource {
    pub fn label(&self) -> &'static str {
        match self {
            MemorySource::Hot => "recent",
            MemorySource::Cold => "learned",
            MemorySource::Observation => "observed",
        }
    }
}

/// Search all memory tiers for `query`, return top `limit` results.
///
/// Scoring: keyword_score * importance * tier_weight
/// - Cold memories get a 1.5x boost (they represent synthesized patterns)
/// - Hot memories get 1.0x
/// - Observations get 0.8x (raw journal extracts, lower signal density)
///
/// Hard cap: `limit` results maximum. If `limit` is 0, returns empty vec.
pub fn memory_search(db_path: &Path, query: &str, limit: usize) -> Vec<MemoryResult> {
    if limit == 0 || query.trim().is_empty() {
        return vec![];
    }

    let db = match AxonixDb::open(db_path) {
        Ok(d) => d,
        Err(_) => return vec![],
    };

    let mut results: Vec<MemoryResult> = Vec::new();

    // Hot memories
    if let Ok(rows) = db.hot_memory_search(query, limit * 2) {
        for row in rows {
            if row.importance >= 0.5 {
                results.push(MemoryResult {
                    content: format!("[{}] {}", row.summary, row.content),
                    source: MemorySource::Hot,
                    importance: row.importance,
                    score: row.importance * 1.0,
                    created_at: row.created_at,
                });
            }
        }
    }

    // Cold memories
    if let Ok(rows) = db.cold_memory_search(query, limit * 2) {
        for row in rows {
            results.push(MemoryResult {
                content: row.content,
                source: MemorySource::Cold,
                importance: row.importance,
                score: row.importance * 1.5,
                created_at: row.created_at,
            });
        }
    }

    // Observations
    if let Ok(rows) = db.search_memory(query, limit * 2) {
        for row in rows {
            results.push(MemoryResult {
                content: row.text,
                source: MemorySource::Observation,
                importance: 0.5,
                score: row.score * 0.8,
                created_at: row.created_at,
            });
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    results
}
