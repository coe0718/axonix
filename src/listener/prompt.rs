//! System prompt builder and history formatting for the listener agent.

use crate::conversation_memory::ConversationMemory;
use yoagent::agent::Agent;
use yoagent::provider::AnthropicProvider;
use yoagent::tools::default_tools;
use yoagent::context::ContextConfig;
use yoagent::retry::RetryConfig;

/// Build the system prompt for the listener's short-context agent.
///
/// The prompt is focused on "be helpful now" — not self-improvement.
/// It includes recent conversation turns from [`ConversationMemory`] so the
/// agent has context about what was discussed without a full session history.
///
/// # Design goals
///
/// - **Concise**: listener responses should fit in a Telegram message (~3000 chars max).
/// - **Helpful**: focus on answering the question, not on agent self-improvement.
/// - **Context-aware**: inject recent conversation turns so the agent isn't starting cold.
pub fn build_listener_system_prompt(
    memory: &ConversationMemory,
    active_goal: Option<&str>,
    memory_context: &[(String, f64)],
) -> String {
    let mut parts = vec![
        "You are Axonix, a personal assistant running as an always-on listener.".to_string(),
        String::new(),
        "## Your role right now".to_string(),
        "Answer the operator's question helpfully and concisely.".to_string(),
        "Keep responses short — they will be sent as Telegram messages.".to_string(),
        "Aim for 1-3 short paragraphs or a brief bullet list. Avoid lengthy preamble.".to_string(),
        String::new(),
        "## What you are NOT doing right now".to_string(),
        "- You are not running a self-improvement session.".to_string(),
        "- You are not writing or committing code (unless explicitly asked).".to_string(),
        "- Self-improvement sessions run separately via evolve.sh in the background.".to_string(),
        String::new(),
        "## Response guidelines".to_string(),
        "- Be direct: answer the question first, explain second.".to_string(),
        "- Use markdown sparingly — Telegram renders basic formatting.".to_string(),
        "- If you don't know something, say so clearly rather than guessing.".to_string(),
        "- If a task needs a full session (code changes, file writes), say so.".to_string(),
    ];

    // Inject active goal context if present
    if let Some(goal) = active_goal {
        parts.push(String::new());
        parts.push("## Current active goal".to_string());
        parts.push(goal.to_string());
    }

    // Inject top memory observations if present
    if !memory_context.is_empty() {
        parts.push(String::new());
        parts.push("## Relevant past observations".to_string());
        for (text, score) in memory_context {
            // Extract tag from text if present in format "text (tag: X)"
            parts.push(format!("- [{score:.2}] {text}"));
        }
    }

    // Inject recent conversation context if available
    let context = memory.format_for_context(10);
    if !context.is_empty() {
        parts.push(String::new());
        parts.push(context);
    }

    parts.join("\n")
}

/// Format the last `n` conversation turns from memory as a Telegram-ready string.
///
/// Each turn is formatted as:
///   #N You: <text truncated to 100 chars>
///   #N Axonix: <text truncated to 100 chars>
///
/// Returns a placeholder message if there are no turns.
pub fn format_history_reply(mem: &ConversationMemory, n: usize) -> String {
    if mem.turns.is_empty() {
        return "📜 No conversation history yet.".to_string();
    }
    let turns = &mem.turns;
    let start = turns.len().saturating_sub(n);
    let recent = &turns[start..];
    let total = turns.len();
    let mut lines = vec![format!(
        "📜 Last {} turn{} (of {total} total):",
        recent.len(),
        if recent.len() == 1 { "" } else { "s" }
    )];
    for (i, turn) in recent.iter().enumerate() {
        let turn_num = start + i + 1;
        let label = if turn.role == "user" { "You" } else { "Axonix" };
        let text = if turn.text.chars().count() > 100 {
            let truncated: String = turn.text.chars().take(100).collect();
            format!("{truncated}…")
        } else {
            turn.text.clone()
        };
        lines.push(format!("#{turn_num} {label}: {text}"));
    }
    lines.join("\n")
}

// ── Listener agent builder ────────────────────────────────────────────────────

pub(crate) fn make_listener_agent(api_key: &str, model: &str, system_prompt: &str) -> Agent {
    Agent::new(AnthropicProvider)
        .with_system_prompt(system_prompt)
        .with_model(model)
        .with_api_key(api_key)
        .with_tools(default_tools())
        .with_context_config(ContextConfig {
            max_context_tokens: 80_000,
            system_prompt_tokens: 4_000,
            keep_recent: 10,
            keep_first: 2,
            tool_output_max_lines: 40,
        })
        .with_retry_config(RetryConfig {
            max_retries: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 30_000,
        })
}

/// Get the last git commit message (first line only).
/// Returns None on any error.
pub(crate) fn get_last_commit_message() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["show", "--no-patch", "--format=%s", "HEAD"])
        .output()
        .ok()?;
    if output.status.success() {
        let msg = String::from_utf8_lossy(&output.stdout);
        Some(msg.trim().lines().next().unwrap_or("").to_string())
    } else {
        None
    }
}
