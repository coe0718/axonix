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
mod startup;
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

    // Initialize Telegram client if credentials are available
    let tg = TelegramClient::from_env();

    // Initialize GitHub client and configure git identity.
    // Use bot_only() so /comment never falls back to posting as the owner account.
    let gh = GitHubClient::bot_only();
    let gh_full = GitHubClient::from_env();

    // Initialize Bluesky client if credentials are available
    let bsky = BlueskyClient::from_env();

    // Build agent, system prompt, configure git, and seed journal DB (G-166).
    let session = startup::initialize_session(&api_key, &model, skills.clone(), gh_full.as_ref());
    let mut agent = session.agent;
    let system_prompt = session.system_prompt;

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
#[path = "main_tests.rs"]
mod tests;
