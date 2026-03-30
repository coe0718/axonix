//! Memory consolidator (Layer 3 — Reflection).
//!
//! Runs once daily. Finds hot memories older than 7 days, groups them
//! by topic, and synthesizes each group into a cold memory via Haiku.

use crate::db::{AxonixDb, HotMemoryRow};
use std::collections::HashMap;
use std::path::Path;

/// Result of a consolidation run.
#[derive(Debug)]
pub struct ConsolidationResult {
    pub groups_processed: usize,
    pub cold_memories_created: usize,
    pub hot_memories_deleted: usize,
}

/// Check whether consolidation should run (any hot memories older than 7 days).
pub fn needs_consolidation(db_path: &Path) -> bool {
    let db = match AxonixDb::open(db_path) {
        Ok(d) => d,
        Err(_) => return false,
    };
    matches!(db.hot_memories_consolidatable(), Ok(rows) if !rows.is_empty())
}

/// Run consolidation: group old hot memories by topic, synthesize cold memories.
///
/// Each group is passed to Haiku to synthesize a single cold memory.
/// If the synthesized memory conflicts with an existing cold memory, a contradiction is recorded.
pub async fn consolidate(db_path: &Path) -> Result<ConsolidationResult, String> {
    let api_key = match std::env::var("ANTHROPIC_API_KEY").or_else(|_| std::env::var("API_KEY")) {
        Ok(k) if !k.is_empty() => k,
        _ => return Err("no API key set".to_string()),
    };

    let db = AxonixDb::open(db_path).map_err(|e| format!("cannot open DB: {e}"))?;
    let old_hot = db.hot_memories_consolidatable().map_err(|e| format!("DB error: {e}"))?;

    if old_hot.is_empty() {
        return Ok(ConsolidationResult {
            groups_processed: 0,
            cold_memories_created: 0,
            hot_memories_deleted: 0,
        });
    }

    // Group by primary topic (first element of topics JSON array, or "general")
    let mut groups: HashMap<String, Vec<HotMemoryRow>> = HashMap::new();
    for row in old_hot {
        let topic = parse_first_topic(&row.topics);
        groups.entry(topic).or_default().push(row);
    }

    let client = reqwest::Client::new();
    let mut cold_memories_created = 0usize;
    let mut hot_memories_deleted = 0usize;

    for (topic, rows) in &groups {
        if rows.len() < 2 {
            // Single hot memory in group — don't synthesize, just let it age
            continue;
        }

        let entries: Vec<String> = rows.iter().map(|r| r.content.clone()).collect();
        let prompt = format!(
            "Synthesize these related memories about '{}' into a single concise cold memory.\n\nMemories:\n- {}\n\nOutput a single sentence or two that captures the durable insight. No JSON, just the synthesized memory text.",
            topic,
            entries.join("\n- ")
        );

        let body = serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 256,
            "messages": [{"role": "user", "content": prompt}]
        });

        let resp = match client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  consolidator: API error for topic '{topic}': {e}");
                continue;
            }
        };

        if !resp.status().is_success() {
            eprintln!("  consolidator: API error {}", resp.status());
            continue;
        }

        let resp_json: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };

        let synthesized = resp_json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        if synthesized.is_empty() {
            continue;
        }

        let topics_json = serde_json::to_string(&[topic.as_str()]).unwrap_or_else(|_| "[]".to_string());
        let avg_importance = rows.iter().map(|r| r.importance).sum::<f64>() / rows.len() as f64;

        // Check for contradiction with existing cold memories
        if let Ok(cold_rows) = db.cold_memory_search(&synthesized, 3) {
            for cold in cold_rows {
                // Simple heuristic: if the topics overlap, flag a potential contradiction
                if cold.topics.contains(topic.as_str()) {
                    let _ = db.contradiction_insert(cold.id, &synthesized);
                }
            }
        }

        match db.cold_memory_insert(&synthesized, &topics_json, avg_importance) {
            Ok(_) => {
                cold_memories_created += 1;
                // Delete the hot memories that were consolidated
                for row in rows {
                    if db.hot_memory_delete(row.id).unwrap_or(false) {
                        hot_memories_deleted += 1;
                    }
                }
            }
            Err(e) => eprintln!("  consolidator: failed to insert cold memory: {e}"),
        }
    }

    Ok(ConsolidationResult {
        groups_processed: groups.len(),
        cold_memories_created,
        hot_memories_deleted,
    })
}

fn parse_first_topic(topics_json: &str) -> String {
    serde_json::from_str::<Vec<String>>(topics_json)
        .ok()
        .and_then(|v| v.into_iter().next())
        .unwrap_or_else(|| "general".to_string())
}
