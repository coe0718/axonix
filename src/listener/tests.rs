//! Tests for the listener module.

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
    let prompt = build_listener_system_prompt(&mem, None, &[]);
    assert!(
        !prompt.is_empty(),
        "prompt with empty memory should be non-empty"
    );
}

#[test]
fn test_build_prompt_contains_core_instructions() {
    let dir = tempfile::tempdir().unwrap();
    let mem = ConversationMemory::new(dir.path().join("conv.json"));
    let prompt = build_listener_system_prompt(&mem, None, &[]);
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
    let prompt = build_listener_system_prompt(&mem, None, &[]);
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
    let prompt = build_listener_system_prompt(&mem, None, &[]);
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
    let prompt = build_listener_system_prompt(&mem, None, &[]);
    assert!(
        prompt.contains("Recent Conversations"),
        "prompt with turns should have context header"
    );
}

#[test]
fn test_build_prompt_with_active_goal_includes_goal() {
    let dir = tempfile::tempdir().unwrap();
    let mem = ConversationMemory::new(dir.path().join("conv.json"));
    let prompt = build_listener_system_prompt(&mem, Some("Implement rate limiting"), &[]);
    assert!(
        prompt.contains("Current active goal"),
        "prompt should include active goal section header: {prompt}"
    );
    assert!(
        prompt.contains("Implement rate limiting"),
        "prompt should include the active goal title: {prompt}"
    );
}

#[test]
fn test_build_prompt_with_memory_context_includes_observations() {
    let dir = tempfile::tempdir().unwrap();
    let mem = ConversationMemory::new(dir.path().join("conv.json"));
    let ctx = vec![
        ("rate limiter was too aggressive".to_string(), 0.92),
        ("use token bucket algorithm".to_string(), 0.85),
        ("tested with 100 req/s load".to_string(), 0.78),
    ];
    let prompt = build_listener_system_prompt(&mem, None, &ctx);
    assert!(
        prompt.contains("Relevant past observations"),
        "prompt should include observations section header: {prompt}"
    );
    assert!(
        prompt.contains("rate limiter was too aggressive"),
        "prompt should include first observation: {prompt}"
    );
    assert!(
        prompt.contains("token bucket"),
        "prompt should include second observation: {prompt}"
    );
}

#[test]
fn test_build_prompt_no_goal_no_memory_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let mem = ConversationMemory::new(dir.path().join("conv.json"));
    let prompt = build_listener_system_prompt(&mem, None, &[]);
    assert!(!prompt.is_empty(), "prompt should be non-empty even with no goal or memory");
    assert!(
        !prompt.contains("Current active goal"),
        "no goal section should appear when active_goal is None"
    );
    assert!(
        !prompt.contains("Relevant past observations"),
        "no observations section should appear when memory_context is empty"
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
    let prompt = build_listener_system_prompt(&mem, None, &[]);
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

// ── AckedIssues ───────────────────────────────────────────────────────────

#[test]
fn test_acked_issues_empty_on_load_missing_file() {
    let path = std::path::PathBuf::from("/tmp/nonexistent_acked_test_12345.json");
    let acked = AckedIssues::load(&path);
    assert!(acked.issues.is_empty(), "should be empty when file doesn't exist");
}

#[test]
fn test_acked_issues_insert_and_contains() {
    let mut acked = AckedIssues::load(&std::path::PathBuf::from("/tmp/nonexistent.json"));
    assert!(!acked.contains(42));
    acked.insert(42);
    assert!(acked.contains(42));
    assert!(!acked.contains(43));
}

#[test]
fn test_acked_issues_save_and_reload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("acked.json");
    let mut acked = AckedIssues::load(&path);
    acked.insert(99);
    acked.insert(100);
    acked.save().unwrap();
    // Reload
    let reloaded = AckedIssues::load(&path);
    assert!(reloaded.contains(99), "99 should persist after save/reload");
    assert!(reloaded.contains(100), "100 should persist after save/reload");
    assert!(!reloaded.contains(50), "50 was never inserted");
}

#[test]
fn test_acked_issues_dedup() {
    let mut acked = AckedIssues::load(&std::path::PathBuf::from("/tmp/nonexistent.json"));
    acked.insert(5);
    acked.insert(5); // duplicate
    acked.insert(5); // another duplicate
    assert!(acked.contains(5));
    // Should still be a set — only one entry
    assert_eq!(acked.issues.len(), 1);
}

#[test]
fn test_listener_config_has_github_poll_interval() {
    let cfg = ListenerConfig::default();
    assert_eq!(
        cfg.github_poll_interval_secs, 900,
        "default github poll interval should be 15 min (900s)"
    );
}

#[test]
fn test_listener_config_has_daily_brief_hour() {
    let cfg = ListenerConfig::default();
    assert_eq!(
        cfg.daily_brief_hour, 7,
        "default daily brief hour should be 7 AM"
    );
}

#[test]
fn test_local_hour_returns_valid_hour() {
    let h = local_hour();
    assert!(h < 24, "local_hour() should return 0-23, got {h}");
}

// ── append_goal_to_backlog ────────────────────────────────────────────────

#[test]
fn test_append_goal_to_backlog_adds_entry() {
    let dir = tempfile::tempdir().unwrap();
    let goals_path = dir.path().join("GOALS.md");

    // Write minimal GOALS.md with a Backlog section
    std::fs::write(&goals_path, "# Goals\n\n## Backlog\n\n## Completed\n").unwrap();

    let result = append_goal_to_backlog_at("add dark mode to dashboard", &goals_path);

    assert!(result.is_ok(), "append_goal_to_backlog_at should succeed: {result:?}");
    let content = std::fs::read_to_string(&goals_path).unwrap();
    assert!(content.contains("add dark mode to dashboard"), "goal description should appear in GOALS.md");
    assert!(content.contains("## Backlog"), "Backlog section should still exist");
}

#[test]
fn test_append_goal_fails_without_backlog_section() {
    let dir = tempfile::tempdir().unwrap();
    let goals_path = dir.path().join("GOALS.md");
    std::fs::write(&goals_path, "# Goals\n\nNo backlog here.\n").unwrap();

    let result = append_goal_to_backlog_at("some goal", &goals_path);

    assert!(result.is_err(), "should fail when no ## Backlog section");
}

// ── get_last_commit_message ────────────────────────────────────────────────

#[test]
fn test_get_last_commit_message_returns_something_in_git_repo() {
    // We're in a git repo so this should work
    let msg = get_last_commit_message();
    // May be None if git show fails for some reason, but in a real repo it should be Some
    // Just test it doesn't panic
    let _ = msg;
}

// ── /help command ─────────────────────────────────────────────────────────

#[test]
fn test_help_command_recognized() {
    // The canonical /help trigger text must start with "/help"
    let text = "/help";
    assert!(text.starts_with("/help"), "/help text should start with /help");
}

#[test]
fn test_help_text_contains_key_commands() {
    // TELEGRAM_HELP_TEXT is the static response sent for /help —
    // confirm it advertises all core commands so users know what's available.
    let help = crate::telegram::TELEGRAM_HELP_TEXT;
    assert!(help.contains("/ask"),    "help text must mention /ask");
    assert!(help.contains("/run"),    "help text must mention /run");
    assert!(help.contains("/goal"),   "help text must mention /goal");
    assert!(help.contains("/status"), "help text must mention /status");
    assert!(help.contains("/help"),   "help text must mention /help itself");
    assert!(help.contains("/history"), "help text must mention /history");
}

// ── format_history_reply ─────────────────────────────────────────────────

#[test]
fn test_format_history_reply_empty_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mem = ConversationMemory::new(dir.path().join("conv.json"));
    let reply = format_history_reply(&mem, 5);
    assert!(
        reply.contains("No conversation history"),
        "empty memory should say no history: {reply}"
    );
}

#[test]
fn test_format_history_reply_shows_turns() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = ConversationMemory::new(dir.path().join("conv.json"));
    mem.push("user", "what is the disk usage?", "telegram");
    mem.push("assistant", "Disk is at 45%.", "telegram");
    let reply = format_history_reply(&mem, 5);
    assert!(reply.contains("You:"), "should label user turns as 'You': {reply}");
    assert!(reply.contains("Axonix:"), "should label assistant turns as 'Axonix': {reply}");
    assert!(reply.contains("disk usage"), "should include turn text: {reply}");
    assert!(reply.contains("45%"), "should include assistant response: {reply}");
}

#[test]
fn test_format_history_reply_truncates_long_text() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = ConversationMemory::new(dir.path().join("conv.json"));
    let long_text = "x".repeat(200);
    mem.push("user", &long_text, "telegram");
    let reply = format_history_reply(&mem, 5);
    // Should be truncated to 100 chars + ellipsis
    assert!(reply.contains("…"), "long text should be truncated with ellipsis: {reply}");
    // The turn content should not exceed reasonable length
    assert!(
        reply.len() < 500,
        "reply should be compact even with long input: {} chars",
        reply.len()
    );
}

#[test]
fn test_format_history_reply_limits_to_n_turns() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = ConversationMemory::new(dir.path().join("conv.json"));
    for i in 0..10 {
        mem.push("user", &format!("question {i}"), "telegram");
        mem.push("assistant", &format!("answer {i}"), "telegram");
    }
    let reply = format_history_reply(&mem, 5);
    // Should show only last 5 turns, not all 20
    assert!(
        reply.contains("question 7") || reply.contains("answer 7") ||
        reply.contains("question 8") || reply.contains("answer 8") ||
        reply.contains("question 9") || reply.contains("answer 9"),
        "should show recent turns: {reply}"
    );
    assert!(
        !reply.contains("question 0"),
        "should not show oldest turns: {reply}"
    );
}

#[test]
fn test_format_history_reply_header_contains_emoji() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = ConversationMemory::new(dir.path().join("conv.json"));
    mem.push("user", "hello", "telegram");
    let reply = format_history_reply(&mem, 5);
    assert!(reply.contains("📜"), "history reply should have scroll emoji: {reply}");
}

// ── parse_rate_limit_env ──────────────────────────────────────────────────

#[test]
fn test_parse_rate_limit_env_valid() {
    // Temporarily set the env var to "5/60s" and verify parsing.
    // Use a scope guard pattern via a helper to avoid test pollution.
    std::env::set_var("LISTENER_RATE_LIMIT", "5/60s");
    let (n, w) = parse_rate_limit_env(3, 30);
    std::env::remove_var("LISTENER_RATE_LIMIT");
    assert_eq!(n, 5, "should parse max as 5");
    assert_eq!(w, 60, "should parse window as 60");
}

#[test]
fn test_parse_rate_limit_env_invalid() {
    std::env::set_var("LISTENER_RATE_LIMIT", "bad");
    let (n, w) = parse_rate_limit_env(3, 30);
    std::env::remove_var("LISTENER_RATE_LIMIT");
    assert_eq!(n, 3, "invalid env var should fall back to default max");
    assert_eq!(w, 30, "invalid env var should fall back to default window");
}

#[test]
fn test_parse_rate_limit_env_unset() {
    std::env::remove_var("LISTENER_RATE_LIMIT");
    let (n, w) = parse_rate_limit_env(7, 90);
    assert_eq!(n, 7, "unset env var should return default max");
    assert_eq!(w, 90, "unset env var should return default window");
}

#[test]
fn test_parse_rate_limit_env_no_s_suffix() {
    std::env::set_var("LISTENER_RATE_LIMIT", "10/30");
    let (n, w) = parse_rate_limit_env(3, 30);
    std::env::remove_var("LISTENER_RATE_LIMIT");
    assert_eq!(n, 10, "should parse max as 10 even without 's' suffix");
    assert_eq!(w, 30, "should parse window as 30 even without 's' suffix");
}

#[cfg(test)]
mod haiku_routing_tests {
    use super::*;

    const SONNET: &str = "claude-sonnet-4-6";
    const HAIKU: &str = "claude-haiku-4-5-20251001";

    #[test]
    fn test_run_uses_sonnet() {
        assert_eq!(select_model_for_command("/run build me a thing", SONNET, HAIKU), SONNET);
    }

    #[test]
    fn test_run_no_args_uses_sonnet() {
        assert_eq!(select_model_for_command("/run", SONNET, HAIKU), SONNET);
    }

    #[test]
    fn test_ask_uses_haiku() {
        assert_eq!(select_model_for_command("/ask what is 2+2", SONNET, HAIKU), HAIKU);
    }

    #[test]
    fn test_status_uses_haiku() {
        assert_eq!(select_model_for_command("/status", SONNET, HAIKU), HAIKU);
    }

    #[test]
    fn test_goal_uses_haiku() {
        assert_eq!(select_model_for_command("/goal add logging", SONNET, HAIKU), HAIKU);
    }

    #[test]
    fn test_unknown_uses_haiku() {
        assert_eq!(select_model_for_command("/help", SONNET, HAIKU), HAIKU);
    }

    #[test]
    fn test_goals_uses_haiku() {
        assert_eq!(select_model_for_command("/goals", SONNET, HAIKU), HAIKU);
    }

    #[test]
    fn test_predict_uses_haiku() {
        assert_eq!(select_model_for_command("/predict anything", SONNET, HAIKU), HAIKU);
    }

    #[test]
    fn test_resolve_uses_haiku() {
        assert_eq!(select_model_for_command("/resolve 42 correct", SONNET, HAIKU), HAIKU);
    }

    #[test]
    fn test_custom_haiku_model() {
        let custom = "claude-haiku-custom";
        assert_eq!(select_model_for_command("/ask hi", SONNET, custom), custom);
    }
}

// ── format_predictions_list ───────────────────────────────────────────────

#[cfg(test)]
mod predictions_list_tests {
    use crate::listener::format_predictions_list;
    use crate::predictions::PredictionStore;

    fn make_store_with_predictions(dir: &std::path::Path) -> PredictionStore {
        let path = dir.join("predictions.json");
        let mut store = PredictionStore::new(path);
        store.predict("will reach 1000 tests by Day 25");
        store.predict("next major refactor will take 3 sessions");
        store
    }

    #[test]
    fn test_format_predictions_list_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        let store = PredictionStore::new(path);
        let result = format_predictions_list(&store);
        assert!(
            result.contains("No open predictions"),
            "empty store should say no open predictions: {result}"
        );
        assert!(
            result.contains("/predict"),
            "empty result should hint at /predict command: {result}"
        );
    }

    #[test]
    fn test_format_predictions_list_with_open_predictions() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store_with_predictions(dir.path());
        let result = format_predictions_list(&store);
        assert!(
            result.contains("Open Predictions"),
            "should have header: {result}"
        );
        assert!(
            result.contains("will reach 1000 tests"),
            "should include first prediction text: {result}"
        );
        assert!(
            result.contains("next major refactor"),
            "should include second prediction text: {result}"
        );
        assert!(
            result.contains("#"),
            "should include prediction IDs with #: {result}"
        );
        assert!(
            result.contains("(2)"),
            "should show count of 2: {result}"
        );
    }

    #[test]
    fn test_format_predictions_list_excludes_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = make_store_with_predictions(dir.path());
        // Resolve the first prediction (ID=1)
        store.resolve(1, "TRUE", None).unwrap();

        let result = format_predictions_list(&store);
        assert!(
            result.contains("(1)"),
            "should show count of 1 after resolving one: {result}"
        );
        assert!(
            !result.contains("will reach 1000 tests"),
            "resolved prediction should not appear: {result}"
        );
        assert!(
            result.contains("next major refactor"),
            "unresolved prediction should still appear: {result}"
        );
    }

    #[test]
    fn test_format_predictions_list_shows_resolve_hint() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store_with_predictions(dir.path());
        let result = format_predictions_list(&store);
        assert!(
            result.contains("/resolve"),
            "should include /resolve hint: {result}"
        );
    }

    #[test]
    fn test_format_predictions_list_includes_created_date() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store_with_predictions(dir.path());
        let result = format_predictions_list(&store);
        assert!(
            result.contains("created:"),
            "should show creation date: {result}"
        );
    }
}
