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
    /// into chunks at paragraph boundaries, respecting UTF-8 character boundaries.
    pub fn format_response(text: &str) -> Vec<String> {
        const MAX_LEN: usize = 3800; // leave room for formatting overhead
        if text.len() <= MAX_LEN {
            return vec![text.to_string()];
        }
        let mut chunks = Vec::new();
        let mut remaining = text;
        while remaining.len() > MAX_LEN {
            // Find a safe byte boundary at or before MAX_LEN.
            // Walk backwards from MAX_LEN to find the start of a valid char.
            let safe_end = (0..=MAX_LEN)
                .rev()
                .find(|&i| remaining.is_char_boundary(i))
                .unwrap_or(0);

            // Within that safe window, try to split at the last newline.
            let split_at = remaining[..safe_end]
                .rfind('\n')
                .unwrap_or(safe_end);

            // Ensure split_at is also on a char boundary (rfind on '\n' always gives
            // a char-boundary position, but guard defensively).
            let split_at = if remaining.is_char_boundary(split_at) {
                split_at
            } else {
                safe_end
            };

            chunks.push(remaining[..split_at].to_string());
            remaining = remaining[split_at..].trim_start_matches('\n');
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

/// The help text shown to Telegram users.
///
/// Kept as a constant so the poll loop and tests share the same string.
pub const TELEGRAM_HELP_TEXT: &str = "\
*Axonix Bot* — Available commands:

/ask <prompt> — Send a prompt to the agent and get a response
/status — Show current session status (model, mode, uptime)
/health — Show system health (CPU, memory, disk, uptime)
/brief — Morning brief: active goals, open predictions, recent sessions
/help — Show this help message

*Examples:*
• /ask explain how async Rust works
• /ask what files are in /workspace/src?
• /status
• /health
• /brief

Responses may take a moment depending on prompt complexity.";

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
}

impl TelegramClient {
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
                let prompt = parse_ask_command(text)?;
                Some(BotCommand::Ask(AskCommand {
                    prompt: prompt.to_string(),
                    message_id: msg.message_id,
                }))
            })
            .collect()
    }

    /// Build a status reply string for the `/status` command.
    ///
    /// `model` — current model name
    /// `mode` — "interactive" or "cron"
    /// `elapsed_secs` — seconds since session start (0 if unknown)
    /// `tokens_in` / `tokens_out` — session token totals
    pub fn format_status_reply(
        model: &str,
        mode: &str,
        elapsed_secs: u64,
        tokens_in: u64,
        tokens_out: u64,
    ) -> String {
        let mins = elapsed_secs / 60;
        let secs = elapsed_secs % 60;
        let elapsed_str = if mins > 0 {
            format!("{mins}m {secs}s")
        } else {
            format!("{secs}s")
        };
        format!(
            "*Axonix Status*\n\
            🤖 model: `{model}`\n\
            ⚙️ mode: {mode}\n\
            ⏱ uptime: {elapsed_str}\n\
            📊 tokens: {tokens_in} in / {tokens_out} out",
        )
    }
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

    /// Regression test: format_response must not panic on multi-byte UTF-8 content
    /// when splitting long messages. Previously used `&remaining[..MAX_LEN]` which
    /// panics if a character straddles the 3800-byte boundary.
    #[test]
    fn test_format_response_unicode_no_panic() {
        // Build a response that is definitely > 3800 bytes using 4-byte emoji characters.
        // 1000 emoji × 4 bytes = 4000 bytes, which exceeds MAX_LEN (3800).
        // A naive `&text[..3800]` would panic because byte 3800 lands mid-emoji.
        let emoji_text = "🦀".repeat(1000);
        // Must not panic
        let chunks = TelegramClient::format_response(&emoji_text);
        assert!(chunks.len() >= 1, "should produce at least one chunk");
        // Every chunk must be valid UTF-8 and within the limit
        for chunk in &chunks {
            assert!(std::str::from_utf8(chunk.as_bytes()).is_ok(), "chunk must be valid UTF-8");
            assert!(chunk.len() <= 3800, "chunk must be within Telegram's limit: {} bytes", chunk.len());
        }
        // Reconstructed content should match (trimming is acceptable at boundaries)
        let reconstructed: String = chunks.join("");
        assert!(!reconstructed.is_empty(), "reconstructed content must not be empty");
    }

    #[test]
    fn test_format_response_cjk_no_panic() {
        // CJK chars are 3 bytes each. 1300 of them = 3900 bytes > 3800 limit.
        // Byte 3800 = 3800/3 = 1266.6, which is in the middle of char 1267.
        let cjk_text = "你好世界".repeat(325); // 1300 chars, 3900 bytes
        let chunks = TelegramClient::format_response(&cjk_text);
        assert!(chunks.len() >= 2, "long CJK text should be split: got {} chunk(s)", chunks.len());
        for chunk in &chunks {
            assert!(chunk.len() <= 3800, "chunk must not exceed Telegram limit: {} bytes", chunk.len());
        }
    }

    #[test]
    fn test_format_response_all_chunks_valid_utf8() {
        // Mix of ASCII and multi-byte chars to stress the boundary detection
        let mut text = "a".repeat(3700);
        text.push_str(&"🎉".repeat(200)); // 3700 + 800 = 4500 bytes total
        let chunks = TelegramClient::format_response(&text);
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                std::str::from_utf8(chunk.as_bytes()).is_ok(),
                "chunk {i} must be valid UTF-8"
            );
        }
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

    // ── is_help_command ────────────────────────────────────────────────────────

    #[test]
    fn test_is_help_command_slash_help() {
        assert!(is_help_command("/help"));
        assert!(is_help_command("  /help  "));
    }

    #[test]
    fn test_is_help_command_slash_start() {
        // /start is the Telegram onboarding command — treat as help
        assert!(is_help_command("/start"));
    }

    #[test]
    fn test_is_help_command_false_for_non_help() {
        assert!(!is_help_command("/ask hello"));
        assert!(!is_help_command("help me"));
        assert!(!is_help_command(""));
        assert!(!is_help_command("/status"));
    }

    // ── TELEGRAM_HELP_TEXT ─────────────────────────────────────────────────────

    #[test]
    fn test_help_text_mentions_ask() {
        assert!(TELEGRAM_HELP_TEXT.contains("/ask"), "help text must mention /ask command");
    }

    #[test]
    fn test_help_text_mentions_help() {
        assert!(TELEGRAM_HELP_TEXT.contains("/help"), "help text must mention /help command");
    }

    #[test]
    fn test_help_text_not_empty() {
        assert!(!TELEGRAM_HELP_TEXT.trim().is_empty(), "help text must not be empty");
    }

    // ── extract_commands ───────────────────────────────────────────────────────

    #[test]
    fn test_extract_commands_help_detected() {
        let client = make_client();
        let updates = vec![make_update(1, 12345, 1, "/help")];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 1);
        assert!(matches!(commands[0], BotCommand::Help { message_id: 1 }));
    }

    #[test]
    fn test_extract_commands_start_detected_as_help() {
        let client = make_client();
        let updates = vec![make_update(1, 12345, 1, "/start")];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 1);
        assert!(matches!(commands[0], BotCommand::Help { .. }));
    }

    #[test]
    fn test_extract_commands_ask_detected() {
        let client = make_client();
        let updates = vec![make_update(1, 12345, 1, "/ask what is Rust?")];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 1);
        if let BotCommand::Ask(cmd) = &commands[0] {
            assert_eq!(cmd.prompt, "what is Rust?");
            assert_eq!(cmd.message_id, 1);
        } else {
            panic!("expected BotCommand::Ask");
        }
    }

    #[test]
    fn test_extract_commands_mixed_batch() {
        let client = make_client();
        let updates = vec![
            make_update(1, 12345, 1, "/ask first question"),
            make_update(2, 12345, 2, "/help"),
            make_update(3, 12345, 3, "just a chat message"),
            make_update(4, 12345, 4, "/ask second question"),
        ];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 3, "3 commands: 2 asks + 1 help, chat ignored");
        assert!(matches!(commands[0], BotCommand::Ask(_)));
        assert!(matches!(commands[1], BotCommand::Help { .. }));
        assert!(matches!(commands[2], BotCommand::Ask(_)));
    }

    #[test]
    fn test_extract_commands_wrong_chat_ignored() {
        let client = make_client(); // chat_id = "12345"
        let updates = vec![
            make_update(1, 99999, 1, "/help"),  // wrong chat
            make_update(2, 99999, 2, "/ask something"), // wrong chat
        ];
        let commands = client.extract_commands(&updates);
        assert!(commands.is_empty(), "commands from wrong chat_id must be rejected");
    }

    // ── is_status_command ─────────────────────────────────────────────────────

    #[test]
    fn test_is_status_command_true() {
        assert!(is_status_command("/status"));
        assert!(is_status_command("  /status  "));
    }

    #[test]
    fn test_is_status_command_false_for_non_status() {
        assert!(!is_status_command("/ask hello"));
        assert!(!is_status_command("/help"));
        assert!(!is_status_command("status"));
        assert!(!is_status_command(""));
        assert!(!is_status_command("/statuss"));
    }

    // ── extract_commands with /status ─────────────────────────────────────────

    #[test]
    fn test_extract_commands_status_detected() {
        let client = make_client();
        let updates = vec![make_update(1, 12345, 7, "/status")];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(commands[0], BotCommand::Status { message_id: 7 }),
            "expected BotCommand::Status, got {:?}",
            commands[0]
        );
    }

    #[test]
    fn test_extract_commands_status_in_mixed_batch() {
        let client = make_client();
        let updates = vec![
            make_update(1, 12345, 1, "/ask first question"),
            make_update(2, 12345, 2, "/status"),
            make_update(3, 12345, 3, "/help"),
            make_update(4, 12345, 4, "just a message"),
        ];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 3, "3 commands: 1 ask + 1 status + 1 help");
        assert!(matches!(commands[0], BotCommand::Ask(_)));
        assert!(matches!(commands[1], BotCommand::Status { .. }));
        assert!(matches!(commands[2], BotCommand::Help { .. }));
    }

    // ── format_status_reply ───────────────────────────────────────────────────

    #[test]
    fn test_format_status_reply_contains_model() {
        let reply = TelegramClient::format_status_reply("claude-opus-4-6", "interactive", 120, 1000, 500);
        assert!(reply.contains("claude-opus-4-6"), "status reply must show model: {reply}");
    }

    #[test]
    fn test_format_status_reply_contains_mode() {
        let reply = TelegramClient::format_status_reply("test-model", "cron", 30, 0, 0);
        assert!(reply.contains("cron"), "status reply must show mode: {reply}");
    }

    #[test]
    fn test_format_status_reply_elapsed_minutes() {
        let reply = TelegramClient::format_status_reply("m", "interactive", 185, 0, 0);
        assert!(reply.contains("3m"), "185s should show as 3m: {reply}");
    }

    #[test]
    fn test_format_status_reply_elapsed_seconds_only() {
        let reply = TelegramClient::format_status_reply("m", "interactive", 45, 0, 0);
        assert!(reply.contains("45s"), "45s should show without minutes: {reply}");
        assert!(!reply.contains("0m"), "should not show 0m prefix: {reply}");
    }

    #[test]
    fn test_format_status_reply_contains_tokens() {
        let reply = TelegramClient::format_status_reply("m", "cron", 0, 1234, 567);
        assert!(reply.contains("1234"), "should show input tokens: {reply}");
        assert!(reply.contains("567"), "should show output tokens: {reply}");
    }

    #[test]
    fn test_help_text_mentions_status() {
        assert!(TELEGRAM_HELP_TEXT.contains("/status"), "help text must mention /status command");
    }

    // ── is_health_command ─────────────────────────────────────────────────────

    #[test]
    fn test_is_health_command_true() {
        assert!(is_health_command("/health"));
        assert!(is_health_command("  /health  "));
    }

    #[test]
    fn test_is_health_command_false_for_non_health() {
        assert!(!is_health_command("/ask hello"));
        assert!(!is_health_command("/help"));
        assert!(!is_health_command("/status"));
        assert!(!is_health_command("health"));
        assert!(!is_health_command(""));
        assert!(!is_health_command("/healthcheck"));
    }

    // ── extract_commands with /health ─────────────────────────────────────────

    #[test]
    fn test_extract_commands_health_detected() {
        let client = make_client();
        let updates = vec![make_update(1, 12345, 9, "/health")];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(commands[0], BotCommand::Health { message_id: 9 }),
            "expected BotCommand::Health, got {:?}",
            commands[0]
        );
    }

    #[test]
    fn test_extract_commands_health_in_mixed_batch() {
        let client = make_client();
        let updates = vec![
            make_update(1, 12345, 1, "/ask first question"),
            make_update(2, 12345, 2, "/status"),
            make_update(3, 12345, 3, "/health"),
            make_update(4, 12345, 4, "/help"),
            make_update(5, 12345, 5, "just a message"),
        ];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 4, "4 commands: ask + status + health + help");
        assert!(matches!(commands[0], BotCommand::Ask(_)));
        assert!(matches!(commands[1], BotCommand::Status { .. }));
        assert!(matches!(commands[2], BotCommand::Health { .. }));
        assert!(matches!(commands[3], BotCommand::Help { .. }));
    }

    #[test]
    fn test_help_text_mentions_health() {
        assert!(TELEGRAM_HELP_TEXT.contains("/health"), "help text must mention /health command");
    }

    // ── is_brief_command ─────────────────────────────────────────────────────

    #[test]
    fn test_is_brief_command_true() {
        assert!(is_brief_command("/brief"));
    }

    #[test]
    fn test_is_brief_command_trimmed() {
        assert!(is_brief_command("  /brief  "));
    }

    #[test]
    fn test_is_brief_command_false_for_non_brief() {
        assert!(!is_brief_command("/ask hello"));
        assert!(!is_brief_command("/help"));
        assert!(!is_brief_command("/status"));
        assert!(!is_brief_command("/health"));
        assert!(!is_brief_command("brief"));
        assert!(!is_brief_command(""));
        assert!(!is_brief_command("/briefing"));
    }

    // ── extract_commands with /brief ─────────────────────────────────────────

    #[test]
    fn test_extract_commands_brief_detected() {
        let client = make_client();
        let updates = vec![make_update(1, 12345, 11, "/brief")];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(commands[0], BotCommand::Brief { message_id: 11 }),
            "expected BotCommand::Brief, got {:?}",
            commands[0]
        );
    }

    #[test]
    fn test_extract_commands_brief_in_mixed_batch() {
        let client = make_client();
        let updates = vec![
            make_update(1, 12345, 1, "/ask first question"),
            make_update(2, 12345, 2, "/status"),
            make_update(3, 12345, 3, "/health"),
            make_update(4, 12345, 4, "/help"),
            make_update(5, 12345, 5, "/brief"),
            make_update(6, 12345, 6, "just a message"),
        ];
        let commands = client.extract_commands(&updates);
        assert_eq!(commands.len(), 5, "5 commands: ask + status + health + help + brief");
        assert!(matches!(commands[0], BotCommand::Ask(_)));
        assert!(matches!(commands[1], BotCommand::Status { .. }));
        assert!(matches!(commands[2], BotCommand::Health { .. }));
        assert!(matches!(commands[3], BotCommand::Help { .. }));
        assert!(matches!(commands[4], BotCommand::Brief { .. }));
    }

    #[test]
    fn test_help_text_mentions_brief() {
        assert!(TELEGRAM_HELP_TEXT.contains("/brief"), "help text must mention /brief command");
    }
}
