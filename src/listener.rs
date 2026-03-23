//! Always-on Telegram listener daemon for the personal assistant.
//!
//! Runs 24/7 as a separate process alongside evolve.sh sessions.
//! Handles `/ask` commands immediately with a short-context agent.
//! Writes conversation turns to [`ConversationMemory`] for session context injection.
//!
//! # Two-process model
//!
//! - **evolve.sh sessions** — long-running, high-context self-improvement loops.
//! - **axonix-listener** — always-on, low-latency daemon for operator queries.
//!
//! These run as separate Docker containers sharing the workspace volume.
//! They communicate via `.axonix/conversation_memory.json`.
//!
//! # Note on `run_listener`
//!
//! The async poll loop (`run_listener`) is not included here yet — wiring it into
//! `main.rs` with a `--listen` flag is a separate task.  This module provides the
//! configuration, statistics, and system prompt builder needed to design and test the
//! listener without requiring the full async runtime setup.
//!
//! # Example
//!
//! ```
//! use axonix::listener::{ListenerConfig, ListenerStats, build_listener_system_prompt};
//! use axonix::conversation_memory::ConversationMemory;
//!
//! let config = ListenerConfig::default();
//! assert_eq!(config.poll_interval_secs, 2);
//!
//! let mut stats = ListenerStats::new();
//! stats.messages_handled += 1;
//! let summary = stats.format();
//! assert!(summary.contains("messages"));
//!
//! let dir = tempfile::tempdir().unwrap();
//! let mem = ConversationMemory::new(dir.path().join("conv.json"));
//! let prompt = build_listener_system_prompt(&mem);
//! assert!(!prompt.is_empty());
//! ```

use crate::conversation_memory::ConversationMemory;
use crate::telegram::{TelegramClient, BotCommand, TELEGRAM_HELP_TEXT};
use crate::health::HealthSnapshot;
use yoagent::agent::Agent;
use yoagent::provider::AnthropicProvider;
use yoagent::tools::default_tools;
use yoagent::context::ContextConfig;
use yoagent::retry::RetryConfig;
use yoagent::{AgentEvent, StreamDelta};

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration for the always-on Telegram listener.
#[derive(Debug, Clone)]
pub struct ListenerConfig {
    /// How often to poll Telegram for new messages (seconds).  Default: 2.
    pub poll_interval_secs: u64,
    /// Maximum characters in a listener response (keeps Telegram replies short).  Default: 3000.
    pub max_response_chars: usize,
    /// Path to the conversation memory file.  `None` = use default path.
    pub memory_path: Option<String>,
    /// Maximum number of conversation turns to keep in memory.  Default: 100.
    pub max_memory_turns: usize,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 2,
            max_response_chars: 3000,
            memory_path: None,
            max_memory_turns: 100,
        }
    }
}

// ── Stats ─────────────────────────────────────────────────────────────────────

/// Runtime statistics for the listener daemon.
#[derive(Debug, Clone)]
pub struct ListenerStats {
    /// Total number of messages successfully handled.
    pub messages_handled: u64,
    /// Total number of errors encountered.
    pub errors: u64,
    /// Seconds since the listener started.
    pub uptime_secs: u64,
}

impl ListenerStats {
    /// Create a new zero-initialised stats block.
    pub fn new() -> Self {
        Self {
            messages_handled: 0,
            errors: 0,
            uptime_secs: 0,
        }
    }

    /// Format the stats as a compact human-readable string.
    ///
    /// Example: `"📊 Listener: 42 messages, 3h 20m uptime, 0 errors"`
    pub fn format(&self) -> String {
        let uptime = format_duration(self.uptime_secs);
        format!(
            "📊 Listener: {} messages, {} uptime, {} errors",
            self.messages_handled, uptime, self.errors
        )
    }
}

impl Default for ListenerStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a duration in seconds as a human-readable string.
///
/// - `< 60s` → `"0m"`
/// - `< 1h`  → `"42m"`
/// - `>= 1h` → `"3h 20m"`
fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours == 0 {
        format!("{}m", minutes)
    } else {
        format!("{}h {}m", hours, minutes)
    }
}

// ── System prompt ─────────────────────────────────────────────────────────────

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
pub fn build_listener_system_prompt(memory: &ConversationMemory) -> String {
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

    // Inject recent conversation context if available
    let context = memory.format_for_context(10);
    if !context.is_empty() {
        parts.push(String::new());
        parts.push(context);
    }

    parts.join("\n")
}

// ── Listener agent builder ────────────────────────────────────────────────────

fn make_listener_agent(api_key: &str, model: &str, system_prompt: &str) -> Agent {
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

// ── run_listener ──────────────────────────────────────────────────────────────

/// Run the always-on Telegram listener loop.
///
/// Polls Telegram every `config.poll_interval_secs` seconds.
/// Handles `/ask <text>` commands with a short-context agent response.
/// Records every conversation turn to ConversationMemory.
/// Runs until Ctrl+C (i.e. until the process is killed).
#[allow(clippy::too_many_lines)]
pub async fn run_listener(
    config: &ListenerConfig,
    tg: &TelegramClient,
    api_key: &str,
    model: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load conversation memory from config path or default
    let memory_path = config
        .memory_path
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(crate::conversation_memory::default_conversation_memory_path);
    let mut mem = ConversationMemory::load(&memory_path);

    // Build system prompt and agent (refreshed if context grows too large)
    let system_prompt = build_listener_system_prompt(&mem);
    let mut agent = make_listener_agent(api_key, model, &system_prompt);
    let mut agent_turn_count: usize = 0;

    let mut stats = ListenerStats::new();
    let mut offset: i64 = 0;
    let start_time = std::time::Instant::now();

    loop {
        // Update uptime
        stats.uptime_secs = start_time.elapsed().as_secs();

        // Poll for updates
        let updates = match tg.get_updates(offset).await {
            Ok(u) => u,
            Err(e) => {
                eprintln!("  ⚠ listener: get_updates error: {e}");
                stats.errors += 1;
                tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
                continue;
            }
        };

        // Advance offset to acknowledge received updates
        if let Some(last) = updates.last() {
            offset = last.update_id + 1;
        }

        // Extract all recognised commands from the batch
        let commands = tg.extract_commands(&updates);

        for cmd in commands {
            match cmd {
                BotCommand::Ask(ask_cmd) => {
                    // Send "processing" acknowledgement
                    let _ = tg.reply_to("⏳ Processing...", ask_cmd.message_id).await;

                    // Rebuild agent if it has processed too many turns (context hygiene)
                    if agent_turn_count > 50 {
                        let fresh_prompt = build_listener_system_prompt(&mem);
                        agent = make_listener_agent(api_key, model, &fresh_prompt);
                        agent_turn_count = 0;
                    }

                    // Run the agent — prompt() returns a streaming event receiver
                    let mut rx = agent.prompt(&ask_cmd.prompt).await;
                    let mut response_text = String::new();
                    let mut had_error = false;

                    while let Some(event) = rx.recv().await {
                        match event {
                            AgentEvent::MessageUpdate {
                                delta: StreamDelta::Text { delta },
                                ..
                            } => {
                                response_text.push_str(&delta);
                            }
                            AgentEvent::InputRejected { reason } => {
                                eprintln!("  ⚠ listener: input rejected: {reason}");
                                had_error = true;
                            }
                            _ => {}
                        }
                    }

                    if had_error || response_text.is_empty() {
                        let msg = if had_error {
                            "⚠️ Request was rejected by the agent.".to_string()
                        } else {
                            "⚠️ No response from agent.".to_string()
                        };
                        let _ = tg.reply_to(&msg, ask_cmd.message_id).await;
                        stats.errors += 1;
                    } else {
                        // Truncate to max_response_chars
                        let reply = if response_text.chars().count() > config.max_response_chars {
                            let truncated: String = response_text
                                .chars()
                                .take(config.max_response_chars)
                                .collect();
                            format!("{truncated}\n_(truncated)_")
                        } else {
                            response_text.clone()
                        };

                        let _ = tg.reply_to(&reply, ask_cmd.message_id).await;

                        // Record turn in memory
                        mem.push("user", &ask_cmd.prompt, "telegram");
                        mem.push("assistant", &response_text, "telegram");
                        if let Err(e) = mem.save() {
                            eprintln!("  ⚠ listener: failed to save memory: {e}");
                        }

                        agent_turn_count += 2; // user + assistant
                        stats.messages_handled += 1;
                    }
                }
                BotCommand::Health { message_id } => {
                    let snap = HealthSnapshot::collect();
                    let reply = snap.format_compact();
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::Brief { message_id } => {
                    let brief = crate::brief::Brief::collect();
                    let reply = brief.format_telegram();
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::Status { message_id } => {
                    let reply = TelegramClient::format_status_reply(
                        model,
                        "listener",
                        stats.uptime_secs,
                        0,
                        0,
                    );
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::Help { message_id } => {
                    let _ = tg.reply_to(TELEGRAM_HELP_TEXT, message_id).await;
                    stats.messages_handled += 1;
                }
            }
        }

        // Log stats summary every 100 messages
        if stats.messages_handled > 0 && stats.messages_handled % 100 == 0 {
            eprintln!("  {}", stats.format());
        }

        // Sleep between polls
        tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation_memory::ConversationMemory;

    // ── ListenerConfig ────────────────────────────────────────────────────────

    #[test]
    fn test_listener_config_default_values() {
        let cfg = ListenerConfig::default();
        assert_eq!(cfg.poll_interval_secs, 2, "poll interval should be 2s");
        assert_eq!(cfg.max_response_chars, 3000, "max response should be 3000 chars");
        assert!(cfg.memory_path.is_none(), "memory_path should default to None");
        assert_eq!(cfg.max_memory_turns, 100, "max_memory_turns should be 100");
    }

    #[test]
    fn test_listener_config_clone() {
        let cfg = ListenerConfig::default();
        let cloned = cfg.clone();
        assert_eq!(cloned.poll_interval_secs, cfg.poll_interval_secs);
        assert_eq!(cloned.max_response_chars, cfg.max_response_chars);
    }

    // ── ListenerStats ─────────────────────────────────────────────────────────

    #[test]
    fn test_listener_stats_new_starts_at_zero() {
        let stats = ListenerStats::new();
        assert_eq!(stats.messages_handled, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.uptime_secs, 0);
    }

    #[test]
    fn test_listener_stats_format_contains_messages_and_uptime() {
        let stats = ListenerStats::new();
        let formatted = stats.format();
        assert!(
            formatted.contains("messages"),
            "format should contain 'messages': {formatted}"
        );
        assert!(
            formatted.contains("uptime"),
            "format should contain 'uptime': {formatted}"
        );
    }

    #[test]
    fn test_listener_stats_format_shows_correct_count() {
        let mut stats = ListenerStats::new();
        stats.messages_handled = 42;
        stats.uptime_secs = 12000; // 3h 20m
        let formatted = stats.format();
        assert!(
            formatted.contains("42"),
            "format should show 42 messages: {formatted}"
        );
        assert!(
            formatted.contains("3h"),
            "format should show 3h uptime: {formatted}"
        );
        assert!(
            formatted.contains("20m"),
            "format should show 20m: {formatted}"
        );
    }

    #[test]
    fn test_listener_stats_format_zero_uptime() {
        let stats = ListenerStats::new();
        let formatted = stats.format();
        assert!(
            formatted.contains("0m"),
            "zero uptime should show '0m': {formatted}"
        );
    }

    #[test]
    fn test_listener_stats_default_is_new() {
        let s1 = ListenerStats::new();
        let s2 = ListenerStats::default();
        assert_eq!(s1.messages_handled, s2.messages_handled);
        assert_eq!(s1.errors, s2.errors);
        assert_eq!(s1.uptime_secs, s2.uptime_secs);
    }

    // ── build_listener_system_prompt ──────────────────────────────────────────

    #[test]
    fn test_build_prompt_with_empty_memory_returns_non_empty_string() {
        let dir = tempfile::tempdir().unwrap();
        let mem = ConversationMemory::new(dir.path().join("conv.json"));
        let prompt = build_listener_system_prompt(&mem);
        assert!(
            !prompt.is_empty(),
            "prompt with empty memory should be non-empty"
        );
    }

    #[test]
    fn test_build_prompt_contains_core_instructions() {
        let dir = tempfile::tempdir().unwrap();
        let mem = ConversationMemory::new(dir.path().join("conv.json"));
        let prompt = build_listener_system_prompt(&mem);
        assert!(prompt.contains("Axonix"), "prompt should mention Axonix");
        assert!(
            prompt.contains("Telegram"),
            "prompt should mention Telegram"
        );
        assert!(
            prompt.to_lowercase().contains("concise") || prompt.to_lowercase().contains("short"),
            "prompt should emphasise conciseness"
        );
    }

    #[test]
    fn test_build_prompt_mentions_evolve_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let mem = ConversationMemory::new(dir.path().join("conv.json"));
        let prompt = build_listener_system_prompt(&mem);
        assert!(
            prompt.contains("evolve"),
            "prompt should mention evolve.sh background sessions: {prompt}"
        );
    }

    #[test]
    fn test_build_prompt_with_memory_includes_turn_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = ConversationMemory::new(dir.path().join("conv.json"));
        mem.push("user", "check the disk usage please", "telegram");
        mem.push("assistant", "Disk is at 45%.", "telegram");
        let prompt = build_listener_system_prompt(&mem);
        assert!(
            prompt.contains("disk usage"),
            "prompt should include turn text: {prompt}"
        );
        assert!(
            prompt.contains("45%"),
            "prompt should include assistant response: {prompt}"
        );
    }

    #[test]
    fn test_build_prompt_with_memory_has_context_section() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = ConversationMemory::new(dir.path().join("conv.json"));
        mem.push("user", "hello", "telegram");
        let prompt = build_listener_system_prompt(&mem);
        assert!(
            prompt.contains("Recent Conversations"),
            "prompt with turns should have context header"
        );
    }

    // ── format_duration helper ────────────────────────────────────────────────

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(0), "0m");
    }

    #[test]
    fn test_format_duration_minutes_only() {
        assert_eq!(format_duration(2700), "45m"); // 45 minutes
    }

    #[test]
    fn test_format_duration_hours_and_minutes() {
        assert_eq!(format_duration(12000), "3h 20m");
    }

    // ── run_listener unit tests ───────────────────────────────────────────────

    /// Verify default ListenerConfig values for poll interval and response length.
    #[test]
    fn test_listener_config_default_poll_and_response() {
        let cfg = ListenerConfig::default();
        assert_eq!(cfg.poll_interval_secs, 2, "default poll interval must be 2s");
        assert_eq!(cfg.max_response_chars, 3000, "default max_response_chars must be 3000");
    }

    /// Verify ListenerStats messages_handled increments correctly.
    #[test]
    fn test_listener_stats_increment() {
        let mut stats = ListenerStats::new();
        assert_eq!(stats.messages_handled, 0);
        stats.messages_handled += 1;
        assert_eq!(stats.messages_handled, 1);
        stats.messages_handled += 99;
        assert_eq!(stats.messages_handled, 100);
    }

    /// Verify build_listener_system_prompt includes recent turns when memory has turns.
    #[test]
    fn test_listener_system_prompt_with_recent_turns() {
        let dir = tempfile::tempdir().unwrap();
        let mut mem = ConversationMemory::new(dir.path().join("conv.json"));
        mem.push("user", "what is the weather?", "telegram");
        mem.push("assistant", "I cannot check the weather.", "telegram");
        let prompt = build_listener_system_prompt(&mem);
        assert!(
            prompt.contains("weather"),
            "prompt should include recent turn text: {prompt}"
        );
        assert!(
            prompt.contains("Recent Conversations"),
            "prompt should include context section header: {prompt}"
        );
        assert_eq!(
            mem.turns.len(),
            2,
            "memory should still have exactly 2 turns after building prompt"
        );
    }
}
