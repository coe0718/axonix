//! TelegramClient struct and all its implementations.
//!
//! Formatting helpers live in `client_format.rs`.

use super::types::{TelegramUpdate, AskCommand, BotCommand};
use super::types::TelegramApiResponse;
use super::commands::{
    parse_ask_command, is_help_command, is_status_command, is_health_command,
    is_brief_command, is_history_command, is_goals_command, parse_predict_command,
    parse_resolve_command, parse_run_command, parse_goal_command, parse_memory_command,
    is_list_predictions_command,
};

// Formatting impl block lives in client_format.rs
#[path = "client_format.rs"]
mod client_format;

const TELEGRAM_API: &str = "https://api.telegram.org";

/// Telegram Bot API client.
///
/// Requires `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID` env vars.
#[derive(Clone)]
pub struct TelegramClient {
    pub(super) token: String,
    pub(super) chat_id: String,
    pub(super) client: reqwest::Client,
}

impl TelegramClient {
    /// Create a client from environment variables.
    ///
    /// Reads `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID`.
    /// Returns `None` if either is missing or empty.
    pub fn from_env() -> Option<Self> {
        let token = std::env::var("TELEGRAM_BOT_TOKEN")
            .or_else(|_| std::env::var("TELEGRAM_TOKEN"))
            .ok()
            .filter(|s| !s.is_empty())?;
        let chat_id = std::env::var("TELEGRAM_CHAT_ID")
            .ok()
            .filter(|s| !s.is_empty())?;
        Some(Self::new(token, chat_id))
    }

    /// Create a client with explicit credentials.
    pub fn new(token: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            chat_id: chat_id.into(),
            client: crate::http_client::get(),
        }
    }

    /// Send a text message to the configured chat.
    ///
    /// Errors are soft — a failed notification should never crash the agent.
    pub async fn send_message(&self, text: &str) -> Result<(), String> {
        let url = format!("{}/bot{}/sendMessage", TELEGRAM_API, self.token);
        let res = self.client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
            }))
            .send()
            .await
            .map_err(|e| format!("telegram send error: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("telegram API error {status}: {body}"));
        }
        Ok(())
    }

    /// Send a reply to a specific message.
    pub async fn reply_to(&self, text: &str, reply_to_message_id: i64) -> Result<(), String> {
        let url = format!("{}/bot{}/sendMessage", TELEGRAM_API, self.token);
        let res = self.client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "reply_to_message_id": reply_to_message_id
            }))
            .send()
            .await
            .map_err(|e| format!("telegram reply error: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("telegram API error {status}: {body}"));
        }
        Ok(())
    }

    /// Poll for new updates since `offset`.
    ///
    /// Returns a list of updates. Pass `last_update_id + 1` as `offset`
    /// to acknowledge received updates and avoid reprocessing.
    pub async fn get_updates(&self, offset: i64) -> Result<Vec<TelegramUpdate>, String> {
        let url = format!("{}/bot{}/getUpdates", TELEGRAM_API, self.token);
        let res = self.client
            .get(&url)
            .query(&[
                ("offset", offset.to_string()),
                ("timeout", "5".to_string()), // long-poll for 5 seconds
                ("allowed_updates", "[\"message\"]".to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("telegram getUpdates error: {e}"))?;

        let body: TelegramApiResponse<Vec<TelegramUpdate>> = res
            .json()
            .await
            .map_err(|e| format!("telegram parse error: {e}"))?;

        if !body.ok {
            return Err(body.description.unwrap_or_else(|| "unknown error".to_string()));
        }
        Ok(body.result.unwrap_or_default())
    }

    /// Scan a batch of updates for `/ask <prompt>` commands.
    ///
    /// Only processes messages from the configured chat ID (security: ignore
    /// messages from other chats to prevent prompt injection from strangers).
    pub fn extract_ask_commands(&self, updates: &[TelegramUpdate]) -> Vec<AskCommand> {
        updates
            .iter()
            .filter_map(|u| u.message.as_ref())
            .filter(|msg| msg.chat.id.to_string() == self.chat_id)
            .filter_map(|msg| {
                let text = msg.text.as_deref()?;
                let prompt = parse_ask_command(text)?;
                Some(AskCommand {
                    prompt: prompt.to_string(),
                    message_id: msg.message_id,
                })
            })
            .collect()
    }

    /// Scan a batch of updates for bot commands (ask, help, status, health, brief).
    ///
    /// Only processes messages from the configured chat ID (security: ignore
    /// messages from other chats to prevent prompt injection from strangers).
    pub fn extract_commands(&self, updates: &[TelegramUpdate]) -> Vec<BotCommand> {
        updates
            .iter()
            .filter_map(|u| u.message.as_ref())
            .filter(|msg| msg.chat.id.to_string() == self.chat_id)
            .filter_map(|msg| {
                let text = msg.text.as_deref()?;
                if is_help_command(text) {
                    return Some(BotCommand::Help { message_id: msg.message_id });
                }
                if is_status_command(text) {
                    return Some(BotCommand::Status { message_id: msg.message_id });
                }
                if is_health_command(text) {
                    return Some(BotCommand::Health { message_id: msg.message_id });
                }
                if is_brief_command(text) {
                    return Some(BotCommand::Brief { message_id: msg.message_id });
                }
                if is_history_command(text) {
                    return Some(BotCommand::History { message_id: msg.message_id });
                }
                if is_goals_command(text) {
                    return Some(BotCommand::Goals { message_id: msg.message_id });
                }
                if is_list_predictions_command(text) {
                    return Some(BotCommand::ListPredictions { message_id: msg.message_id });
                }
                if let Some(pred_text) = parse_predict_command(text) {
                    return Some(BotCommand::Predict { text: pred_text.to_string(), message_id: msg.message_id });
                }
                if let Some((id, verdict)) = parse_resolve_command(text) {
                    return Some(BotCommand::Resolve { id, verdict, message_id: msg.message_id });
                }
                if let Some(task) = parse_run_command(text) {
                    return Some(BotCommand::Run { task: task.to_string(), message_id: msg.message_id });
                }
                if let Some(desc) = parse_goal_command(text) {
                    return Some(BotCommand::Goal { description: desc.to_string(), message_id: msg.message_id });
                }
                if let Some(action) = parse_memory_command(text) {
                    return Some(BotCommand::Memory { action, message_id: msg.message_id });
                }
                let prompt = parse_ask_command(text)?;
                Some(BotCommand::Ask(AskCommand {
                    prompt: prompt.to_string(),
                    message_id: msg.message_id,
                }))
            })
            .collect()
    }
}
