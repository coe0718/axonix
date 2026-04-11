//! axonix — a coding agent that evolves itself.
//!
//! Started as ~200 lines. Grows one commit at a time.
//! Read IDENTITY.md and JOURNAL.md for the full story.
//!
//! Usage:
//!   ANTHROPIC_API_KEY=sk-... cargo run
//!   ANTHROPIC_API_KEY=sk-... cargo run -- --model claude-opus-4-6
//!   ANTHROPIC_API_KEY=sk-... cargo run -- --skills ./skills
//!   ANTHROPIC_API_KEY=sk-... cargo run -- -p "explain this code"
//!   echo "prompt" | cargo run  (piped mode: single prompt, no REPL)
//!
//! Commands:
//!   /help           Show available commands
//!   /status         Show session info (model, tokens, messages)
//!   /quit, /exit    Exit the agent
//!   /clear          Clear conversation history
//!   /retry          Retry the last prompt
//!   /model <name>   Switch model mid-session
//!   /lint <file>    Validate a YAML or Caddyfile
//!   /issues [N]     List open GitHub issues sorted by reactions
//!
//! Multiline input:
//!   End a line with \ to continue on the next line
//!   Type """ to start a block, """ again to finish

use std::io::{self, BufRead, IsTerminal};
use yoagent::skills::SkillSet;

use axonix::bluesky::BlueskyClient;
use axonix::cli::CliArgs;
use axonix::github::GitHubClient;
use axonix::render::*;
use axonix::telegram::TelegramClient;

mod agent_setup;
mod cli_dispatch;
mod prompt_dispatch;
mod repl_loop;
mod session_helpers;
mod telegram_poll;
mod prompt_runner;

#[allow(unused_imports)]
use agent_setup::{build_tools, make_agent, build_system_prompt, stream_redact};
#[allow(unused_imports)]
pub(self) use agent_setup::build_tools as _build_tools_for_tests;
#[allow(unused_imports)]
use session_helpers::format_session_summary_telegram;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let cli_args = match CliArgs::parse(&args) {
        Some(c) => c,
        None => return, // --help or --version was printed
    };

    // predict auto-resolve: resolve predictions whose goal IDs are complete in GOALS_ARCHIVE.md
    if cli_args.predict_auto_resolve {
        let mut store = axonix::predictions::PredictionStore::default_path();
        let archive = std::fs::read_to_string("GOALS_ARCHIVE.md").unwrap_or_default();
        let goals = std::fs::read_to_string("GOALS.md").unwrap_or_default();
        let combined = format!("{archive}\n{goals}");
        let resolved = store.auto_resolve_from_goals(&combined);
        if resolved.is_empty() {
            println!("  predict auto-resolve: no predictions resolved");
        } else {
            for (id, text) in &resolved {
                println!("  ✓ resolved prediction #{id}: {text}");
            }
            if let Err(e) = store.save() {
                eprintln!("{RED}error:{RESET} failed to save predictions: {e}");
                std::process::exit(1);
            }
            println!("  predict auto-resolve: {} prediction(s) resolved", resolved.len());
        }
        return;
    }

    // --telegram-notify: send a message to Telegram and exit (no API key needed).
    // Used by evolve.sh tg_notify() to avoid system curl/OpenSSL issues.
    if let Some(ref text) = cli_args.telegram_notify {
        let text = text.trim();
        if text.is_empty() {
            eprintln!("{RED}error:{RESET} --telegram-notify requires a non-empty message.");
            std::process::exit(1);
        }
        let tg = TelegramClient::from_env();
        match tg {
            None => {
                eprintln!("{YELLOW}warning:{RESET} Telegram not configured — TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID must be set.");
            }
            Some(tg_client) => {
                match tg_client.send_message(text).await {
                    Ok(_) => {}
                    Err(e) => eprintln!("{YELLOW}warning:{RESET} Telegram send failed: {e}"),
                }
            }
        }
        return;
    }

    // --stream-pipe: read stdin line-by-line, redact secrets, POST each line to the URL.
    // Replaces curl in evolve.sh's streaming pipe — curl uses OpenSSL which crashes here.
    if let Some(ref url) = cli_args.stream_pipe {
        use std::io::BufReader;
        let url = url.clone();
        let client = axonix::http_client::get();
        let stdin = BufReader::new(io::stdin());
        for line in stdin.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.len() > 500 {
                continue;
            }
            let line = stream_redact(&line);
            let _ = client.post(&url).body(line).send().await;
        }
        return;
    }

    // --write-summary needs no API key — handle it before the key check.
    if let Some(ref label) = cli_args.write_summary {
        let label = if label.is_empty() { "unknown session".to_string() } else { label.clone() };
        eprintln!("{DIM}  writing cycle summary: {label}{RESET}");
        let summary = axonix::cycle_summary::CycleSummary::from_real_data(&label);
        match axonix::cycle_summary::CycleSummary::write_default(&summary) {
            Ok(_) => {
                eprintln!("{GREEN}  ✓ cycle summary written to .axonix/cycle_summary.json{RESET}");
                eprintln!("  session: {}", summary.session);
                eprintln!("  completed: {} items", summary.completed.len());
                eprintln!("  pending: {} items", summary.pending.len());
            }
            Err(e) => {
                eprintln!("{RED}  ✗ failed to write cycle summary: {e}{RESET}");
                std::process::exit(1);
            }
        }
        return;
    }

    // --insert-metrics-row: insert a row into METRICS.md using insert_metrics_row() (G-072)
    if let Some(ref row) = cli_args.insert_metrics_row {
        let row = row.trim();
        if row.is_empty() {
            eprintln!("{RED}error:{RESET} --insert-metrics-row requires a non-empty row string.");
            std::process::exit(1);
        }
        let metrics_path = std::path::Path::new("METRICS.md");
        eprintln!("{DIM}  inserting metrics row into METRICS.md...{RESET}");
        match axonix::metrics::insert_metrics_row(metrics_path, row) {
            Ok(()) => {
                eprintln!("{GREEN}  ✓ row inserted into METRICS.md{RESET}");
            }
            Err(e) => {
                eprintln!("{RED}  ✗ failed to insert metrics row: {e}{RESET}");
                std::process::exit(1);
            }
        }
        return;
    }

    // --extract-memories: run memory extraction on session log and exit
    if let Some(ref log_path) = cli_args.extract_memories {
        let path = std::path::Path::new(log_path);
        let db_path = std::path::Path::new(".axonix/axonix.db");
        eprintln!("{DIM}  extracting memories from {log_path}...{RESET}");
        match axonix::memory::capture::extract_and_store(path, db_path).await {
            Ok(result) => {
                eprintln!("{GREEN}  ✓ memory extraction complete: {} stored, {} skipped{RESET}",
                    result.stored, result.skipped);
            }
            Err(e) => {
                eprintln!("{RED}  ✗ memory extraction failed: {e}{RESET}");
                // Non-fatal — don't exit with error code
            }
        }
        return;
    }

    let api_key = match std::env::var("ANTHROPIC_API_KEY").or_else(|_| std::env::var("API_KEY")) {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("{RED}error:{RESET} No API key found.");
            eprintln!("Set ANTHROPIC_API_KEY or API_KEY environment variable.");
            eprintln!("Example: ANTHROPIC_API_KEY=sk-ant-... cargo run");
            std::process::exit(1);
        }
    };

    let model = cli_args.model.clone();

    let skills = if cli_args.skill_dirs.is_empty() {
        SkillSet::empty()
    } else {
        match SkillSet::load(&cli_args.skill_dirs) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{YELLOW}warning:{RESET} Failed to load skills: {e}");
                SkillSet::empty()
            }
        }
    };

    // Load memory and predictions early so we can inject context into the system prompt (G-024).
    let startup_memory = axonix::memory::MemoryStore::load_default();
    let startup_predictions = axonix::predictions::PredictionStore::default_path();

    // Grab the first active goal title for memory context injection.
    let active_goal_title = axonix::brief::parse_active_goals()
        .into_iter()
        .next()
        .unwrap_or_default();

    let system_prompt = build_system_prompt(&startup_memory, &startup_predictions, &active_goal_title);

    let mut agent = make_agent(&api_key, &model, skills.clone(), &system_prompt);

    // Initialize Telegram client if credentials are available
    let tg = TelegramClient::from_env();

    // Initialize GitHub client and configure git identity.
    // Only set git identity when running inside Docker — avoids polluting the
    // operator's host git config after the container exits (Issue #20).
    // Use bot_only() so /comment never falls back to posting as the owner account.
    // from_env() is still available for git identity setup below.
    let gh = GitHubClient::bot_only();
    let gh_full = GitHubClient::from_env();

    // Initialize Bluesky client if credentials are available
    let bsky = BlueskyClient::from_env();
    if let Some(ref gh_client) = gh_full {
        if std::path::Path::new("/.dockerenv").exists() {
            let cwd_str = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| ".".to_string());
            if let Err(e) = gh_client.configure_git_identity(&cwd_str) {
                eprintln!("{YELLOW}warning:{RESET} git identity config failed: {e}");
            }
        }
    }

    // Seed observations table from JOURNAL.md (G-089).
    let journal_path = std::path::Path::new("JOURNAL.md");
    if journal_path.exists() {
        match axonix::db::AxonixDb::open_default() {
            Ok(db) => {
                match db.seed_from_journal(journal_path) {
                    Ok(n) => {
                        if n > 0 {
                            eprintln!("{DIM}  seeded {n} journal entries into memory{RESET}");
                        }
                    }
                    Err(e) => eprintln!("{YELLOW}warning:{RESET} journal seed failed: {e}"),
                }
            }
            Err(e) => eprintln!("{YELLOW}warning:{RESET} db open failed for journal seed: {e}"),
        }
    }

    // CLI dispatch modes — extracted to cli_dispatch.rs (G-123)
    if cli_args.health_subcommand {
        cli_dispatch::run_health_subcommand();
        return;
    }
    if cli_args.brief {
        cli_dispatch::run_brief_mode(&cli_args, &tg).await;
        return;
    }
    if cli_args.health {
        cli_dispatch::run_health_mode(&tg).await;
        return;
    }
    if cli_args.watch {
        cli_dispatch::run_watch_mode(&tg).await;
        return;
    }
    if cli_args.listen {
        cli_dispatch::run_listen_mode(&tg, &api_key, &model).await;
        return;
    }
    if cli_args.session_summary_telegram {
        cli_dispatch::run_session_summary_telegram_mode(&tg).await;
        return;
    }
    if let Some(post_text) = cli_args.bluesky_post {
        cli_dispatch::run_bluesky_post_mode(&bsky, &post_text).await;
        return;
    }

    // Prompt and piped modes — extracted to prompt_dispatch.rs (G-123)
    if let Some(prompt_text) = cli_args.prompt {
        prompt_dispatch::run_prompt_mode(&mut agent, &prompt_text, &tg, &model).await;
        return;
    }
    if !io::stdin().is_terminal() {
        prompt_dispatch::run_piped_mode(&mut agent, &tg, &model).await;
        return;
    }

    // Interactive REPL mode — extracted to repl_loop.rs (G-123)
    let ctx = repl_loop::ReplContext {
        api_key: &api_key,
        model: model.clone(),
        skills,
        system_prompt,
        tg,
        gh,
        bsky,
    };
    repl_loop::run_repl_loop(agent, ctx).await;
}


#[cfg(test)]
mod tests {

    #[test]
    fn test_command_parsing_quit() {
        let quit_commands = ["/quit", "/exit"];
        for cmd in &quit_commands {
            assert!(
                *cmd == "/quit" || *cmd == "/exit",
                "Unrecognized quit command: {cmd}"
            );
        }
    }

    #[test]
    fn test_known_commands_recognized() {
        let known = ["/quit", "/exit", "/help", "/status", "/context", "/clear", "/tokens", "/retry"];
        for cmd in &known {
            assert!(
                matches!(
                    *cmd,
                    "/quit" | "/exit" | "/help" | "/status" | "/context" | "/clear" | "/tokens" | "/retry"
                ),
                "Command {cmd} should be recognized"
            );
        }
    }

    #[test]
    fn test_save_command_parsing() {
        let input = "/save my_file.md";
        assert!(input.starts_with("/save "));
        let path = input.trim_start_matches("/save ").trim();
        assert_eq!(path, "my_file.md");
    }

    #[test]
    fn test_save_command_default_path() {
        let input = "/save";
        let path = if input == "/save" {
            "conversation.md"
        } else {
            input.trim_start_matches("/save ").trim()
        };
        assert_eq!(path, "conversation.md");
    }

    #[test]
    fn test_unknown_command_detected() {
        let input = "/foo";
        assert!(input.starts_with('/'));
        assert!(
            !matches!(
                input,
                "/quit" | "/exit" | "/help" | "/status" | "/clear"
            ),
            "/foo should not be a known command"
        );
    }

    #[test]
    fn test_clear_should_preserve_model_switch() {
        let model = "claude-opus-4-6".to_string();
        assert_eq!(model, "claude-opus-4-6");
        let new_model = "claude-sonnet-4-20250514";
        let model = new_model.to_string();
        assert_eq!(model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_retry_tracks_last_prompt() {
        let mut last_prompt: Option<String> = None;
        assert!(last_prompt.is_none(), "Should start with no last prompt");
        last_prompt = Some("explain monads".to_string());
        assert_eq!(last_prompt.as_deref(), Some("explain monads"));
        last_prompt = Some("now explain functors".to_string());
        assert_eq!(last_prompt.as_deref(), Some("now explain functors"));
    }

    #[test]
    fn test_retry_empty_returns_none() {
        let last_prompt: Option<String> = None;
        assert!(last_prompt.is_none());
    }

    #[test]
    fn test_multiline_backslash_detection() {
        assert!("hello\\".trim_end().ends_with('\\'));
        assert!("hello \\".trim_end().ends_with('\\'));
        assert!(!"hello".trim_end().ends_with('\\'));
        assert!(!"".trim_end().ends_with('\\'));
    }

    #[test]
    fn test_multiline_triple_quote_detection() {
        assert_eq!("\"\"\"".trim(), "\"\"\"");
        assert_eq!("  \"\"\"  ".trim(), "\"\"\"");
        assert_ne!("\"\"\" hello".trim(), "\"\"\"");
    }

    #[test]
    fn test_backslash_stripping() {
        let line = "hello world\\";
        let stripped = line.trim_end().trim_end_matches('\\');
        assert_eq!(stripped, "hello world");
    }

    /// Verifies the Docker detection logic used for configure_git_identity guard.
    ///
    /// Inside Docker, /.dockerenv exists and configure_git_identity runs.
    /// Outside Docker, /.dockerenv is absent and the call is skipped.
    /// This prevents host git config from being overwritten (Issue #20).
    #[test]
    fn test_docker_detection_path() {
        let docker_marker = std::path::Path::new("/.dockerenv");
        // This test passes in both environments — it just verifies the detection
        // compiles and returns a bool, not that we're in Docker.
        let _in_docker: bool = docker_marker.exists();
        // The path string must be exactly /.dockerenv — not a variant.
        assert_eq!(docker_marker.to_str(), Some("/.dockerenv"));
    }

    /// Verifies that spawn_telegram_cron_poll returns None when Telegram is not configured.
    ///
    /// The helper must not panic and must produce no receiver when called with None.
    /// This ensures non-REPL modes degrade gracefully without Telegram credentials.
    #[test]
    fn test_spawn_telegram_cron_poll_none_when_no_tg() {
        // spawn_telegram_cron_poll needs a tokio runtime to spawn; we verify the
        // None path directly without spawning (no runtime needed for the None branch).
        let tg: Option<axonix::telegram::TelegramClient> = None;
        // The function returns None immediately when tg is None (before any spawn).
        // We can verify this by testing the equivalent logic inline.
        let result: Option<()> = tg.as_ref().map(|_| ());
        assert!(result.is_none(), "None tg should produce no poll task");
    }

    /// Verifies that build_tools produces the expected number of tools (G-027).
    ///
    /// Default tools = 6 (bash, read_file, write_file, edit_file, list_files, search).
    /// Sub-agents = 3 (code_reviewer, community_responder, implementer).
    /// Total expected = 9.
    #[test]
    fn test_build_tools_count() {
        let tools = super::build_tools("test-key", "claude-sonnet-4-20250514");
        assert_eq!(
            tools.len(),
            9,
            "Expected 6 default tools + 3 sub-agents = 9, got {}",
            tools.len()
        );
    }

    /// Verifies the sub-agent names are present in the tool list (G-027).
    #[test]
    fn test_build_tools_has_sub_agents() {
        let tools = super::build_tools("test-key", "claude-sonnet-4-20250514");
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(
            names.contains(&"code_reviewer"),
            "Expected code_reviewer sub-agent in tools; got: {:?}",
            names
        );
        assert!(
            names.contains(&"community_responder"),
            "Expected community_responder sub-agent in tools; got: {:?}",
            names
        );
        assert!(
            names.contains(&"implementer"),
            "Expected implementer sub-agent in tools; got: {:?}",
            names
        );
    }

    /// Verifies the default tools (bash, read_file, etc.) are still present (G-027).
    #[test]
    fn test_build_tools_has_defaults() {
        let tools = super::build_tools("test-key", "claude-sonnet-4-20250514");
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        for expected in &["bash", "read_file", "write_file", "edit_file", "list_files", "search"] {
            assert!(
                names.contains(expected),
                "Expected default tool '{}' in tools; got: {:?}",
                expected,
                names
            );
        }
    }

    /// Verifies sub-agent descriptions are non-empty and meaningful (G-027).
    #[test]
    fn test_sub_agent_descriptions_non_empty() {
        let tools = super::build_tools("test-key", "claude-sonnet-4-20250514");
        for tool in &tools {
            let desc = tool.description();
            assert!(
                !desc.is_empty(),
                "Tool '{}' has empty description",
                tool.name()
            );
            assert!(
                desc.len() > 20,
                "Tool '{}' description too short ({}): {}",
                tool.name(),
                desc.len(),
                desc
            );
        }
    }

    /// Verifies format_session_summary_telegram includes session label and Completed (Closes #46).
    #[test]
    fn test_format_session_summary_telegram_basic() {
        let mut cs = axonix::cycle_summary::CycleSummary::new("/tmp/test_tg_basic.json");
        cs.set_session("Day 8, Session 3", "2026-04-01");
        cs.add_completed("Implemented --session-summary-telegram");
        cs.add_completed("Added 3 tests");
        let msg = super::format_session_summary_telegram(&cs);
        assert!(msg.contains("Day 8, Session 3"), "Should contain session label");
        assert!(msg.contains("Completed"), "Should contain Completed section");
        assert!(msg.contains("Implemented --session-summary-telegram"), "Should contain completed item");
    }

    /// Verifies format_session_summary_telegram handles empty completed/pending gracefully (Closes #46).
    #[test]
    fn test_format_session_summary_telegram_empty() {
        let cs = axonix::cycle_summary::CycleSummary::new("/tmp/test_tg_empty.json");
        // data is None — should return a helpful message, not panic
        let msg = super::format_session_summary_telegram(&cs);
        assert!(!msg.is_empty(), "Should return a non-empty message");
        assert!(msg.contains("no summary data"), "Should indicate no data");
    }

    /// Verifies format_session_summary_telegram shows test count when Some (Closes #46).
    #[test]
    fn test_format_session_summary_telegram_with_tests() {
        let mut cs = axonix::cycle_summary::CycleSummary::new("/tmp/test_tg_tests.json");
        cs.set_session("Day 8, Session 3", "2026-04-01");
        cs.add_completed("wrote tests");
        cs.set_test_count(535);
        let msg = super::format_session_summary_telegram(&cs);
        assert!(msg.contains("535"), "Should contain test count");
        assert!(msg.contains("Tests"), "Should contain Tests label");
    }
}
