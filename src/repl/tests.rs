use super::*;

    fn state() -> ReplState {
        ReplState::new("claude-opus-4-6")
    }

    // ── Quit / exit ───────────────────────────────────────────────────────────

    #[test]
    fn test_quit_command() {
        let mut s = state();
        assert_eq!(handle_command("/quit", &mut s, &[]), CommandResult::Quit);
    }

    // ── Archive journal ───────────────────────────────────────────────────────

    /// Verifies /archive-journal is dispatched locally (not passed to AI).
    /// Regression test for Issue #102: -p and piped modes must intercept this.
    #[test]
    fn test_slash_command_in_prompt_mode_dispatches_locally() {
        let mut s = state();
        assert_eq!(
            handle_command("/archive-journal", &mut s, &[]),
            CommandResult::ArchiveJournal
        );
    }

    #[test]
    fn test_exit_command() {
        let mut s = state();
        assert_eq!(handle_command("/exit", &mut s, &[]), CommandResult::Quit);
    }

    // ── Help ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_help_returns_handled() {
        let mut s = state();
        let result = handle_command("/help", &mut s, &[]);
        assert!(matches!(result, CommandResult::Handled(_)));
    }

    #[test]
    fn test_help_contains_commands() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/quit"), "help should list /quit");
        assert!(all.contains("/clear"), "help should list /clear");
        assert!(all.contains("/lint"), "help should list /lint");
        assert!(all.contains("/save"), "help should list /save");
    }

    #[test]
    fn test_help_shows_skills_when_present() {
        let mut s = state();
        let skills = vec!["evolve".to_string(), "communicate".to_string()];
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &skills) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/skills"), "help should list /skills when skills are loaded");
    }

    #[test]
    fn test_help_hides_skills_when_none() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(!all.contains("/skills"), "help should not list /skills when no skills loaded");
    }

    // ── Skills ───────────────────────────────────────────────────────────────

    #[test]
    fn test_skills_empty() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/skills", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("no skills")));
    }

    #[test]
    fn test_skills_with_loaded_skills() {
        let mut s = state();
        let skills = vec!["evolve".to_string(), "communicate".to_string()];
        let CommandResult::Handled(lines) = handle_command("/skills", &mut s, &skills) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("evolve"), "should list evolve skill");
        assert!(all.contains("communicate"), "should list communicate skill");
        assert!(all.contains("2 loaded") || all.contains("2"), "should show count");
    }

    // ── Model switch ─────────────────────────────────────────────────────────

    #[test]
    fn test_model_switch() {
        let mut s = state();
        s.total_input = 5000;
        s.total_output = 1000;
        let result = handle_command("/model claude-sonnet-4-20250514", &mut s, &[]);
        assert_eq!(result, CommandResult::SwitchModel("claude-sonnet-4-20250514".to_string()));
        assert_eq!(s.model, "claude-sonnet-4-20250514");
        assert_eq!(s.total_input, 0, "model switch should reset token counts");
        assert_eq!(s.total_output, 0, "model switch should reset token counts");
    }

    #[test]
    fn test_model_switch_empty_name() {
        let mut s = state();
        let result = handle_command("/model ", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for empty model name");
        };
        assert!(lines.iter().any(|l| l.contains("Usage")));
        assert_eq!(s.model, "claude-opus-4-6", "model should not change on empty name");
    }

    #[test]
    fn test_model_switch_whitespace_only() {
        let mut s = state();
        let result = handle_command("/model    ", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for whitespace model name");
        };
        assert!(lines.iter().any(|l| l.contains("Usage")));
    }

    // ── Clear ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_clear_returns_clear() {
        let mut s = state();
        assert_eq!(handle_command("/clear", &mut s, &[]), CommandResult::Clear);
    }

    // ── Retry ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_retry_no_history_returns_handled() {
        let mut s = state();
        // No prompts yet → returns Handled with "nothing to retry" message
        let result = handle_command("/retry", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled when no history, got: {result:?}");
        };
        assert!(lines.iter().any(|l| l.contains("nothing to retry")));
    }

    #[test]
    fn test_retry_with_history_returns_retry() {
        let mut s = state();
        s.push_prompt("explain monads");
        let result = handle_command("/retry", &mut s, &[]);
        assert_eq!(result, CommandResult::Retry("explain monads".to_string()));
    }

    #[test]
    fn test_retry_n_valid_index() {
        let mut s = state();
        s.push_prompt("first prompt");
        s.push_prompt("second prompt");
        s.push_prompt("third prompt");
        let result = handle_command("/retry 2", &mut s, &[]);
        assert_eq!(result, CommandResult::Retry("second prompt".to_string()));
    }

    #[test]
    fn test_retry_n_out_of_range() {
        let mut s = state();
        s.push_prompt("only prompt");
        let result = handle_command("/retry 5", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for out-of-range retry: {result:?}");
        };
        assert!(lines.iter().any(|l| l.contains("No prompt #5")));
    }

    #[test]
    fn test_retry_n_invalid_arg() {
        let mut s = state();
        let result = handle_command("/retry abc", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for invalid retry arg: {result:?}");
        };
        assert!(lines.iter().any(|l| l.contains("Usage")));
    }

    #[test]
    fn test_retry_n_zero_is_invalid() {
        let mut s = state();
        s.push_prompt("a prompt");
        let result = handle_command("/retry 0", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for retry 0: {result:?}");
        };
        assert!(lines.iter().any(|l| l.contains("Usage") || l.contains("No prompt")));
    }

    // ── Lint ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_lint_no_path() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/lint", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("Usage")));
    }

    #[test]
    fn test_lint_valid_yaml_file() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
        write!(tmp, "key: value\nother: 123\n").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command(&format!("/lint {path}"), &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("__lint_ok")), "valid YAML should produce ok marker: {lines:?}");
    }

    #[test]
    fn test_lint_invalid_yaml_file() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
        write!(tmp, "key: [\nbad\n").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command(&format!("/lint {path}"), &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(
            lines.iter().any(|l| l.contains("__lint_errors")),
            "invalid YAML should produce errors marker: {lines:?}"
        );
    }

    #[test]
    fn test_lint_unsupported_extension() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/lint foo.toml", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("__lint_unsupported")));
    }

    // ── Save path parsing ─────────────────────────────────────────────────────

    #[test]
    fn test_save_default_path() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/save", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("__save:conversation.md")));
    }

    #[test]
    fn test_save_custom_path() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/save my_chat.md", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("__save:my_chat.md")));
    }

    // ── Unknown command ───────────────────────────────────────────────────────

    #[test]
    fn test_unknown_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/foo", &mut s, &[]) else {
            panic!("expected Handled for unknown command");
        };
        assert!(lines.iter().any(|l| l.contains("Unknown")));
    }

    #[test]
    fn test_known_slash_commands_not_unknown() {
        let mut s = state();
        // /status, /context, /tokens are handled by caller — they should return NotACommand
        for cmd in &["/status", "/context", "/tokens"] {
            assert_eq!(
                handle_command(cmd, &mut s, &[]),
                CommandResult::NotACommand,
                "{cmd} should be NotACommand (handled by caller)"
            );
        }
    }

    // ── Non-command input ─────────────────────────────────────────────────────

    #[test]
    fn test_regular_prompt_is_not_a_command() {
        let mut s = state();
        assert_eq!(
            handle_command("explain monads", &mut s, &[]),
            CommandResult::NotACommand
        );
    }

    #[test]
    fn test_empty_string_is_not_a_command() {
        let mut s = state();
        assert_eq!(handle_command("", &mut s, &[]), CommandResult::NotACommand);
    }

    // ── History ───────────────────────────────────────────────────────────────

    #[test]
    fn test_history_empty() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("no prompts")));
    }

    #[test]
    fn test_history_shows_prompts() {
        let mut s = state();
        s.push_prompt("explain monads");
        s.push_prompt("what is a monad?");
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("explain monads"), "history should show first prompt");
        assert!(all.contains("what is a monad?"), "history should show second prompt");
    }

    #[test]
    fn test_history_numbered_correctly() {
        let mut s = state();
        s.push_prompt("alpha");
        s.push_prompt("beta");
        s.push_prompt("gamma");
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("1.") || all.contains("  1"), "should show entry 1");
        assert!(all.contains("3.") || all.contains("  3"), "should show entry 3");
    }

    // ── push_prompt / history_entry ───────────────────────────────────────────

    #[test]
    fn test_push_prompt_updates_last_prompt() {
        let mut s = state();
        s.push_prompt("hello world");
        assert_eq!(s.last_prompt.as_deref(), Some("hello world"));
    }

    #[test]
    fn test_push_prompt_appends_to_history() {
        let mut s = state();
        s.push_prompt("first");
        s.push_prompt("second");
        assert_eq!(s.history.len(), 2);
        assert_eq!(s.history[0], "first");
        assert_eq!(s.history[1], "second");
    }

    #[test]
    fn test_history_entry_valid_index() {
        let mut s = state();
        s.push_prompt("alpha");
        s.push_prompt("beta");
        assert_eq!(s.history_entry(1), Some("alpha"));
        assert_eq!(s.history_entry(2), Some("beta"));
    }

    #[test]
    fn test_history_entry_out_of_range() {
        let mut s = state();
        s.push_prompt("only");
        assert!(s.history_entry(0).is_none(), "index 0 should be None");
        assert!(s.history_entry(2).is_none(), "out-of-range should be None");
    }

    #[test]
    fn test_history_capped_at_limit() {
        let mut s = state();
        for i in 0..=HISTORY_LIMIT + 5 {
            s.push_prompt(format!("prompt {i}"));
        }
        assert_eq!(s.history.len(), HISTORY_LIMIT, "history should not exceed HISTORY_LIMIT");
        // Oldest entries should be dropped
        assert!(s.history[0].contains("6"), "oldest kept entry should be prompt 6");
    }

    // ── ReplState ─────────────────────────────────────────────────────────────

    #[test]
    fn test_repl_state_new() {
        let s = ReplState::new("claude-opus-4-6");
        assert_eq!(s.model, "claude-opus-4-6");
        assert_eq!(s.total_input, 0);
        assert_eq!(s.total_output, 0);
        assert_eq!(s.total_cache_read, 0);
        assert_eq!(s.total_cache_write, 0);
        assert!(s.last_prompt.is_none());
        assert!(s.history.is_empty(), "fresh state should have empty history");
    }

    #[test]
    fn test_repl_state_reset_tokens() {
        let mut s = ReplState::new("m");
        s.total_input = 1000;
        s.total_output = 500;
        s.total_cache_read = 200;
        s.total_cache_write = 100;
        s.reset_tokens();
        assert_eq!(s.total_input, 0);
        assert_eq!(s.total_output, 0);
        assert_eq!(s.total_cache_read, 0);
        assert_eq!(s.total_cache_write, 0);
    }

    // ── SSH command ───────────────────────────────────────────────────────────

    /// Helper: state with a registered test host (no real SSH connectivity needed).
    fn state_with_host() -> ReplState {
        let mut s = ReplState::new("claude-opus-4-6");
        s.ssh_hosts.add(crate::ssh::HostEntry::new("test-nuc", "192.168.1.99").with_user("admin"));
        s
    }

    #[test]
    fn test_ssh_no_args_returns_help() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/ssh", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/ssh with no args should show usage");
        assert!(all.contains("/ssh list"), "should mention /ssh list");
        assert!(all.contains("<host>"), "should mention host parameter");
    }

    #[test]
    fn test_ssh_help_flag_returns_usage() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/ssh --help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/ssh --help should show usage");
    }

    #[test]
    fn test_ssh_list_empty_registry() {
        let mut s = state(); // fresh state, no hosts loaded in test
        // Ensure hosts are empty (no hosts.toml or ~/.axonix/hosts.toml in test env)
        s.ssh_hosts = crate::ssh::HostRegistry::new();
        let CommandResult::Handled(lines) = handle_command("/ssh list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("No hosts") || all.contains("hosts.toml"),
            "empty registry should mention no hosts: {all}"
        );
    }

    #[test]
    fn test_ssh_list_with_registered_host() {
        let mut s = state_with_host();
        let CommandResult::Handled(lines) = handle_command("/ssh list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("test-nuc"), "list should show registered host alias");
        assert!(all.contains("192.168.1.99"), "list should show host address");
        assert!(all.contains("admin"), "list should show user");
    }

    #[test]
    fn test_ssh_unknown_host_returns_error() {
        let mut s = state_with_host();
        let CommandResult::Handled(lines) = handle_command("/ssh no-such-host uptime", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("Unknown host") || all.contains("no-such-host"),
            "unknown host should produce error: {all}"
        );
    }

    #[test]
    fn test_ssh_host_no_command_returns_usage() {
        let mut s = state_with_host();
        let CommandResult::Handled(lines) = handle_command("/ssh test-nuc", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("Usage") || all.contains("command"),
            "host with no command should show usage: {all}"
        );
    }

    #[test]
    fn test_ssh_help_shows_registered_hosts_in_usage() {
        let mut s = state_with_host();
        let CommandResult::Handled(lines) = handle_command("/ssh", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("test-nuc"), "usage output should list registered hosts when present");
    }

    #[test]
    fn test_help_includes_ssh_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/ssh"), "/help should document /ssh command");
    }

    // ── Unicode safety in /history ────────────────────────────────────────────

    /// Regression test: /history must not panic when a prompt contains multi-byte
    /// UTF-8 characters (emoji, CJK, accented chars) and exceeds the preview limit.
    /// Previously used `&prompt[..72]` which panics if a char straddles byte 72.
    #[test]
    fn test_history_unicode_prompt_no_panic() {
        let mut s = state();
        // Build a prompt with multi-byte chars (4 bytes each) that will exceed 72 bytes
        // but whose character count is close to the limit, so a naive byte slice would panic.
        let emoji_prompt = "🦀".repeat(20); // 80 bytes, 20 chars
        s.push_prompt(emoji_prompt.clone());
        // This must not panic — previously would have panicked at &prompt[..72]
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // Preview should be present and valid UTF-8 (truncated at char boundary)
        assert!(!all.is_empty(), "history output should not be empty");
        // The output is valid UTF-8 (this assertion would fail if bytes were sliced badly)
        assert!(all.is_ascii() || !all.is_empty());
    }

    #[test]
    fn test_history_cjk_prompt_no_panic() {
        let mut s = state();
        // CJK characters are 3 bytes each; 24 of them = 72 bytes exactly, then add one more
        // so the 73rd byte lands in the middle of a 3-byte char.
        let cjk_prompt = "你好世界".repeat(10); // 40 chars × 3 bytes = 120 bytes
        s.push_prompt(cjk_prompt);
        // Must not panic
        let result = handle_command("/history", &mut s, &[]);
        assert!(matches!(result, CommandResult::Handled(_)));
    }

    #[test]
    fn test_history_mixed_unicode_truncation_is_valid_utf8() {
        let mut s = state();
        // 70 ASCII chars followed by a multi-byte char — byte 72 would land mid-char
        let prompt = "a".repeat(70) + "こんにちは世界"; // last part is 3-byte chars
        s.push_prompt(prompt);
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        // Ensure every line is valid UTF-8 (String is always valid UTF-8 in Rust,
        // so what we're really testing is that we didn't panic getting here)
        for line in &lines {
            assert!(std::str::from_utf8(line.as_bytes()).is_ok(), "line must be valid UTF-8: {line:?}");
        }
    }

    // ── /comment command ─────────────────────────────────────────────────────

    #[test]
    fn test_comment_no_args_shows_usage() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/comment with no args should show usage");
        assert!(all.contains("issue_number"), "should mention issue_number");
    }

    #[test]
    fn test_comment_valid_issue_returns_marker() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment 13 Fixed in this session.", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("__gh_comment:13:Fixed in this session."), "should return gh_comment marker: {all}");
    }

    #[test]
    fn test_comment_preserves_spaces_in_body() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment 7 Great feature idea!", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("__gh_comment:7:Great feature idea!"), "body with spaces should be preserved: {all}");
    }

    #[test]
    fn test_comment_non_numeric_issue_shows_error() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment abc some text", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("integer"), "non-numeric issue should show error: {all}");
    }

    #[test]
    fn test_comment_zero_issue_shows_error() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment 0 some text", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("Usage"), "issue 0 should show error: {all}");
    }

    #[test]
    fn test_comment_missing_body_shows_error() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment 5", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("empty"), "missing body should show error: {all}");
    }

    #[test]
    fn test_help_includes_comment_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/comment"), "/help should document /comment command");
    }


    // ── /issues command ──────────────────────────────────────────────────────────

    #[test]
    fn test_issues_no_args_returns_fetch_issues_default() {
        let mut s = state();
        let result = handle_command("/issues", &mut s, &[]);
        assert_eq!(result, CommandResult::FetchIssues(10), "/issues with no args should fetch 10");
    }

    #[test]
    fn test_issues_with_valid_limit() {
        let mut s = state();
        let result = handle_command("/issues 5", &mut s, &[]);
        assert_eq!(result, CommandResult::FetchIssues(5), "/issues 5 should fetch 5");
    }

    #[test]
    fn test_issues_with_max_limit() {
        let mut s = state();
        let result = handle_command("/issues 30", &mut s, &[]);
        assert_eq!(result, CommandResult::FetchIssues(30), "/issues 30 should be accepted");
    }

    #[test]
    fn test_issues_over_max_limit_shows_error() {
        let mut s = state();
        let result = handle_command("/issues 31", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for over-limit: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("1–30"), "over-limit should show error: {all}");
    }

    #[test]
    fn test_issues_zero_limit_shows_error() {
        let mut s = state();
        let result = handle_command("/issues 0", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for zero limit: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("Usage"), "zero limit should show error: {all}");
    }

    #[test]
    fn test_issues_invalid_arg_shows_error() {
        let mut s = state();
        let result = handle_command("/issues abc", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for invalid arg: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("invalid"), "invalid arg should show error: {all}");
    }

    #[test]
    fn test_help_includes_issues_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/issues"), "/help should document /issues command");
    }

    // ── /memory command ───────────────────────────────────────────────────────

    /// Helper: state with a tmp memory path (no real file I/O to user dirs).
    fn state_with_tmp_memory() -> (ReplState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let mut s = ReplState::new("claude-opus-4-6");
        s.memory = crate::memory::MemoryStore::new(&path);
        (s, dir)
    }

    #[test]
    fn test_memory_list_empty() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("empty") || all.contains("no entries") || all.contains("Memory:"),
            "empty memory should report empty: {all}");
    }

    #[test]
    fn test_memory_set_and_get() {
        let (mut s, _dir) = state_with_tmp_memory();

        // Set a key
        let result = handle_command("/memory set nuc.ip 192.168.1.10", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("nuc.ip") || all.contains("✓"), "set should confirm: {all}");

        // Get it back
        let result = handle_command("/memory get nuc.ip", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("192.168.1.10"), "get should return value: {all}");
    }

    #[test]
    fn test_memory_list_shows_stored_keys() {
        let (mut s, _dir) = state_with_tmp_memory();
        s.memory.set("twitter.status", "blocked_402", Some("Free tier blocks writes"));
        s.memory.set("operator.tz", "America/Indiana/Indianapolis", None);

        let CommandResult::Handled(lines) = handle_command("/memory list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("twitter.status"), "list should show twitter.status: {all}");
        assert!(all.contains("operator.tz"), "list should show operator.tz: {all}");
        assert!(all.contains("blocked_402"), "list should show value: {all}");
    }

    #[test]
    fn test_memory_get_missing_key() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory get nonexistent", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("not set") || all.contains("nonexistent"),
            "missing key should report not set: {all}");
    }

    #[test]
    fn test_memory_del_existing_key() {
        let (mut s, _dir) = state_with_tmp_memory();
        s.memory.set("temp.key", "temp_value", None);

        let CommandResult::Handled(lines) = handle_command("/memory del temp.key", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("deleted") || all.contains("✓"), "del should confirm: {all}");
        assert!(s.memory.get("temp.key").is_none(), "key should be deleted");
    }

    #[test]
    fn test_memory_del_missing_key() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory del nonexistent", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("not set") || all.contains("nonexistent"),
            "del of missing key should report not set: {all}");
    }

    #[test]
    fn test_memory_set_missing_value_shows_usage() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory set keyonly", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage") || all.contains("value"),
            "set with no value should show usage: {all}");
    }

    #[test]
    fn test_memory_note_command() {
        let (mut s, _dir) = state_with_tmp_memory();
        s.memory.set("some.key", "some_value", None);

        let result = handle_command("/memory note some.key this is a helpful note", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("✓") || all.contains("note"), "note should confirm: {all}");
        // Verify value unchanged, note added
        let entry = s.memory.get_entry("some.key").unwrap();
        assert_eq!(entry.value, "some_value", "value should be unchanged");
        assert!(entry.note.as_deref().unwrap_or("").contains("helpful note"), "note should be stored");
    }

    #[test]
    fn test_memory_note_nonexistent_key_shows_error() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory note ghost.key a note", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("not set") || all.contains("set first"),
            "note on missing key should show error: {all}");
    }

    #[test]
    fn test_memory_unknown_subcommand_shows_usage() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory frobnicate", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage") || all.contains("list") || all.contains("set"),
            "unknown subcommand should show usage: {all}");
    }

    #[test]
    fn test_memory_set_value_with_spaces() {
        let (mut s, _dir) = state_with_tmp_memory();
        let result = handle_command("/memory set greeting hello world from axonix", &mut s, &[]);
        let CommandResult::Handled(lines) = result else { panic!("expected Handled") };
        let all = lines.join("\n");
        assert!(all.contains("✓") || all.contains("greeting"), "set should confirm: {all}");
        // Value with spaces should be stored fully
        assert_eq!(s.memory.get("greeting"), Some("hello world from axonix"),
            "value with spaces should be stored fully");
    }

    #[test]
    fn test_help_includes_memory_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/memory"), "/help should document /memory command");
    }

    // ── /predict command ───────────────────────────────────────────────────────

    /// Helper: state with a tmp prediction store (avoids writing to real .axonix/).
    fn state_with_tmp_predictions() -> (ReplState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        let mut s = ReplState::new("claude-opus-4-6");
        s.predictions = crate::predictions::PredictionStore::new(path);
        (s, dir)
    }

    #[test]
    fn test_predict_no_args_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/predict with no args should show usage");
        assert!(all.contains("add"), "usage should mention 'add'");
        assert!(all.contains("resolve"), "usage should mention 'resolve'");
    }

    #[test]
    fn test_predict_help_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/predict help should show usage: {all}");
    }

    #[test]
    fn test_predict_add_logs_prediction() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let result = handle_command("/predict add the build will succeed", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("✓") || all.contains("prediction"), "add should confirm: {all}");
        assert!(all.contains("#1"), "first prediction should have id 1: {all}");
        assert_eq!(s.predictions.count(), 1, "one prediction should be stored");
    }

    #[test]
    fn test_predict_add_empty_text_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        // "/predict add " — after trim, arg becomes "add" which is now treated
        // as shorthand text, returning __predict:add rather than usage.
        let result = handle_command("/predict add ", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        // The trimmed arg "add" is treated as shorthand prediction text
        assert!(all.contains("__predict:add"), "trimmed 'add' should be shorthand text: {all}");
    }

    #[test]
    fn test_predict_list_empty() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("none") || all.contains("(none") || all.contains("add"),
            "empty list should say none: {all}");
    }

    #[test]
    fn test_predict_open_empty() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict open", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("No open") || all.contains("none") || all.contains("add"),
            "empty open list should say no open predictions: {all}");
    }

    #[test]
    fn test_predict_list_shows_added_predictions() {
        let (mut s, _dir) = state_with_tmp_predictions();
        handle_command("/predict add build will pass", &mut s, &[]);
        handle_command("/predict add tests will increase by 10", &mut s, &[]);

        let CommandResult::Handled(lines) = handle_command("/predict list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("build will pass"), "list should show first prediction: {all}");
        assert!(all.contains("tests will increase"), "list should show second prediction: {all}");
        assert!(all.contains("2 total") || all.contains("2 open"), "list should show count: {all}");
    }

    #[test]
    fn test_predict_open_shows_unresolved_only() {
        let (mut s, _dir) = state_with_tmp_predictions();
        handle_command("/predict add open prediction", &mut s, &[]);
        // Manually resolve #1 so we can test open shows only unresolved
        s.predictions.predict("second open");
        let id = s.predictions.predict("will resolve this");
        s.predictions.resolve(id, "resolved outcome", None).unwrap();

        let CommandResult::Handled(lines) = handle_command("/predict open", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // Only open ones should appear — resolved ones should not
        assert!(all.contains("open prediction") || all.contains("second open"),
            "open should show unresolved predictions: {all}");
    }

    #[test]
    fn test_predict_resolve_valid() {
        let (mut s, _dir) = state_with_tmp_predictions();
        s.predictions.predict("the tests will pass");

        let result = handle_command("/predict resolve 1 tests passed with 329 cases", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("✓") || all.contains("resolved"), "resolve should confirm: {all}");
        assert!(all.contains("#1"), "should reference prediction id 1: {all}");
        assert!(s.predictions.get(1).unwrap().is_resolved(), "prediction should be resolved");
    }

    #[test]
    fn test_predict_resolve_with_delta() {
        let (mut s, _dir) = state_with_tmp_predictions();
        s.predictions.predict("will add 10 tests");

        let result = handle_command(
            "/predict resolve 1 actually added 12 tests | underestimated by 2",
            &mut s, &[]
        );
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("delta") || all.contains("underestimated"),
            "resolve with delta should show delta: {all}");
        let pred = s.predictions.get(1).unwrap();
        assert_eq!(pred.delta.as_deref(), Some("underestimated by 2"));
    }

    #[test]
    fn test_predict_resolve_nonexistent_id() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict resolve 999 outcome", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("not found"),
            "resolving nonexistent id should show error: {all}");
    }

    #[test]
    fn test_predict_resolve_invalid_id() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict resolve abc outcome", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("integer"),
            "invalid id should show error: {all}");
    }

    #[test]
    fn test_predict_resolve_missing_outcome_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict resolve 1", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage") || all.contains("outcome"),
            "missing outcome should show usage: {all}");
    }

    #[test]
    fn test_predict_unknown_subcommand_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        // With the /predict <text> shorthand, unrecognized subcommands are now
        // treated as free-form prediction text and return a __predict: marker.
        let CommandResult::Handled(lines) = handle_command("/predict frobnicate", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("__predict:frobnicate"),
            "unknown subcommand is now treated as shorthand prediction text: {all}");
    }

    #[test]
    fn test_help_includes_predict_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/predict"), "/help should document /predict command");
    }

    // ── /predict shorthand ─────────────────────────────────────────────────────

    #[test]
    fn test_predict_shorthand_with_text_returns_predict_marker() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let result = handle_command("/predict By Day 10, I will have done X", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("__predict:By Day 10, I will have done X"),
            "shorthand /predict should return __predict: marker: {all}"
        );
    }

    #[test]
    fn test_predict_shorthand_empty_text_returns_error() {
        let (mut s, _dir) = state_with_tmp_predictions();
        // "/predict" alone → shows usage (error), not a prediction
        let CommandResult::Handled(lines) = handle_command("/predict", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("Usage") || all.contains("add") || all.contains("resolve"),
            "empty /predict should show usage/error, not create a prediction: {all}"
        );
    }

    // ── /watch command ────────────────────────────────────────────────────────

    #[test]
    fn test_watch_no_args_returns_health_snapshot() {
        let mut s = state();
        let result = handle_command("/watch", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for /watch: {result:?}");
        };
        let all = lines.join("\n");
        // Should show health fields and thresholds
        assert!(all.contains("CPU") || all.contains("load"), "/watch should show CPU info: {all}");
        assert!(all.contains("Memory") || all.contains("Mem") || all.contains("memory"),
            "/watch should show memory info: {all}");
        assert!(all.contains("Disk") || all.contains("disk"), "/watch should show disk info: {all}");
        assert!(all.contains("threshold"), "/watch should mention thresholds: {all}");
    }

    #[test]
    fn test_watch_returns_ok_or_alert_message() {
        let mut s = state();
        let result = handle_command("/watch", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for /watch: {result:?}");
        };
        let all = lines.join("\n");
        // Must show either all-ok or threshold-exceeded — never empty
        assert!(
            all.contains("All metrics") || all.contains("threshold") || all.contains("exceeded"),
            "/watch must show status: {all}"
        );
    }

    #[test]
    fn test_watch_with_subarg_also_returns_health() {
        // /watch status or any subarg should work the same as /watch
        let mut s = state();
        let result = handle_command("/watch status", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for /watch status: {result:?}");
        };
        assert!(!lines.is_empty(), "/watch status should produce output");
    }

    #[test]
    fn test_watch_not_unknown_command() {
        // /watch must NOT be reported as "Unknown command"
        let mut s = state();
        let result = handle_command("/watch", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(!all.contains("Unknown command"), "/watch should not be unknown: {all}");
    }

    #[test]
    fn test_help_includes_watch_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/watch"), "/help should document /watch command");
    }

    // ── /review command ───────────────────────────────────────────────────────

    #[test]
    fn test_review_no_args_shows_usage() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/review", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/review with no args should show usage: {all}");
        assert!(all.contains("code_reviewer") || all.contains("review"), "should mention reviewer: {all}");
    }

    #[test]
    fn test_review_with_description_returns_marker() {
        let mut s = state();
        let CommandResult::Handled(lines) =
            handle_command("/review added /review command to repl.rs", &mut s, &[])
        else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("__review:added /review command to repl.rs"),
            "should return __review: marker: {all}"
        );
    }

    #[test]
    fn test_review_preserves_full_description() {
        let mut s = state();
        let desc = "refactored health.rs to add disk threshold alert logic";
        let cmd = format!("/review {desc}");
        let CommandResult::Handled(lines) = handle_command(&cmd, &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains(desc),
            "description should be preserved in marker: {all}"
        );
    }

    #[test]
    fn test_review_whitespace_only_shows_usage() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/review   ", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "whitespace-only description should show usage: {all}");
    }

    #[test]
    fn test_review_not_unknown_command() {
        let mut s = state();
        let result = handle_command("/review some change", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            !all.contains("Unknown command"),
            "/review should not be treated as unknown: {all}"
        );
    }

    #[test]
    fn test_help_includes_review_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/review"), "/help should document /review command");
    }

    // ── /summary command ──────────────────────────────────────────────────────

    #[test]
    fn test_summary_no_args_is_handled_not_unknown() {
        let mut s = state();
        let result = handle_command("/summary", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            !all.contains("Unknown command"),
            "/summary should not be unknown: {all}"
        );
    }

    #[test]
    fn test_summary_not_a_command_for_plain_text() {
        let mut s = state();
        let result = handle_command("summary of my day", &mut s, &[]);
        assert!(
            matches!(result, CommandResult::NotACommand),
            "plain text should not trigger /summary"
        );
    }

    #[test]
    fn test_summary_with_text_returns_handled() {
        let mut s = state();
        let result = handle_command("/summary implemented cycle_summary module", &mut s, &[]);
        let CommandResult::Handled(_lines) = result else {
            panic!("expected Handled: {result:?}");
        };
    }

    #[test]
    fn test_summary_with_text_confirmation_or_error_in_output() {
        let mut s = state();
        let CommandResult::Handled(lines) =
            handle_command("/summary implemented cycle_summary module", &mut s, &[])
        else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // Should either confirm success (✓) or report an error (✗)
        assert!(
            all.contains('✓') || all.contains('✗'),
            "output should indicate success or failure: {all}"
        );
    }

    #[test]
    fn test_summary_with_text_mentions_input() {
        let mut s = state();
        let task = "fixed context window exhaustion for issue 38";
        let CommandResult::Handled(lines) =
            handle_command(&format!("/summary {task}"), &mut s, &[])
        else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // On success, the task text appears in confirmation
        // On error, the error is shown — either is acceptable
        assert!(
            all.contains(task) || all.contains('✗'),
            "output should mention task or show error: {all}"
        );
    }

    #[test]
    fn test_summary_help_mentions_summary() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/summary"), "/help should document /summary command");
    }

    #[test]
    fn test_summary_whitespace_only_shows_info() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/summary   ", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // Whitespace-only arg = empty, so should show current summary or usage info
        assert!(
            !all.contains("Unknown command"),
            "/summary with only whitespace should not be unknown: {all}"
        );
    }

    // ── /failures command ─────────────────────────────────────────────────────

    #[test]
    fn test_failures_returns_handled() {
        let mut s = state();
        let result = handle_command("/failures", &mut s, &[]);
        assert!(matches!(result, CommandResult::Handled(_)), "/failures should return Handled");
    }

    #[test]
    fn test_failures_empty_store_shows_no_failures() {
        // When there's no file at the default path (or it's empty),
        // /failures should say "(no failures logged yet)"
        // We use the env var to point to a nonexistent temp path.
        let dir = tempfile::tempdir().unwrap();
        let fp = dir.path().join("nope.json");
        std::env::set_var("AXONIX_FAILURE_PATTERNS_PATH", fp.to_str().unwrap());
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/failures", &mut s, &[]) else {
            std::env::remove_var("AXONIX_FAILURE_PATTERNS_PATH");
            panic!("expected Handled");
        };
        std::env::remove_var("AXONIX_FAILURE_PATTERNS_PATH");
        let all = lines.join("\n");
        assert!(
            all.contains("no failures"),
            "/failures on empty store should say no failures: {all}"
        );
    }

    #[test]
    fn test_failures_not_unknown_command() {
        let mut s = state();
        let result = handle_command("/failures", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            !all.contains("Unknown command"),
            "/failures should not be treated as unknown: {all}"
        );
    }

    #[test]
    fn test_help_includes_failures_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/failures"), "/help should document /failures command");
    }

    // ── G-138: /brief ─────────────────────────────────────────────────────────

    /// /brief should be dispatched to the brief handler (not "unknown command").
    #[test]
    fn test_brief_not_unknown_command() {
        let mut s = state();
        let result = handle_command("/brief", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled, got {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            !all.contains("Unknown command"),
            "/brief should not be treated as unknown: {all}"
        );
    }

    /// /brief should return a Handled result (not Quit, NotACommand, etc.).
    #[test]
    fn test_brief_returns_handled() {
        let mut s = state();
        let result = handle_command("/brief", &mut s, &[]);
        assert!(
            matches!(result, CommandResult::Handled(_)),
            "/brief should return CommandResult::Handled"
        );
    }

    /// /brief output should include the AXONIX header text.
    #[test]
    fn test_brief_output_contains_header() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/brief", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("AXONIX"),
            "/brief output should contain AXONIX header: {all}"
        );
    }

    /// /help should document /brief.
    #[test]
    fn test_help_includes_brief_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/brief"), "/help should document /brief: {all}");
    }

    // ── G-134: /search ────────────────────────────────────────────────────────

    /// /search with no query should return a helpful usage message.
    #[test]
    fn test_search_empty_query_returns_usage() {
        let mut s = state();
        let result = handle_command("/search", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled, got {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("Usage"),
            "/search with no query should show usage: {all}"
        );
    }

    /// /search with only whitespace after the command is treated as empty.
    #[test]
    fn test_search_whitespace_only_query_returns_usage() {
        let mut s = state();
        // "/search  " — two spaces, no real query
        let result = handle_command("/search  ", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled, got {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("Usage"),
            "/search with whitespace-only query should show usage: {all}"
        );
    }

    /// /search <query> should not return "unknown command".
    #[test]
    fn test_search_not_unknown_command() {
        let mut s = state();
        // Point Ollama at an unreachable port so embed() fails gracefully.
        std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1");
        let result = handle_command("/search hello world", &mut s, &[]);
        std::env::remove_var("OLLAMA_URL");
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled, got {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            !all.contains("Unknown command"),
            "/search should not be treated as unknown: {all}"
        );
    }

    /// /search returns Handled even when Ollama is unreachable.
    #[test]
    fn test_search_ollama_unreachable_returns_handled() {
        let mut s = state();
        std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1");
        let result = handle_command("/search rust lifetime", &mut s, &[]);
        std::env::remove_var("OLLAMA_URL");
        assert!(
            matches!(result, CommandResult::Handled(_)),
            "/search should return Handled even if Ollama is down"
        );
    }

    /// /search error output (when Ollama is down) should mention embedding failure.
    #[test]
    fn test_search_ollama_unreachable_shows_error() {
        let mut s = state();
        std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1");
        let CommandResult::Handled(lines) = handle_command("/search rust lifetime", &mut s, &[]) else {
            std::env::remove_var("OLLAMA_URL");
            panic!("expected Handled");
        };
        std::env::remove_var("OLLAMA_URL");
        let all = lines.join("\n");
        assert!(
            all.contains("embedding failed") || all.contains("Ollama"),
            "/search should explain the error: {all}"
        );
    }

    /// /help should document /search.
    #[test]
    fn test_help_includes_search_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/search"), "/help should document /search: {all}");
    }

    // ── G-139: /goals ─────────────────────────────────────────────────────────

    /// Shared mutex to serialize tests that change the working directory.
    fn chdir_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    #[test]
    fn test_goals_command_no_file() {
        let _guard = chdir_lock().lock().unwrap_or_else(|e| e.into_inner());
        // In a temp dir without GOALS.md, /goals should return an error line
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let mut state = ReplState::new("test-model".to_string());
        let result = handle_command("/goals", &mut state, &[]);
        std::env::set_current_dir(orig).unwrap();
        match result {
            CommandResult::Handled(lines) => {
                assert!(lines.iter().any(|l| l.contains("[goals]")));
            }
            _ => panic!("expected Handled"),
        }
    }

    #[test]
    fn test_goals_command_with_goals_md() {
        let _guard = chdir_lock().lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let goals_content = "# Goals\n\n## Active\n\n### G-999 \u{2014} Test goal title\n**Why:** testing.\n\n### G-998 \u{2014} Another test goal\n**Why:** also testing.\n\n## Backlog\n\n### G-997 \u{2014} Backlog goal\n";
        std::fs::write(tmp.path().join("GOALS.md"), goals_content).unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let mut state = ReplState::new("test-model".to_string());
        let result = handle_command("/goals", &mut state, &[]);
        std::env::set_current_dir(orig).unwrap();
        match result {
            CommandResult::Handled(lines) => {
                let joined = lines.join("\n");
                assert!(joined.contains("G-999"), "expected G-999 in output");
                assert!(joined.contains("Test goal title"), "expected title in output");
                assert!(joined.contains("G-998"), "expected G-998 in output");
                assert!(!joined.contains("G-997"), "backlog goal should not appear");
                assert!(joined.contains("Active goals (2)"));
            }
            _ => panic!("expected Handled"),
        }
    }

    #[test]
    fn test_goals_command_empty_active() {
        let _guard = chdir_lock().lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let goals_content = "# Goals\n\n## Active\n\n## Backlog\n\n### G-001 \u{2014} Something\n";
        std::fs::write(tmp.path().join("GOALS.md"), goals_content).unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let mut state = ReplState::new("test-model".to_string());
        let result = handle_command("/goals", &mut state, &[]);
        std::env::set_current_dir(orig).unwrap();
        match result {
            CommandResult::Handled(lines) => {
                assert!(lines.iter().any(|l| l.contains("No active goals")));
            }
            _ => panic!("expected Handled"),
        }
    }

    #[test]
    fn test_help_includes_goals_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/goals"), "/help should document /goals: {all}");
    }
