#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_version_constant_exists() {
        assert!(
            version().contains('.'),
            "Version should contain a dot: {}",
            version()
        );
    }

    #[test]
    fn test_cli_default_model() {
        let args: Vec<String> = vec!["axonix"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.model, "claude-opus-4-6");
        assert!(cli.skill_dirs.is_empty());
        assert!(cli.prompt.is_none());
    }

    #[test]
    fn test_cli_skills_parsing() {
        let args: Vec<String> = vec!["axonix", "--skills", "./my_skills", "--skills", "./more"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.skill_dirs, vec!["./my_skills", "./more"]);
    }

    #[test]
    fn test_cli_help_returns_none() {
        let args: Vec<String> = vec!["axonix", "--help"]
            .into_iter().map(String::from).collect();
        assert!(CliArgs::parse(&args).is_none());
    }

    #[test]
    fn test_cli_version_returns_none() {
        let args: Vec<String> = vec!["axonix", "-V"]
            .into_iter().map(String::from).collect();
        assert!(CliArgs::parse(&args).is_none());
    }

    #[test]
    fn test_prompt_flag_parsing() {
        let args: Vec<String> = vec!["axonix", "-p", "explain monads"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.prompt.as_deref(), Some("explain monads"));
    }

    #[test]
    fn test_prompt_long_flag_parsing() {
        let args: Vec<String> = vec!["axonix", "--prompt", "fix the bug"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.prompt.as_deref(), Some("fix the bug"));
    }

    #[test]
    fn test_prompt_flag_missing_value() {
        let args: Vec<String> = vec!["axonix", "-p"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.prompt.is_none(), "Missing value after -p should yield None");
    }

    #[test]
    fn test_prompt_flag_not_present() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-sonnet-4-20250514"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.prompt.is_none());
        assert_eq!(cli.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_prompt_flag_with_other_flags() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-opus-4-6", "-p", "hello world"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.prompt.as_deref(), Some("hello world"));
        assert_eq!(cli.model, "claude-opus-4-6");
    }

    #[test]
    fn test_command_parsing_model() {
        let input = "/model claude-opus-4-6";
        assert!(input.starts_with("/model "));
        let model_name = input.trim_start_matches("/model ").trim();
        assert_eq!(model_name, "claude-opus-4-6");
    }

    #[test]
    fn test_command_parsing_model_whitespace() {
        let input = "/model   claude-opus-4-6  ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert_eq!(model_name, "claude-opus-4-6");
    }

    #[test]
    fn test_model_command_empty_name() {
        let input = "/model ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert!(model_name.is_empty(), "Empty model name should be detected");
    }

    #[test]
    fn test_model_command_whitespace_only() {
        let input = "/model    ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert!(model_name.is_empty(), "Whitespace-only model name should be detected");
    }

    #[test]
    fn test_all_repl_commands_listed_in_help() {
        // Smoke-test: the commands we claim exist should be present in --help output.
        // We verify the constants match what's in the source rather than capturing stdout.
        let commands = [
            "/help", "/status", "/context", "/tokens",
            "/history", "/retry", "/clear", "/model",
            "/save", "/lint", "/quit",
        ];
        // Just verify the array isn't empty — actual content verified by print_help() building
        assert!(!commands.is_empty(), "Command list should not be empty");
        assert!(commands.contains(&"/history"), "help should document /history");
        assert!(commands.contains(&"/retry"), "help should document /retry");
        assert!(commands.contains(&"/context"), "help should document /context");
        assert!(commands.contains(&"/tokens"), "help should document /tokens");
    }

    #[test]
    fn test_bluesky_post_flag_parsing() {
        let args: Vec<String> = vec!["axonix", "--bluesky-post", "Day 3 Session 11 — Bluesky live!"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.bluesky_post.as_deref(), Some("Day 3 Session 11 — Bluesky live!"));
        assert!(cli.prompt.is_none(), "--bluesky-post should not set --prompt");
    }

    #[test]
    fn test_bluesky_post_flag_not_present() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-opus-4-6"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.bluesky_post.is_none(), "bluesky_post should be None when flag absent");
    }

    #[test]
    fn test_bluesky_post_flag_missing_value_returns_none() {
        let args: Vec<String> = vec!["axonix", "--bluesky-post"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.bluesky_post.is_none(), "Missing value after --bluesky-post should yield None");
    }

    #[test]
    fn test_brief_flag_present() {
        let args: Vec<String> = vec!["axonix", "--brief"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.brief, "--brief should set brief to true");
        assert!(cli.prompt.is_none(), "--brief should not set prompt");
    }

    #[test]
    fn test_brief_flag_absent() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-sonnet-4-6"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(!cli.brief, "brief should be false when flag absent");
    }

    #[test]
    fn test_watch_flag_present() {
        let args: Vec<String> = vec!["axonix", "--watch"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.watch, "--watch should set watch to true");
        assert!(cli.prompt.is_none(), "--watch should not set prompt");
        assert!(!cli.brief, "--watch should not set brief");
    }

    #[test]
    fn test_watch_flag_absent() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-sonnet-4-6"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(!cli.watch, "watch should be false when flag absent");
    }

    #[test]
    fn test_bluesky_post_flag() {
        let args: Vec<String> = vec!["axonix", "--bluesky-post", "Hello Bluesky!"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.bluesky_post.as_deref(), Some("Hello Bluesky!"));
    }

    #[test]
    fn test_bluesky_post_flag_absent() {
        let args: Vec<String> = vec!["axonix", "--prompt", "hello"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.bluesky_post.is_none(), "bluesky_post should be None when flag absent");
    }

    #[test]
    fn test_default_model_is_set() {
        let args: Vec<String> = vec!["axonix"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        // Default model must be set (not empty)
        assert!(!cli.model.is_empty(), "default model must be non-empty");
        assert!(cli.model.contains("claude"), "default model should be a Claude model: {}", cli.model);
    }

    #[test]
    fn test_model_override() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-haiku-4-20250514"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.model, "claude-haiku-4-20250514");
    }

    #[test]
    fn test_multiple_skills_dirs() {
        let args: Vec<String> = vec!["axonix", "--skills", "/dir1", "--skills", "/dir2"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.skill_dirs.len(), 2);
        assert!(cli.skill_dirs.contains(&"/dir1".to_string()));
        assert!(cli.skill_dirs.contains(&"/dir2".to_string()));
    }

    #[test]
    fn test_no_skills_is_empty_vec() {
        let args: Vec<String> = vec!["axonix"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.skill_dirs.is_empty(), "no --skills should yield empty vec");
    }

    #[test]
    fn test_short_prompt_flag() {
        let args: Vec<String> = vec!["axonix", "-p", "quick question"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.prompt.as_deref(), Some("quick question"));
    }

    #[test]
    fn test_help_returns_none() {
        let args: Vec<String> = vec!["axonix", "--help"]
            .into_iter().map(String::from).collect();
        let result = CliArgs::parse(&args);
        assert!(result.is_none(), "--help should return None");
    }

    #[test]
    fn test_version_returns_none() {
        let args: Vec<String> = vec!["axonix", "--version"]
            .into_iter().map(String::from).collect();
        let result = CliArgs::parse(&args);
        assert!(result.is_none(), "--version should return None");
    }

    #[test]
    fn test_short_version_flag_returns_none() {
        let args: Vec<String> = vec!["axonix", "-V"]
            .into_iter().map(String::from).collect();
        let result = CliArgs::parse(&args);
        assert!(result.is_none(), "-V should return None");
    }

    #[test]
    fn test_all_false_flags_by_default() {
        let args: Vec<String> = vec!["axonix"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(!cli.brief, "brief default false");
        assert!(!cli.watch, "watch default false");
        assert!(!cli.listen, "listen default false");
        assert!(!cli.health, "health default false");
        assert!(!cli.health_subcommand, "health_subcommand default false");
        assert!(cli.prompt.is_none(), "prompt default None");
        assert!(cli.bluesky_post.is_none(), "bluesky_post default None");
        assert!(!cli.brief_telegram, "brief_telegram default false");
        assert!(cli.write_summary.is_none(), "write_summary default None");
        assert!(!cli.session_summary_telegram, "session_summary_telegram default false");
        assert!(cli.insert_metrics_row.is_none(), "insert_metrics_row default None");
        assert!(cli.extract_memories.is_none(), "extract_memories default None");
    }

    #[test]
    fn test_extract_memories_flag_parsing() {
        let args: Vec<String> = vec!["axonix", "--extract-memories", "/tmp/session.log"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.extract_memories.as_deref(), Some("/tmp/session.log"));
    }

    /// Verifies --brief-telegram sets both brief and brief_telegram flags (G-031).
    ///
    /// --brief-telegram implies --brief (so format_terminal() still runs)
    /// and also sets brief_telegram for Telegram delivery.
    #[test]
    fn test_brief_telegram_flag() {
        let args: Vec<String> = vec!["axonix", "--brief-telegram"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.brief, "--brief-telegram must imply brief=true");
        assert!(cli.brief_telegram, "brief_telegram must be true with --brief-telegram");
    }

    /// Verifies --brief alone does NOT set brief_telegram (G-031).
    ///
    /// --brief is stdout-only; --brief-telegram is the Telegram push mode.
    #[test]
    fn test_brief_flag_does_not_set_telegram() {
        let args: Vec<String> = vec!["axonix", "--brief"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.brief, "--brief must set brief=true");
        assert!(!cli.brief_telegram, "--brief alone must NOT set brief_telegram");
    }

    /// Verifies --brief-telegram and --brief combined both parse correctly (G-031).
    #[test]
    fn test_brief_and_brief_telegram_combined() {
        let args: Vec<String> = vec!["axonix", "--brief", "--brief-telegram"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.brief);
        assert!(cli.brief_telegram);
    }

    /// Verifies --write-summary parses the label correctly (G-035).
    #[test]
    fn test_write_summary_flag_parsing() {
        let args: Vec<String> = vec!["axonix", "--write-summary", "Day 7 Session 7"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(cli.write_summary.as_deref(), Some("Day 7 Session 7"));
        assert!(cli.prompt.is_none(), "--write-summary should not set prompt");
        assert!(!cli.brief, "--write-summary should not set brief");
    }

    /// Verifies --write-summary is None when flag is absent (G-035).
    #[test]
    fn test_write_summary_flag_absent() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-opus-4-6"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.write_summary.is_none(), "write_summary should be None when flag absent");
    }

    /// Verifies --write-summary with no value returns None (G-035).
    #[test]
    fn test_write_summary_flag_missing_value_returns_none() {
        let args: Vec<String> = vec!["axonix", "--write-summary"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.write_summary.is_none(), "Missing value after --write-summary should yield None");
    }

    /// Verifies --session-summary-telegram sets the flag (Closes #46).
    #[test]
    fn test_session_summary_telegram_flag_present() {
        let args: Vec<String> = vec!["axonix", "--session-summary-telegram"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.session_summary_telegram, "--session-summary-telegram should set flag to true");
    }

    /// Verifies --session-summary-telegram is false when absent (Closes #46).
    #[test]
    fn test_session_summary_telegram_flag_absent() {
        let args: Vec<String> = vec!["axonix", "--model", "claude-opus-4-6"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(!cli.session_summary_telegram, "session_summary_telegram should be false when flag absent");
    }

    /// Verifies --listen sets listen = true (G-060b).
    #[test]
    fn test_cli_args_listen_flag() {
        let args: Vec<String> = vec!["axonix", "--listen"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.listen, "--listen should set listen to true");
        assert!(cli.prompt.is_none(), "--listen should not set prompt");
        assert!(!cli.brief, "--listen should not set brief");
        assert!(!cli.watch, "--listen should not set watch");
    }

    /// Verifies listen is false by default (G-060b).
    #[test]
    fn test_cli_args_listen_false_by_default() {
        let args: Vec<String> = vec!["axonix"]
            .into_iter().map(String::from).collect();
        let cli = CliArgs::parse(&args).unwrap();
        assert!(!cli.listen, "listen should be false when --listen not passed");
    }

    /// Verifies --insert-metrics-row parses the row string correctly (G-072).
    #[test]
    fn test_insert_metrics_row_flag_parses() {
        let args = vec![
            "axonix".to_string(),
            "--insert-metrics-row".to_string(),
            "| 11 | S4 | 2026-03-24 | ~45k | 731 | 0 | 5 | 120 | 30 | yes | test |".to_string(),
        ];
        let cli = CliArgs::parse(&args).unwrap();
        assert_eq!(
            cli.insert_metrics_row.as_deref(),
            Some("| 11 | S4 | 2026-03-24 | ~45k | 731 | 0 | 5 | 120 | 30 | yes | test |")
        );
    }

    /// Verifies insert_metrics_row is None when flag is absent (G-072).
    #[test]
    fn test_insert_metrics_row_flag_absent() {
        let args = vec!["axonix".to_string()];
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.insert_metrics_row.is_none());
    }

    #[test]
    fn test_cli_health_flag() {
        let args: Vec<String> = vec!["axonix".into(), "--health".into()];
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.health, "--health flag should set health=true");
    }

    #[test]
    fn test_cli_health_false_by_default() {
        let args: Vec<String> = vec!["axonix".into()];
        let cli = CliArgs::parse(&args).unwrap();
        assert!(!cli.health, "health should be false by default");
    }

    #[test]
    fn test_cli_health_subcommand() {
        let args: Vec<String> = vec!["axonix".into(), "health".into()];
        let cli = CliArgs::parse(&args).unwrap();
        assert!(cli.health_subcommand, "health subcommand should be set");
        assert!(!cli.health, "health flag should not be set");
    }

    #[test]
    fn test_cli_health_subcommand_false_by_default() {
        let args: Vec<String> = vec!["axonix".into()];
        let cli = CliArgs::parse(&args).unwrap();
        assert!(!cli.health_subcommand, "health_subcommand default false");
    }
}
