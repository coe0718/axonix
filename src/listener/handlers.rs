//! Command handler helpers for the Telegram listener.
//!
//! Pure functions called from the main poll loop in `run.rs`.
//! These handle goal appending, predictions, and mini-session execution.

use yoagent::agent::Agent;
use yoagent::provider::AnthropicProvider;
use yoagent::tools::default_tools;
use yoagent::context::ContextConfig;
use yoagent::retry::RetryConfig;
use yoagent::{AgentEvent, StreamDelta};

/// Append a new goal to the default GOALS.md.
pub(super) fn append_goal_to_backlog(description: &str) -> Result<(), String> {
    append_goal_to_backlog_at(description, std::path::Path::new("GOALS.md"))
}

/// Append a new goal to a GOALS.md file at the given path.
///
/// Adds a minimal entry under the `## Backlog` heading.
/// Returns Ok(()) if written, Err with message on failure.
pub(crate) fn append_goal_to_backlog_at(description: &str, path: &std::path::Path) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read GOALS.md: {e}"))?;

    if !content.contains("## Backlog") {
        return Err("GOALS.md has no ## Backlog section".to_string());
    }

    // Generate a simple ID based on timestamp (last 4 digits of epoch seconds)
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() % 10000)
        .unwrap_or(0);

    // Get date from system
    let date = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let entry = format!(
        "\n### G-TG{ts} — {description}\n\
        **Why:** Requested via Telegram on {date}.\n\
        **Definition of done:** TBD — refine in next session.\n",
    );

    // Insert after "## Backlog\n"
    let new_content = content.replacen(
        "## Backlog\n",
        &format!("## Backlog\n{entry}"),
        1,
    );

    std::fs::write(path, new_content)
        .map_err(|e| format!("cannot write GOALS.md: {e}"))?;

    Ok(())
}

/// Append a new prediction to `.axonix/predictions.json`.
///
/// Loads the existing store via `PredictionStore::default_path()`, calls `predict()`,
/// then `save()`. Returns the assigned prediction ID on success.
pub(super) fn append_prediction(text: &str) -> Result<u32, String> {
    let mut store = crate::predictions::PredictionStore::default_path();
    let id = store.predict(text);
    store.save()?;
    Ok(id)
}

/// Resolve a prediction by ID with a correct/wrong verdict.
/// Maps `true` → `"TRUE"` outcome, `false` → `"FALSE"` outcome.
/// Returns Ok(prediction_text) on success, Err(message) on failure.
pub(super) fn resolve_prediction(id: u32, correct: bool) -> Result<String, String> {
    let mut store = crate::predictions::PredictionStore::default_path();
    let outcome = if correct { "TRUE" } else { "FALSE" };
    let text = store.resolve(id, outcome, None)?;
    store.save().map_err(|e| format!("saved resolve but failed to persist: {e}"))?;
    Ok(text)
}

/// Compute prediction accuracy from `.axonix/predictions.json`.
///
/// Returns a formatted string like `"8/12 correct — 67%"` when there are resolved
/// predictions, or `None` if there are no resolved predictions.
///
/// Positive outcomes: `"correct"`, `"TRUE"`, `"EARLY"` (case-insensitive prefix match).
/// Negative outcomes: `"wrong"`, `"FALSE"`, `"PARTIAL"`, `"Expired"`, etc.
/// Skipped: `null` (unresolved).
pub(super) fn compute_prediction_accuracy() -> Option<String> {
    let store = crate::predictions::PredictionStore::default_path();
    let resolved = store.resolved();
    let total = resolved.len();
    if total == 0 {
        return None;
    }
    let correct = resolved.iter().filter(|(_, p)| {
        if let Some(outcome) = &p.outcome {
            let upper = outcome.to_uppercase();
            upper.starts_with("CORRECT") || upper.starts_with("TRUE") || upper.starts_with("EARLY")
        } else {
            false
        }
    }).count();
    let pct = (correct as f64 / total as f64 * 100.0).round() as u64;
    Some(format!("{correct}/{total} correct — {pct}%"))
}

/// Run a task as a mini sub-agent session and return the result text.
///
/// Uses a short-context agent (max 8 turns) focused on the given task.
pub(super) async fn run_mini_session(task: &str, api_key: &str, model: &str) -> Result<String, String> {
    let system_prompt = "You are Axonix, running a short focused task requested via Telegram.\n\
        Complete the task concisely. Keep your response under 2000 characters.\n\
        If you cannot complete the task in 8 turns, summarise what you did and what remains.";

    let mut agent = Agent::new(AnthropicProvider)
        .with_system_prompt(system_prompt)
        .with_model(model)
        .with_api_key(api_key)
        .with_tools(default_tools())
        .with_context_config(ContextConfig {
            max_context_tokens: 40_000,
            system_prompt_tokens: 2_000,
            keep_recent: 8,
            keep_first: 1,
            tool_output_max_lines: 20,
        })
        .with_retry_config(RetryConfig {
            max_retries: 2,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 15_000,
        });

    let mut rx = agent.prompt(task).await;
    let mut result = String::new();

    while let Some(event) = rx.recv().await {
        if let AgentEvent::MessageUpdate {
            delta: StreamDelta::Text { delta },
            ..
        } = event
        {
            result.push_str(&delta);
        }
    }

    if result.is_empty() {
        Err("mini-session produced no output".to_string())
    } else {
        Ok(result)
    }
}
