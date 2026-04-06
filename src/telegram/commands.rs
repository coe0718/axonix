//! Telegram command parsing functions and help text.

/// The help text shown to Telegram users.
///
/// Kept as a constant so the poll loop and tests share the same string.
pub const TELEGRAM_HELP_TEXT: &str = "\
*Axonix Bot* — Available commands:

/ask <prompt> — Send a prompt to the agent and get a response
/run <task> — Execute a task as a mini-session and report the result
/goal <description> — Add a goal to the backlog immediately
/predict <text> — Append a new prediction to .axonix/predictions.json
/predictions — List all open (unresolved) predictions with IDs
/resolve <id> correct|wrong — Mark a prediction as correct or wrong
/memory add <text>       — store an observation (optionally /memory add:<category>)
/memory search <query>   — search past observations
/memory list             — list recent observations
/status — Show current session status (model, mode, uptime)
/health — Show system health (CPU, memory, disk, uptime)
/brief — Morning brief: active goals, open predictions, recent sessions
/history — Show last 5 conversation turns
/goals — Show active goals and first backlog item
/help — Show this help message

*Examples:*
• /ask explain how async Rust works
• /ask what files are in /workspace/src?
• /run check disk usage
• /goal add dark mode to dashboard
• /predict the test count will exceed 1000 by Day 25
• /predictions
• /resolve 42 correct
• /resolve 43 wrong
• /status
• /health
• /brief
• /history
• /goals

Responses may take a moment depending on prompt complexity.";

/// Parse an `/ask <prompt>` command from a Telegram message text.
///
/// Accepts:
/// - `/ask <prompt>` — standard bot command format
/// - `ask: <prompt>` — natural language format
///
/// Returns the prompt text, or `None` if the message is not an ask command.
pub fn parse_ask_command(text: &str) -> Option<&str> {
    let text = text.trim();
    // Standard bot command: /ask <prompt>
    if let Some(rest) = text.strip_prefix("/ask") {
        let prompt = rest.trim();
        if !prompt.is_empty() {
            return Some(prompt);
        }
    }
    // Natural language: "ask: <prompt>"
    if let Some(rest) = text.strip_prefix("ask:") {
        let prompt = rest.trim();
        if !prompt.is_empty() {
            return Some(prompt);
        }
    }
    None
}

/// Check whether a Telegram message is addressed to Axonix
/// (starts with /ask, ask:, or is a direct reply to a bot message).
pub fn is_ask_command(text: &str) -> bool {
    parse_ask_command(text).is_some()
}

/// Check whether a Telegram message is a `/help` command.
pub fn is_help_command(text: &str) -> bool {
    matches!(text.trim(), "/help" | "/start")
}

/// Check whether a Telegram message is a `/status` command.
pub fn is_status_command(text: &str) -> bool {
    matches!(text.trim(), "/status")
}

/// Check whether a Telegram message is a `/health` command.
pub fn is_health_command(text: &str) -> bool {
    matches!(text.trim(), "/health")
}

/// Check whether a Telegram message is a `/brief` command.
pub fn is_brief_command(text: &str) -> bool {
    matches!(text.trim(), "/brief")
}

/// Check whether a Telegram message is a `/history` command.
pub fn is_history_command(text: &str) -> bool {
    matches!(text.trim(), "/history")
}

/// Check whether a Telegram message is a `/goals` command.
pub fn is_goals_command(text: &str) -> bool {
    matches!(text.trim(), "/goals")
}

/// Parse a `/run <task>` command. Returns the task text, or None.
pub fn parse_run_command(text: &str) -> Option<&str> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("/run") {
        let task = rest.trim();
        if !task.is_empty() {
            return Some(task);
        }
    }
    None
}

/// Parse a `/goal <description>` command. Returns the description, or None.
pub fn parse_goal_command(text: &str) -> Option<&str> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("/goal") {
        let desc = rest.trim();
        if !desc.is_empty() {
            return Some(desc);
        }
    }
    None
}

/// Check whether a Telegram message is a `/run <task>` command.
pub fn is_run_command(text: &str) -> bool {
    parse_run_command(text).is_some()
}

/// Check whether a Telegram message is a `/goal <description>` command.
pub fn is_goal_command(text: &str) -> bool {
    parse_goal_command(text).is_some()
}

/// Check whether a Telegram message is a `/predict <text>` command.
/// Note: this is distinct from `/predictions` (list command) and `/preds`.
pub fn parse_predict_command(text: &str) -> Option<&str> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("/predict") {
        // Must be followed by whitespace (not letters like "/predictions")
        if rest.starts_with(|c: char| c.is_whitespace()) {
            let pred_text = rest.trim();
            if !pred_text.is_empty() {
                return Some(pred_text);
            }
        }
    }
    None
}

/// Parse `/resolve <id> correct|wrong` or `/resolve <id> true|false`.
/// Returns `(id, verdict)` where verdict is true for correct/true, false for wrong/false.
/// Returns `None` if the command doesn't match or can't be parsed.
pub fn parse_resolve_command(text: &str) -> Option<(u32, bool)> {
    let lower = text.trim().to_lowercase();
    let rest = lower.strip_prefix("/resolve")?.trim_start().to_string();
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let mut parts = rest.splitn(2, char::is_whitespace);
    let id_str = parts.next()?.trim();
    let verdict_str = parts.next()?.trim();
    let id: u32 = id_str.parse().ok()?;
    let verdict = match verdict_str {
        "correct" | "true" | "yes" => true,
        "wrong" | "false" | "no" => false,
        _ => return None,
    };
    Some((id, verdict))
}

/// Parse a `/memory ...` command text into a `MemoryAction`.
///
/// Formats:
/// - `/memory add <text>`             → Add { category: "learned" }
/// - `/memory add:<category> <text>`  → Add { category }
/// - `/memory search <query>`         → Search { query }
/// - `/memory list`                   → List
///
/// Returns `None` if the text is not a `/memory` command or is malformed.
pub fn parse_memory_command(text: &str) -> Option<super::types::MemoryAction> {
    let text = text.trim();
    let rest = text.strip_prefix("/memory")?.trim_start();
    if rest.is_empty() {
        return None;
    }
    if rest == "list" {
        return Some(super::types::MemoryAction::List);
    }
    if let Some(search_rest) = rest.strip_prefix("search") {
        let query = search_rest.trim();
        if !query.is_empty() {
            return Some(super::types::MemoryAction::Search { query: query.to_string() });
        }
        return None;
    }
    if let Some(add_rest) = rest.strip_prefix("add") {
        // Could be "add <text>" or "add:<category> <text>"
        if let Some(cat_and_text) = add_rest.strip_prefix(':') {
            // "add:<category> <text>"
            let mut parts = cat_and_text.splitn(2, ' ');
            let category = parts.next().unwrap_or("learned").trim();
            let text_part = parts.next().unwrap_or("").trim();
            if !text_part.is_empty() {
                return Some(super::types::MemoryAction::Add {
                    text: text_part.to_string(),
                    category: category.to_string(),
                });
            }
            return None;
        } else {
            let body = add_rest.trim();
            if !body.is_empty() {
                return Some(super::types::MemoryAction::Add {
                    text: body.to_string(),
                    category: "learned".to_string(),
                });
            }
            return None;
        }
    }
    None
}

/// Check whether a Telegram message is a `/predictions` or `/preds` command.
pub fn is_list_predictions_command(text: &str) -> bool {
    matches!(text.trim(), "/predictions" | "/preds")
}
