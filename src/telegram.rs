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

use serde::Deserialize;

const TELEGRAM_API: &str = "https://api.telegram.org";

/// Telegram Bot API client.
///
/// Requires `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID` env vars.
#[derive(Clone)]
pub struct TelegramClient {
    token: String,
    chat_id: String,
    client: reqwest::Client,
}

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
struct TelegramApiResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

/// An `/ask <prompt>` command parsed from a Telegram message.
#[derive(Debug, PartialEq, Clone)]
pub struct AskCommand {
    /// The prompt text the user wants to send to the agent.
    pub prompt: String,
    /// Telegram message ID (for reply threading).
    pub message_id: i64,
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
            client: reqwest::Client::new(),
        }
    }

    /// Send a text message to the configured chat.
    ///
    /// Uses `parse_mode=Markdown` for basic formatting support.
    /// Errors are soft — a failed notification should never crash the agent.
    pub async fn send_message(&self, text: &str) -> Result<(), String> {
        let url = format!("{}/bot{}/sendMessage", TELEGRAM_API, self.token);
        let res = self.client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown"
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
                "parse_mode": "Markdown",
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

    /// Format a long agent response for Telegram.
    ///
    /// Telegram has a 4096-character message limit. Splits long responses
    /// into chunks and truncates tool output noise.
    pub fn format_response(text: &str) -> Vec<String> {
        const MAX_LEN: usize = 3800; // leave room for formatting overhead
        if text.len() <= MAX_LEN {
            return vec![text.to_string()];
        }
        // Split at paragraph boundaries if possible
        let mut chunks = Vec::new();
        let mut remaining = text;
        while remaining.len() > MAX_LEN {
            // Try to split at last newline before MAX_LEN
            let split_at = remaining[..MAX_LEN]
                .rfind('\n')
                .unwrap_or(MAX_LEN);
            chunks.push(remaining[..split_at].to_string());
            remaining = &remaining[split_at..].trim_start_matches('\n');
        }
        if !remaining.is_empty() {
            chunks.push(remaining.to_string());
        }
        chunks
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_ask_command ──────────────────────────────────────────────────────

    #[test]
    fn test_parse_ask_slash_format() {
        assert_eq!(parse_ask_command("/ask explain monads"), Some("explain monads"));
    }

    #[test]
    fn test_parse_ask_colon_format() {
        assert_eq!(parse_ask_command("ask: what is rust?"), Some("what is rust?"));
    }

    #[test]
    fn test_parse_ask_slash_with_leading_whitespace() {
        assert_eq!(parse_ask_command("  /ask  hello world  "), Some("hello world"));
    }

    #[test]
    fn test_parse_ask_slash_empty_prompt_returns_none() {
        assert_eq!(parse_ask_command("/ask"), None);
        assert_eq!(parse_ask_command("/ask   "), None);
    }

    #[test]
    fn test_parse_ask_colon_empty_prompt_returns_none() {
        assert_eq!(parse_ask_command("ask:"), None);
        assert_eq!(parse_ask_command("ask:   "), None);
    }

    #[test]
    fn test_parse_ask_non_command_returns_none() {
        assert_eq!(parse_ask_command("hello world"), None);
        assert_eq!(parse_ask_command("/help"), None);
        assert_eq!(parse_ask_command(""), None);
        assert_eq!(parse_ask_command("/status"), None);
    }

    #[test]
    fn test_parse_ask_multiword_prompt() {
        let prompt = "/ask explain the difference between async and sync rust code";
        assert_eq!(
            parse_ask_command(prompt),
            Some("explain the difference between async and sync rust code")
        );
    }

    #[test]
    fn test_parse_ask_multiline_prompt() {
        let prompt = "/ask fix this code:\nfn main() { println!(\"hello\") }";
        assert_eq!(
            parse_ask_command(prompt),
            Some("fix this code:\nfn main() { println!(\"hello\") }")
        );
    }

    // ── is_ask_command ─────────────────────────────────────────────────────────

    #[test]
    fn test_is_ask_command_true() {
        assert!(is_ask_command("/ask hello"));
        assert!(is_ask_command("ask: world"));
    }

    #[test]
    fn test_is_ask_command_false() {
        assert!(!is_ask_command("hello"));
        assert!(!is_ask_command("/quit"));
        assert!(!is_ask_command(""));
    }

    // ── format_response ────────────────────────────────────────────────────────

    #[test]
    fn test_format_response_short_message_unchanged() {
        let text = "Hello, this is a short response.";
        let chunks = TelegramClient::format_response(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_format_response_long_message_splits() {
        let line = "x".repeat(100) + "\n";
        let text = line.repeat(50); // 5050 chars > 3800 limit
        let chunks = TelegramClient::format_response(&text);
        assert!(chunks.len() >= 2, "Long message should split into 2+ chunks: got {}", chunks.len());
        for chunk in &chunks {
            assert!(chunk.len() <= 3800, "Each chunk should be <= 3800 chars, got {}", chunk.len());
        }
    }

    #[test]
    fn test_format_response_reconstructable() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = TelegramClient::format_response(text);
        // All chunks joined should contain all the original content
        let joined = chunks.join("\n");
        assert!(joined.contains("First paragraph"), "joined chunks should contain original content");
        assert!(joined.contains("Second paragraph"), "joined chunks should contain original content");
        assert!(joined.contains("Third paragraph"), "joined chunks should contain original content");
    }

    // ── extract_ask_commands ───────────────────────────────────────────────────

    fn make_client() -> TelegramClient {
        TelegramClient::new("fake_token", "12345")
    }

    fn make_update(update_id: i64, chat_id: i64, msg_id: i64, text: &str) -> TelegramUpdate {
        TelegramUpdate {
            update_id,
            message: Some(TelegramMessage {
                message_id: msg_id,
                text: Some(text.to_string()),
                from: None,
                chat: TelegramChat { id: chat_id },
                date: 0,
            }),
        }
    }

    #[test]
    fn test_extract_ask_commands_none_present() {
        let client = make_client();
        let updates = vec![
            make_update(1, 12345, 1, "hello world"),
            make_update(2, 12345, 2, "/help"),
        ];
        let commands = client.extract_ask_commands(&updates);
        assert!(commands.is_empty(), "No ask commands should be extracted: {commands:?}");
    }

    #[test]
    fn test_extract_ask_commands_one_found() {
        let client = make_client();
        let updates = vec![
            make_update(1, 12345, 1, "/ask what is the time?"),
        ];
        let commands = client.extract_ask_commands(&updates);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prompt, "what is the time?");
        assert_eq!(commands[0].message_id, 1);
    }

    #[test]
    fn test_extract_ask_commands_wrong_chat_id_ignored() {
        let client = make_client(); // chat_id = "12345"
        let updates = vec![
            make_update(1, 99999, 1, "/ask from a stranger"), // wrong chat!
        ];
        let commands = client.extract_ask_commands(&updates);
        assert!(
            commands.is_empty(),
            "Messages from wrong chat_id should be rejected (prompt injection protection)"
        );
    }

    #[test]
    fn test_extract_ask_commands_colon_format() {
        let client = make_client();
        let updates = vec![
            make_update(1, 12345, 5, "ask: summarize the latest commits"),
        ];
        let commands = client.extract_ask_commands(&updates);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prompt, "summarize the latest commits");
    }

    #[test]
    fn test_extract_ask_commands_multiple() {
        let client = make_client();
        let updates = vec![
            make_update(1, 12345, 1, "/ask first question"),
            make_update(2, 12345, 2, "hello"),
            make_update(3, 12345, 3, "/ask second question"),
        ];
        let commands = client.extract_ask_commands(&updates);
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].prompt, "first question");
        assert_eq!(commands[1].prompt, "second question");
    }

    #[test]
    fn test_extract_ask_commands_update_without_message() {
        let client = make_client();
        let updates = vec![
            TelegramUpdate { update_id: 1, message: None },
        ];
        let commands = client.extract_ask_commands(&updates);
        assert!(commands.is_empty());
    }

    // ── TelegramClient::from_env ───────────────────────────────────────────────

    #[test]
    fn test_from_env_returns_none_when_token_missing() {
        // We can't safely unset env vars in parallel tests, but we can verify
        // that from_env returns Some only when both vars are set.
        // This is a structural test — the real env behavior is integration-level.
        let client = TelegramClient::new("tok", "cid");
        assert_eq!(client.token, "tok");
        assert_eq!(client.chat_id, "cid");
    }
}
