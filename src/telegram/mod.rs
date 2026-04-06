//! Telegram Bot API integration.
//!
//! Provides bidirectional communication via Telegram:
//! - Send messages to a configured chat (session notifications, agent responses)
//! - Poll for inbound `/ask <prompt>` commands and queue them for the agent
//!
//! # Configuration
//!
//! Set these environment variables:
//!   - `TELEGRAM_BOT_TOKEN` — bot token from @BotFather
//!   - `TELEGRAM_CHAT_ID` — target chat ID (get from @userinfobot or via getUpdates)
//!
//! # Example
//!
//! ```no_run
//! use axonix::telegram::TelegramClient;
//!
//! # async fn example() {
//! let tg = TelegramClient::from_env().unwrap();
//! tg.send_message("Hello from Axonix!").await.ok();
//! # }
//! ```

pub mod types;
pub mod commands;
pub mod client;

pub use types::{
    TelegramUpdate, TelegramMessage, TelegramUser, TelegramChat,
    AskCommand, MemoryAction, BotCommand,
};
pub use commands::{
    TELEGRAM_HELP_TEXT,
    parse_ask_command, is_ask_command,
    is_help_command, is_status_command, is_health_command, is_brief_command,
    is_history_command, is_goals_command,
    parse_run_command, is_run_command,
    parse_goal_command, is_goal_command,
    parse_predict_command,
    parse_resolve_command,
    parse_memory_command,
};
pub use client::TelegramClient;

#[cfg(test)]
mod tests;
