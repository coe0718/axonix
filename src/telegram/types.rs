//! Telegram API type definitions.

use serde::Deserialize;

/// A single Telegram update (message received by the bot).
#[derive(Debug, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

/// A Telegram message.
#[derive(Debug, Deserialize, Clone)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub text: Option<String>,
    pub from: Option<TelegramUser>,
    pub chat: TelegramChat,
    pub date: i64,
}

/// Telegram user info.
#[derive(Debug, Deserialize, Clone)]
pub struct TelegramUser {
    pub id: i64,
    pub first_name: String,
    pub username: Option<String>,
}

/// Telegram chat info.
#[derive(Debug, Deserialize, Clone)]
pub struct TelegramChat {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct TelegramApiResponse<T> {
    pub ok: bool,
    pub result: Option<T>,
    pub description: Option<String>,
}

/// An `/ask <prompt>` command parsed from a Telegram message.
#[derive(Debug, PartialEq, Clone)]
pub struct AskCommand {
    /// The prompt text the user wants to send to the agent.
    pub prompt: String,
    /// Telegram message ID (for reply threading).
    pub message_id: i64,
}

/// Action for the `/memory` command.
#[derive(Debug, PartialEq, Clone)]
pub enum MemoryAction {
    /// `/memory add <text>` or `/memory add:<category> <text>` — store an observation.
    Add { text: String, category: String },
    /// `/memory search <query>` — search past observations.
    Search { query: String },
    /// `/memory list` — list recent observations.
    List,
}

/// A bot command parsed from an inbound Telegram update.
///
/// Unifies all supported command types so the main poll loop
/// can dispatch them with a single match.
#[derive(Debug, PartialEq, Clone)]
pub enum BotCommand {
    /// `/ask <prompt>` — forward prompt to the agent.
    Ask(AskCommand),
    /// `/help` or `/start` — send help text back to the user.
    Help { message_id: i64 },
    /// `/status` — report current session status (model, mode, uptime).
    Status { message_id: i64 },
    /// `/health` — report system health (CPU, memory, disk, uptime).
    Health { message_id: i64 },
    /// `/brief` — send the morning brief (active goals, predictions, recent sessions).
    Brief { message_id: i64 },
    /// `/run <task>` — spin up a mini sub-agent session with the given prompt.
    Run { task: String, message_id: i64 },
    /// `/goal <description>` — append a goal to GOALS.md backlog.
    Goal { description: String, message_id: i64 },
    /// `/memory add <text>` — store a structured observation.
    /// `/memory search <query>` — search structured observations.
    /// `/memory list` — list recent observations.
    Memory { action: MemoryAction, message_id: i64 },
    /// `/history` — show last 5 conversation turns from ConversationMemory.
    History { message_id: i64 },
    /// `/goals` — show active goals and first backlog item.
    Goals { message_id: i64 },
    /// `/predict <text>` — append a new prediction to .axonix/predictions.json.
    Predict { text: String, message_id: i64 },
    /// `/resolve <id> correct|wrong` — resolve a prediction by ID.
    Resolve { id: u32, verdict: bool, message_id: i64 },
    /// `/predictions` or `/preds` — list all open (unresolved) predictions.
    ListPredictions { message_id: i64 },
}
