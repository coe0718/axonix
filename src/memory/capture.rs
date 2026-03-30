//! Session memory extraction (Layer 1 — Capture).
//!
//! At session end, reads the session log and calls Haiku once to extract
//! structured memories. Only memories with importance >= 0.5 are stored.

use crate::db::AxonixDb;
use std::path::Path;

/// A single extracted memory, as returned by the Haiku extraction call.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ExtractedMemory {
    pub content: String,
    pub summary: String,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    pub importance: f64,
}

/// Result of an extraction pass.
#[derive(Debug)]
pub struct ExtractionResult {
    pub stored: usize,
    pub skipped: usize,
}

/// Read `log_path`, extract memories via Haiku, store hot memories in the DB.
///
/// Returns an ExtractionResult with counts of stored and skipped memories.
/// If ANTHROPIC_API_KEY is not set, returns Ok with 0 stored.
pub async fn extract_and_store(log_path: &Path, db_path: &Path) -> Result<ExtractionResult, String> {
    let api_key = match std::env::var("ANTHROPIC_API_KEY").or_else(|_| std::env::var("API_KEY")) {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("  memory capture: no API key set, skipping extraction");
            return Ok(ExtractionResult { stored: 0, skipped: 0 });
        }
    };

    let log_content = match std::fs::read_to_string(log_path) {
        Ok(s) => s,
        Err(e) => return Err(format!("cannot read log file {:?}: {e}", log_path)),
    };

    if log_content.trim().is_empty() {
        return Ok(ExtractionResult { stored: 0, skipped: 0 });
    }

    // Truncate to last 12000 chars to fit within Haiku's context
    let log_snippet: String = if log_content.len() > 12000 {
        log_content[log_content.len() - 12000..].to_string()
    } else {
        log_content
    };

    let prompt = format!(
        r#"You are analyzing a session log from an AI coding agent. Extract durable facts worth remembering.

SESSION LOG (last portion):
{log_snippet}

Extract memories from this session. For each memory, output ONLY valid JSON in this exact format:
{{
  "memories": [
    {{
      "content": "Full description of the fact or learning",
      "summary": "Short 5-10 word label",
      "entities": ["entity1", "entity2"],
      "topics": ["topic1", "topic2"],
      "importance": 0.8
    }}
  ]
}}

Rules:
- importance 0.0-1.0: operator preferences/decisions = 0.9+, technical facts = 0.7, observations = 0.5, trivial = 0.2
- Only include memories with importance >= 0.5 (the caller will filter, but prefer not to generate low-importance ones)
- Maximum 10 memories
- If nothing meaningful happened, return {{"memories": []}}
- Output ONLY the JSON object, no other text"#
    );

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 2048,
        "messages": [{"role": "user", "content": prompt}]
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {status}: {text}"));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse API response: {e}"))?;

    let text = resp_json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    let parsed: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse extraction JSON: {e}\nResponse was: {text}"))?;

    let memories: Vec<ExtractedMemory> = match parsed["memories"].as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect(),
        None => return Ok(ExtractionResult { stored: 0, skipped: 0 }),
    };

    let db = AxonixDb::open(db_path).map_err(|e| format!("cannot open DB: {e}"))?;

    let mut stored = 0usize;
    let mut skipped = 0usize;

    for mem in memories {
        if mem.importance < 0.5 {
            skipped += 1;
            continue;
        }
        let entities_json = serde_json::to_string(&mem.entities).unwrap_or_else(|_| "[]".to_string());
        let topics_json = serde_json::to_string(&mem.topics).unwrap_or_else(|_| "[]".to_string());

        match db.hot_memory_insert(&mem.content, &mem.summary, &entities_json, &topics_json, mem.importance) {
            Ok(_) => stored += 1,
            Err(e) => eprintln!("  memory capture: failed to store memory: {e}"),
        }
    }

    Ok(ExtractionResult { stored, skipped })
}
