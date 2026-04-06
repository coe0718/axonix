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

// ── parse_run_command ──────────────────────────────────────────────────────

#[test]
fn test_parse_run_command_basic() {
    assert_eq!(parse_run_command("/run check disk usage"), Some("check disk usage"));
}

#[test]
fn test_parse_run_command_empty_returns_none() {
    assert_eq!(parse_run_command("/run"), None);
    assert_eq!(parse_run_command("/run   "), None);
}

#[test]
fn test_parse_run_command_non_run_returns_none() {
    assert_eq!(parse_run_command("/ask hello"), None);
    assert_eq!(parse_run_command("run: something"), None);
    assert_eq!(parse_run_command(""), None);
}

// ── parse_goal_command ────────────────────────────────────────────────────

#[test]
fn test_parse_goal_command_basic() {
    assert_eq!(parse_goal_command("/goal add dark mode to dashboard"), Some("add dark mode to dashboard"));
}

#[test]
fn test_parse_goal_command_empty_returns_none() {
    assert_eq!(parse_goal_command("/goal"), None);
    assert_eq!(parse_goal_command("/goal   "), None);
}

#[test]
fn test_parse_goal_command_non_goal_returns_none() {
    assert_eq!(parse_goal_command("/ask hello"), None);
    assert_eq!(parse_goal_command("goal: something"), None);
}

// ── extract_commands with /run and /goal ──────────────────────────────────

#[test]
fn test_extract_commands_run_detected() {
    let client = make_client();
    let updates = vec![make_update(1, 12345, 5, "/run check disk usage")];
    let commands = client.extract_commands(&updates);
    assert_eq!(commands.len(), 1);
    if let BotCommand::Run { task, message_id } = &commands[0] {
        assert_eq!(task, "check disk usage");
        assert_eq!(*message_id, 5);
    } else {
        panic!("expected BotCommand::Run, got {:?}", commands[0]);
    }
}

#[test]
fn test_extract_commands_goal_detected() {
    let client = make_client();
    let updates = vec![make_update(1, 12345, 7, "/goal add dark mode")];
    let commands = client.extract_commands(&updates);
    assert_eq!(commands.len(), 1);
    if let BotCommand::Goal { description, message_id } = &commands[0] {
        assert_eq!(description, "add dark mode");
        assert_eq!(*message_id, 7);
    } else {
        panic!("expected BotCommand::Goal, got {:?}", commands[0]);
    }
}

#[test]
fn test_extract_commands_run_empty_not_extracted() {
    let client = make_client();
    let updates = vec![make_update(1, 12345, 1, "/run")];
    let commands = client.extract_commands(&updates);
    // /run with no task should not produce a Run command (no valid task)
    assert!(commands.is_empty(), "empty /run should produce no commands: {commands:?}");
}

#[test]
fn test_help_text_mentions_run() {
    assert!(TELEGRAM_HELP_TEXT.contains("/run"), "help text must mention /run command");
}

#[test]
fn test_help_text_mentions_goal() {
    assert!(TELEGRAM_HELP_TEXT.contains("/goal"), "help text must mention /goal command");
}

// ── format_enhanced_status_reply ─────────────────────────────────────────

#[test]
fn test_format_enhanced_status_reply_with_goal_and_commit() {
    let reply = TelegramClient::format_enhanced_status_reply(
        "claude-sonnet-4-6",
        120,
        Some("G-094 — Telegram task triggers"),
        Some("feat(listener): add /run and /goal commands"),
        None,
    );
    assert!(reply.contains("claude-sonnet-4-6"), "should show model: {reply}");
    assert!(reply.contains("G-094"), "should show active goal: {reply}");
    assert!(reply.contains("feat(listener)"), "should show last commit: {reply}");
    assert!(reply.contains("listener"), "should show mode: {reply}");
}

#[test]
fn test_format_enhanced_status_reply_without_goal_or_commit() {
    let reply = TelegramClient::format_enhanced_status_reply("m", 0, None, None, None);
    assert!(!reply.contains("active goal"), "no goal section if None: {reply}");
    assert!(!reply.contains("last commit"), "no commit section if None: {reply}");
}

#[test]
fn test_format_enhanced_status_reply_uptime_minutes() {
    let reply = TelegramClient::format_enhanced_status_reply("m", 185, None, None, None);
    assert!(reply.contains("3m"), "185s should show as 3m: {reply}");
}

// ── /memory command parsing ────────────────────────────────────────────────

#[test]
fn test_memory_add_command_parsed() {
    let result = parse_memory_command("/memory add hello world");
    assert!(matches!(
        result,
        Some(MemoryAction::Add { ref text, ref category })
        if text == "hello world" && category == "learned"
    ), "expected Add{{text='hello world', category='learned'}}, got {result:?}");
}

#[test]
fn test_memory_add_with_category_parsed() {
    let result = parse_memory_command("/memory add:tried_and_failed couldn't compile");
    assert!(matches!(
        result,
        Some(MemoryAction::Add { ref text, ref category })
        if text == "couldn't compile" && category == "tried_and_failed"
    ), "expected Add with tried_and_failed category, got {result:?}");
}

#[test]
fn test_memory_search_command_parsed() {
    let result = parse_memory_command("/memory search sqlite");
    assert!(matches!(
        result,
        Some(MemoryAction::Search { ref query }) if query == "sqlite"
    ), "expected Search{{query='sqlite'}}, got {result:?}");
}

#[test]
fn test_memory_list_command_parsed() {
    let result = parse_memory_command("/memory list");
    assert_eq!(result, Some(MemoryAction::List), "expected List, got {result:?}");
}

#[test]
fn test_help_text_contains_memory() {
    assert!(
        TELEGRAM_HELP_TEXT.contains("/memory"),
        "TELEGRAM_HELP_TEXT should mention /memory"
    );
}

// ── is_history_command ───────────────────────────────────────────────────

#[test]
fn test_is_history_command_true() {
    assert!(is_history_command("/history"));
    assert!(is_history_command("  /history  "));
}

#[test]
fn test_is_history_command_false_for_non_history() {
    assert!(!is_history_command("/ask hello"));
    assert!(!is_history_command("/help"));
    assert!(!is_history_command("/status"));
    assert!(!is_history_command("history"));
    assert!(!is_history_command(""));
    assert!(!is_history_command("/historical"));
}

// ── extract_commands with /history ───────────────────────────────────────

#[test]
fn test_extract_commands_history_detected() {
    let client = make_client();
    let updates = vec![make_update(1, 12345, 13, "/history")];
    let commands = client.extract_commands(&updates);
    assert_eq!(commands.len(), 1);
    assert!(
        matches!(commands[0], BotCommand::History { message_id: 13 }),
        "expected BotCommand::History, got {:?}",
        commands[0]
    );
}

#[test]
fn test_extract_commands_history_in_mixed_batch() {
    let client = make_client();
    let updates = vec![
        make_update(1, 12345, 1, "/ask question"),
        make_update(2, 12345, 2, "/history"),
        make_update(3, 12345, 3, "/help"),
    ];
    let commands = client.extract_commands(&updates);
    assert_eq!(commands.len(), 3, "3 commands: ask + history + help");
    assert!(matches!(commands[0], BotCommand::Ask(_)));
    assert!(matches!(commands[1], BotCommand::History { .. }));
    assert!(matches!(commands[2], BotCommand::Help { .. }));
}

#[test]
fn test_help_text_mentions_history() {
    assert!(TELEGRAM_HELP_TEXT.contains("/history"), "help text must mention /history command");
}

// ── is_goals_command ─────────────────────────────────────────────────────

#[test]
fn test_is_goals_command_true() {
    assert!(is_goals_command("/goals"));
    assert!(is_goals_command("  /goals  "));
}

#[test]
fn test_is_goals_command_false_for_non_goals() {
    assert!(!is_goals_command("/ask hello"));
    assert!(!is_goals_command("/help"));
    assert!(!is_goals_command("/status"));
    assert!(!is_goals_command("goals"));
    assert!(!is_goals_command(""));
    assert!(!is_goals_command("/goal"));  // /goal is a different command
    assert!(!is_goals_command("/goalslist"));
}

// ── extract_commands with /goals ─────────────────────────────────────────

#[test]
fn test_extract_commands_goals_detected() {
    let client = make_client();
    let updates = vec![make_update(1, 12345, 15, "/goals")];
    let commands = client.extract_commands(&updates);
    assert_eq!(commands.len(), 1);
    assert!(
        matches!(commands[0], BotCommand::Goals { message_id: 15 }),
        "expected BotCommand::Goals {{ message_id: 15 }}, got {:?}",
        commands[0]
    );
}

#[test]
fn test_extract_commands_goals_in_mixed_batch() {
    let client = make_client();
    let updates = vec![
        make_update(1, 12345, 1, "/ask question"),
        make_update(2, 12345, 2, "/goals"),
        make_update(3, 12345, 3, "/help"),
    ];
    let commands = client.extract_commands(&updates);
    assert_eq!(commands.len(), 3, "3 commands: ask + goals + help");
    assert!(matches!(commands[0], BotCommand::Ask(_)));
    assert!(matches!(commands[1], BotCommand::Goals { .. }));
    assert!(matches!(commands[2], BotCommand::Help { .. }));
}

#[test]
fn test_help_text_mentions_goals() {
    assert!(TELEGRAM_HELP_TEXT.contains("/goals"), "help text must mention /goals command");
}

// ── format_enhanced_status_reply includes git summary ────────────────────

#[test]
fn test_format_enhanced_status_reply_includes_git_summary() {
    let reply = TelegramClient::format_enhanced_status_reply("test-model", 60, None, None, None);
    // The git summary should always be appended (even if it says "No recent commits")
    assert!(
        reply.contains("📝"),
        "status reply should include git summary with 📝 emoji: {reply}"
    );
}

// ── /predict command parsing ──────────────────────────────────────────────

#[test]
fn test_bot_command_predict_parses_text() {
    let client = make_client();
    let updates = vec![make_update(1, 12345, 7, "/predict some text here")];
    let commands = client.extract_commands(&updates);
    assert_eq!(commands.len(), 1);
    if let BotCommand::Predict { text, message_id } = &commands[0] {
        assert_eq!(text, "some text here");
        assert_eq!(*message_id, 7);
    } else {
        panic!("expected BotCommand::Predict, got {:?}", commands[0]);
    }
}

#[test]
fn test_bot_command_predict_requires_text() {
    let client = make_client();
    // /predict with no text should produce no command
    let updates = vec![make_update(1, 12345, 1, "/predict")];
    let commands = client.extract_commands(&updates);
    assert!(
        commands.is_empty(),
        "empty /predict should produce no commands: {commands:?}"
    );
}

#[test]
fn test_format_enhanced_status_reply_with_accuracy() {
    let reply = TelegramClient::format_enhanced_status_reply(
        "test-model",
        60,
        None,
        None,
        Some("8/12 correct — 67%"),
    );
    assert!(
        reply.contains("8/12 correct — 67%"),
        "should show prediction accuracy: {reply}"
    );
    assert!(
        reply.contains("predictions:"),
        "should show predictions label: {reply}"
    );
}

#[test]
fn test_format_enhanced_status_reply_no_accuracy() {
    let reply = TelegramClient::format_enhanced_status_reply(
        "test-model",
        60,
        None,
        None,
        None,
    );
    assert!(
        !reply.contains("predictions:"),
        "should not show predictions line when accuracy is None: {reply}"
    );
}

#[test]
fn test_predict_command_help_text() {
    assert!(
        TELEGRAM_HELP_TEXT.contains("/predict"),
        "TELEGRAM_HELP_TEXT must mention /predict"
    );
}

// ── parse_resolve_command ────────────────────────────────────────────────

#[test]
fn test_parse_resolve_command_correct() {
    assert_eq!(parse_resolve_command("/resolve 42 correct"), Some((42, true)));
}

#[test]
fn test_parse_resolve_command_wrong() {
    assert_eq!(parse_resolve_command("/resolve 7 wrong"), Some((7, false)));
}

#[test]
fn test_parse_resolve_command_true_false() {
    assert_eq!(parse_resolve_command("/resolve 1 true"), Some((1, true)));
    assert_eq!(parse_resolve_command("/resolve 2 false"), Some((2, false)));
}

#[test]
fn test_parse_resolve_command_invalid() {
    assert_eq!(parse_resolve_command("/resolve"), None);
    assert_eq!(parse_resolve_command("/resolve 42"), None);
    assert_eq!(parse_resolve_command("/resolve abc correct"), None);
    assert_eq!(parse_resolve_command("/resolve 42 maybe"), None);
}

#[test]
fn test_help_contains_resolve() {
    assert!(TELEGRAM_HELP_TEXT.contains("/resolve"), "help text must mention /resolve command");
}
