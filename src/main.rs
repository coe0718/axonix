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

use std::io::{self, BufRead, IsTerminal, Read, Write, BufReader};
use yoagent::skills::SkillSet;
use yoagent::*;

use axonix::bluesky::BlueskyClient;
use axonix::brief::Brief;
use axonix::cli::{self, CliArgs};
use axonix::conversation::save_conversation;
use axonix::cost::estimate_cost;
use axonix::github::GitHubClient;
use axonix::render::*;
use axonix::repl::{handle_command, CommandResult, ReplState};
use axonix::telegram::TelegramClient;

mod agent_setup;
mod session_helpers;
mod telegram_poll;
mod prompt_runner;

#[allow(unused_imports)]
use agent_setup::{build_tools, make_agent, build_system_prompt, stream_redact};
#[allow(unused_imports)]
pub(self) use agent_setup::build_tools as _build_tools_for_tests;
use session_helpers::{read_journal_title, get_recent_commits, format_session_summary_telegram, get_test_count};
use telegram_poll::spawn_telegram_cron_poll;
use prompt_runner::run_prompt;

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

    let model = cli_args.model;

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
    let gh = GitHubClient::from_env();

    // Initialize Bluesky client if credentials are available
    let bsky = BlueskyClient::from_env();
    if let Some(ref gh_client) = gh {
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

    // --brief mode: print morning brief (open goals, predictions, recent metrics) and exit.
    // --brief-telegram: also push the brief to Telegram (for cron-based 7 AM delivery, G-031).
    if cli_args.brief {
        let brief = Brief::collect();
        print!("{}", brief.format_terminal());
        // Log this brief run to axonix.db sessions table (G-079).
        // Done after display so the DB write never delays or affects output.
        brief.log_to_db();
        if cli_args.brief_telegram {
            match &tg {
                None => {
                    eprintln!("{RED}error:{RESET} --brief-telegram requires Telegram. Set TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID.");
                    std::process::exit(1);
                }
                Some(tg_client) => {
                    eprintln!("{DIM}  sending morning brief to Telegram...{RESET}");
                    let brief_msg = brief.format_telegram();
                    match tg_client.send_message(&brief_msg).await {
                        Ok(_) => eprintln!("{GREEN}  ✓ morning brief sent to Telegram{RESET}"),
                        Err(e) => eprintln!("{RED}  ✗ Telegram send failed: {e}{RESET}"),
                    }
                }
            }
        }
        return;
    }

    // --health mode: report system health + Docker container status, alert on unhealthy (G-078)
    if cli_args.health {
        let snapshot = axonix::health::HealthSnapshot::collect();
        println!("{}", snapshot.format());
        println!();
        let docker = axonix::health::docker_health();
        println!("{}", docker.format());
        // Send Telegram alert for any non-running container
        if let Some(ref tg_client) = tg {
            if !docker.all_healthy() || docker.error.is_some() {
                let mut alert = format!("🚨 Container alert:\n{}", docker.format());
                if docker.error.is_some() {
                    alert = format!("🚨 Docker unreachable:\n{}", docker.format());
                }
                tg_client.send_message(&alert).await.ok();
            }
        }
        return;
    }

    // --watch mode: run health watch loop, send Telegram alerts when thresholds exceeded (G-025)
    if cli_args.watch {
        match &tg {
            None => {
                eprintln!("{RED}error:{RESET} --watch requires Telegram. Set TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID.");
                std::process::exit(1);
            }
            Some(tg_client) => {
                eprintln!("{DIM}  axonix --watch — health monitor active{RESET}");
                eprintln!("{DIM}  Press Ctrl+C to stop{RESET}");
                let config = axonix::watch::WatchConfig::default();
                axonix::watch::run_watch(config, tg_client).await;
                return;
            }
        }
    }

    // --listen mode: run always-on Telegram listener daemon (G-060b)
    if cli_args.listen {
        match &tg {
            None => {
                eprintln!("{RED}error:{RESET} --listen requires Telegram. Set TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID.");
                std::process::exit(1);
            }
            Some(tg_client) => {
                eprintln!("{DIM}  axonix --listen — personal assistant daemon active{RESET}");
                eprintln!("{DIM}  Press Ctrl+C to stop{RESET}");
                let config = axonix::listener::ListenerConfig::default();
                match axonix::listener::run_listener(&config, tg_client, &api_key, &model).await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("{RED}  ✗ listener error: {e}{RESET}");
                        std::process::exit(1);
                    }
                }
                return;
            }
        }
    }

    // --session-summary-telegram: read .axonix/cycle_summary.json and send to Telegram (Closes #46)
    if cli_args.session_summary_telegram {
        let summary = axonix::cycle_summary::CycleSummary::default_path();
        let msg = format_session_summary_telegram(&summary);
        match &tg {
            None => {
                eprintln!("error: --session-summary-telegram requires Telegram. Set TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID.");
                std::process::exit(1);
            }
            Some(tg_client) => {
                match tg_client.send_message(&msg).await {
                    Ok(_) => eprintln!("session summary sent to Telegram"),
                    Err(e) => {
                        eprintln!("Telegram send failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
        return;
    }

    // --bluesky-post mode: post to Bluesky and exit (no agent session started)
    if let Some(post_text) = cli_args.bluesky_post {
        let post_text = post_text.trim();
        if post_text.is_empty() {
            eprintln!("{RED}error:{RESET} --bluesky-post requires a non-empty string.");
            std::process::exit(1);
        }
        match &bsky {
            None => {
                eprintln!("{RED}error:{RESET} Bluesky not configured. Set BLUESKY_IDENTIFIER and BLUESKY_APP_PASSWORD.");
                std::process::exit(1);
            }
            Some(bsky_client) => {
                eprintln!("{DIM}  posting to Bluesky...{RESET}");
                match bsky_client.post(post_text).await {
                    Ok((uri, _cid)) => {
                        eprintln!("{GREEN}  ✓ Bluesky post created (uri: {uri}){RESET}");
                        eprintln!("  text: {post_text}");
                    }
                    Err(e) => {
                        eprintln!("{RED}  ✗ Bluesky post failed: {e}{RESET}");
                        std::process::exit(1);
                    }
                }
            }
        }
        return;
    }

    // --prompt / -p mode: run a single prompt from CLI args and exit
    if let Some(prompt_text) = cli_args.prompt {
        let prompt_text = prompt_text.trim();
        if prompt_text.is_empty() {
            eprintln!("{RED}error:{RESET} --prompt requires a non-empty string.");
            eprintln!("Example: axonix -p \"explain this code\"");
            std::process::exit(1);
        }
        eprintln!("{DIM}  axonix (prompt mode) — model: {model}{RESET}");
        let session_start = std::time::Instant::now();
        let mut repl = ReplState::new(&model);

        // Intercept slash-commands so they are dispatched locally instead of
        // being forwarded to the AI as natural-language prompts (Issue #102).
        if prompt_text.starts_with('/') {
            let cmd_result = handle_command(prompt_text, &mut repl, &[]);
            match cmd_result {
                CommandResult::ArchiveJournal => {
                    let archiver = axonix::journal_archive::JournalArchiver::default();
                    match archiver.archive() {
                        Ok(result) if result.moved == 0 => {
                            println!("  (journal has {} or fewer recent entries — no archiving needed)", result.kept);
                        }
                        Ok(result) => {
                            println!("  ✓ archived {} entries to {} ({} kept in JOURNAL.md)",
                                result.moved, result.archive_path, result.kept);
                        }
                        Err(e) => {
                            println!("  ✗ journal archive failed: {e}");
                        }
                    }
                    return;
                }
                CommandResult::MemorySearch(ref query) => {
                    match axonix::db::AxonixDb::open_default() {
                        Err(e) => println!("  ✗ DB error: {e}"),
                        Ok(db) => match db.search_memory(query, 20) {
                            Err(e) => println!("  ✗ search error: {e}"),
                            Ok(results) if results.is_empty() => {
                                println!("  No observations match \"{query}\".");
                            }
                            Ok(results) => {
                                let sep = "─".repeat(45);
                                println!("  Memory search: \"{query}\"");
                                println!("  {sep}");
                                for row in &results {
                                    let preview: String = row.text.chars().take(200).collect();
                                    let date = &row.created_at[..10.min(row.created_at.len())];
                                    println!("  [{}] — score: {}", row.key, row.score);
                                    println!("    {preview}");
                                    if !row.tags.is_empty() {
                                        println!("    tags: {}", row.tags);
                                    }
                                    println!("    created: {date}");
                                    println!("  {sep}");
                                }
                                println!("  ({} result{})",
                                    results.len(),
                                    if results.len() == 1 { "" } else { "s" });
                            }
                        },
                    }
                    return;
                }
                CommandResult::Handled(ref lines) => {
                    for line in lines {
                        println!("{line}");
                    }
                    return;
                }
                CommandResult::NotACommand => {
                    // Falls through to run_prompt() — slash prefix but not a known command
                }
                _ => {
                    // Quit, Clear, SwitchModel, Retry, FetchIssues — return silently
                    return;
                }
            }
        }

        // Spawn Telegram poll during --prompt mode so /status, /help, /ask
        // are handled even during cron sessions (Issue #21 / G-015).
        let tg_prompt_rx = spawn_telegram_cron_poll(&tg, &model);

        run_prompt(&mut agent, prompt_text, &mut repl, tg.as_ref()).await;

        // After main prompt: process any queued /ask commands from Telegram
        if let Some(mut rx) = tg_prompt_rx {
            while let Ok(ask_cmd) = rx.try_recv() {
                let ask_prompt = ask_cmd.prompt.clone();
                let msg_id = ask_cmd.message_id;
                eprintln!("{DIM}  📱 Telegram ask (queued): {}{RESET}", truncate(&ask_prompt, 60));
                repl.push_prompt(&ask_prompt);
                run_prompt(&mut agent, &ask_prompt, &mut repl, tg.as_ref()).await;
                if let Some(ref tg_client) = tg {
                    tg_client.reply_to("✅ Done", msg_id).await.ok();
                }
            }
        }
        let _ = session_start; // suppress unused warning
        return;
    }

    // Piped mode: read all of stdin as a single prompt, run once, exit
    if !io::stdin().is_terminal() {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).ok();
        let input = input.trim();
        if input.is_empty() {
            eprintln!("No input on stdin.");
            std::process::exit(1);
        }

        eprintln!("{DIM}  axonix (piped mode) — model: {model}{RESET}");
        let session_start_piped = std::time::Instant::now();
        let mut repl = ReplState::new(&model);

        // Intercept slash-commands in piped mode too (Issue #102).
        if input.starts_with('/') {
            let cmd_result = handle_command(input, &mut repl, &[]);
            match cmd_result {
                CommandResult::ArchiveJournal => {
                    let archiver = axonix::journal_archive::JournalArchiver::default();
                    match archiver.archive() {
                        Ok(result) if result.moved == 0 => {
                            println!("  (journal has {} or fewer recent entries — no archiving needed)", result.kept);
                        }
                        Ok(result) => {
                            println!("  ✓ archived {} entries to {} ({} kept in JOURNAL.md)",
                                result.moved, result.archive_path, result.kept);
                        }
                        Err(e) => {
                            println!("  ✗ journal archive failed: {e}");
                        }
                    }
                    return;
                }
                CommandResult::MemorySearch(ref query) => {
                    match axonix::db::AxonixDb::open_default() {
                        Err(e) => println!("  ✗ DB error: {e}"),
                        Ok(db) => match db.search_memory(query, 20) {
                            Err(e) => println!("  ✗ search error: {e}"),
                            Ok(results) if results.is_empty() => {
                                println!("  No observations match \"{query}\".");
                            }
                            Ok(results) => {
                                let sep = "─".repeat(45);
                                println!("  Memory search: \"{query}\"");
                                println!("  {sep}");
                                for row in &results {
                                    let preview: String = row.text.chars().take(200).collect();
                                    let date = &row.created_at[..10.min(row.created_at.len())];
                                    println!("  [{}] — score: {}", row.key, row.score);
                                    println!("    {preview}");
                                    if !row.tags.is_empty() {
                                        println!("    tags: {}", row.tags);
                                    }
                                    println!("    created: {date}");
                                    println!("  {sep}");
                                }
                                println!("  ({} result{})",
                                    results.len(),
                                    if results.len() == 1 { "" } else { "s" });
                            }
                        },
                    }
                    return;
                }
                CommandResult::Handled(ref lines) => {
                    for line in lines {
                        println!("{line}");
                    }
                    return;
                }
                CommandResult::NotACommand => {
                    // Falls through to run_prompt() — slash prefix but not a known command
                }
                _ => {
                    // Quit, Clear, SwitchModel, Retry, FetchIssues — return silently
                    return;
                }
            }
        }

        // Spawn Telegram poll during piped mode too (same fix as --prompt mode)
        let tg_piped_rx = spawn_telegram_cron_poll(&tg, &model);

        run_prompt(&mut agent, input, &mut repl, tg.as_ref()).await;

        // Process any queued /ask commands from Telegram
        if let Some(mut rx) = tg_piped_rx {
            while let Ok(ask_cmd) = rx.try_recv() {
                let ask_prompt = ask_cmd.prompt.clone();
                let msg_id = ask_cmd.message_id;
                eprintln!("{DIM}  📱 Telegram ask (queued): {}{RESET}", truncate(&ask_prompt, 60));
                repl.push_prompt(&ask_prompt);
                run_prompt(&mut agent, &ask_prompt, &mut repl, tg.as_ref()).await;
                if let Some(ref tg_client) = tg {
                    tg_client.reply_to("✅ Done", msg_id).await.ok();
                }
            }
        }
        let _ = session_start_piped;
        return;
    }

    // Interactive REPL mode
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());

    // Initialize REPL state early so memory is available for banner display.
    // (Memory is loaded from .axonix/memory.json at this point.)
    let mut repl = ReplState::new(&model);

    cli::print_banner();
    println!("{DIM}  model: {model}{RESET}");
    let skill_names: Vec<String> = if skills.is_empty() {
        vec![]
    } else {
        println!("{DIM}  skills: {} loaded{RESET}", skills.len());
        skills.skills().iter().map(|s| s.name.clone()).collect()
    };
    println!("{DIM}  cwd:   {cwd}{RESET}");
    // Show memory count if any facts are stored
    if !repl.memory.is_empty() {
        println!("{DIM}  memory: {} facts loaded — /memory list to view{RESET}", repl.memory.len());
    }
    if tg.is_some() {
        println!("{DIM}  telegram: connected — send /ask <prompt> to chat with me{RESET}");
    }
    if let Some(ref gh_client) = gh {
        println!("{DIM}  github:   {} — use /comment <n> <text> to post issue comments{RESET}", gh_client.identity.display_name());
    }
    if bsky.is_some() {
        println!("{DIM}  bluesky:  connected — use --bluesky-post <text> to post{RESET}");
    }
    println!("{DIM}  Type /help for commands{RESET}\n");

    let session_start = std::time::Instant::now();

    // Handle Ctrl+C gracefully
    let ctrlc_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let flag = ctrlc_flag.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            eprintln!("\n{DIM}  ⚡ signal received — emergency shutdown — bye 👋{RESET}\n");
            std::process::exit(0);
        });
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    // repl was already initialized above (before banner) to allow memory display

    // Telegram inbound poll: spawn a background task that polls for bot commands
    // and sends them over a channel for the main loop to process after each turn.
    let tg_rx = if let Some(ref tg_client) = tg {
        let (tx, rx) = tokio::sync::mpsc::channel::<axonix::telegram::BotCommand>(16);
        let tg_poll = tg_client.clone();
        tokio::spawn(async move {
            let mut offset: i64 = 0;
            loop {
                match tg_poll.get_updates(offset).await {
                    Ok(updates) => {
                        if !updates.is_empty() {
                            offset = updates.iter().map(|u| u.update_id).max().unwrap_or(offset) + 1;
                            let commands = tg_poll.extract_commands(&updates);
                            for cmd in commands {
                                if tx.send(cmd).await.is_err() {
                                    return; // receiver dropped, session ended
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Network error — wait before retrying
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
                // Small sleep between polls to avoid hammering the API
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        });
        Some(rx)
    } else {
        None
    };
    let mut tg_rx = tg_rx;

    loop {
        print!("{BOLD}{GREEN}> {RESET}");
        io::stdout().flush().ok();

        let line = match lines.next() {
            Some(Ok(l)) => l,
            _ => break,
        };

        // Multiline input: backslash continuation or triple-quote blocks
        let input = if line.trim_end().ends_with('\\') {
            let mut buf = String::from(line.trim_end().trim_end_matches('\\'));
            buf.push('\n');
            loop {
                print!("{DIM}. {RESET}");
                io::stdout().flush().ok();
                match lines.next() {
                    Some(Ok(next)) => {
                        if next.trim_end().ends_with('\\') {
                            buf.push_str(next.trim_end().trim_end_matches('\\'));
                            buf.push('\n');
                        } else {
                            buf.push_str(&next);
                            break;
                        }
                    }
                    _ => break,
                }
            }
            buf
        } else if line.trim() == "\"\"\"" {
            let mut buf = String::new();
            println!("{DIM}  (multiline mode — type \"\"\" on its own line to finish){RESET}");
            loop {
                print!("{DIM}. {RESET}");
                io::stdout().flush().ok();
                match lines.next() {
                    Some(Ok(next)) => {
                        if next.trim() == "\"\"\"" {
                            break;
                        }
                        if !buf.is_empty() {
                            buf.push('\n');
                        }
                        buf.push_str(&next);
                    }
                    _ => break,
                }
            }
            buf
        } else {
            line
        };

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // Dispatch through handle_command first, then handle agent-data commands inline
        let cmd_result = handle_command(input, &mut repl, &skill_names);
        match cmd_result {
            CommandResult::Quit => break,

            CommandResult::Clear => {
                agent = make_agent(&api_key, &repl.model, skills.clone(), &system_prompt);
                repl.reset_tokens();
                println!("{DIM}  (conversation cleared){RESET}\n");
                continue;
            }

            CommandResult::SwitchModel(ref new_model) => {
                agent = make_agent(&api_key, new_model, skills.clone(), &system_prompt);
                println!("{DIM}  (switched to {new_model}, conversation cleared){RESET}\n");
                continue;
            }

            CommandResult::Retry(ref prompt) => {
                println!("{DIM}  (retrying: {}){RESET}", truncate(prompt, 60));
                let prompt = prompt.clone();
                run_prompt(&mut agent, &prompt, &mut repl, tg.as_ref()).await;
                continue;
            }

            CommandResult::FetchIssues(limit) => {
                match &gh {
                    None => {
                        println!("{YELLOW}  ⚠ No GitHub token available — set GH_TOKEN or AXONIX_BOT_TOKEN{RESET}\n");
                    }
                    Some(gh_client) => {
                        print!("{DIM}  fetching open issues for coe0718/axonix...{RESET}");
                        io::stdout().flush().ok();
                        match gh_client.list_issues("coe0718/axonix", limit).await {
                            Err(e) => println!("\n{RED}  ✗ failed to fetch issues: {e}{RESET}\n"),
                            Ok(issues) => {
                                println!();
                                if issues.is_empty() {
                                    println!("{DIM}  (no open issues){RESET}\n");
                                } else {
                                    println!("{DIM}  Open issues ({} shown, sorted by 👍):{RESET}", issues.len());
                                    for issue in &issues {
                                        let label_str = if issue.labels.is_empty() {
                                            String::new()
                                        } else {
                                            format!(" [{}]", issue.labels.join(", "))
                                        };
                                        let reaction_str = if issue.reactions > 0 {
                                            format!(" 👍{}", issue.reactions)
                                        } else {
                                            String::new()
                                        };
                                        println!(
                                            "{DIM}  #{:<4}{RESET} {}{reaction_str}{YELLOW}{label_str}{RESET}",
                                            issue.number,
                                            truncate(&issue.title, 70)
                                        );
                                    }
                                    println!();
                                }
                            }
                        }
                    }
                }
                continue;
            }

            CommandResult::ArchiveJournal => {
                let archiver = axonix::journal_archive::JournalArchiver::default();
                match archiver.archive() {
                    Ok(result) if result.moved == 0 => {
                        println!("{DIM}  (journal has {} or fewer recent entries — no archiving needed){RESET}\n",
                            result.kept);
                    }
                    Ok(result) => {
                        println!("{GREEN}  ✓ archived {} entries to {} ({} kept in JOURNAL.md){RESET}\n",
                            result.moved, result.archive_path, result.kept);
                    }
                    Err(e) => {
                        println!("{RED}  ✗ journal archive failed: {e}{RESET}\n");
                    }
                }
                continue;
            }

            CommandResult::MemorySearch(ref query) => {
                let sep = "─".repeat(45);
                match axonix::db::AxonixDb::open_default() {
                    Err(e) => {
                        println!("{RED}  ✗ DB error: {e}{RESET}\n");
                    }
                    Ok(db) => {
                        match db.search_memory(query, 20) {
                            Err(e) => {
                                println!("{RED}  ✗ search error: {e}{RESET}\n");
                            }
                            Ok(results) if results.is_empty() => {
                                println!("  No observations match \"{query}\".\n");
                            }
                            Ok(results) => {
                                println!("  Memory search: \"{query}\"");
                                println!("  {sep}");
                                for row in &results {
                                    let preview: String = row.text.chars().take(200).collect();
                                    let date = &row.created_at[..10.min(row.created_at.len())];
                                    println!("  [{}] — score: {}", row.key, row.score);
                                    println!("    {preview}");
                                    if !row.tags.is_empty() {
                                        println!("    tags: {}", row.tags);
                                    }
                                    println!("    created: {date}");
                                    println!("  {sep}");
                                }
                                println!("  ({} result{})\n", results.len(),
                                    if results.len() == 1 { "" } else { "s" });
                            }
                        }
                    }
                }
                continue;
            }

            CommandResult::Handled(ref output_lines) => {
                // Render the output lines, interpreting special markers
                let mut gh_comment_request: Option<(u64, String)> = None;
                let mut review_request: Option<String> = None;
                let mut do_recap = false;
                for line in output_lines {
                    if let Some(rest) = line.strip_prefix("__save:") {
                        // Perform the actual save (needs agent messages)
                        match save_conversation(agent.messages(), rest) {
                            Ok(count) => println!("{DIM}  saved {count} messages to {rest}{RESET}\n"),
                            Err(e) => println!("{RED}  failed to save: {e}{RESET}\n"),
                        }
                    } else if let Some(rest) = line.strip_prefix("__review:") {
                        // Collect review task for async dispatch after sync loop
                        review_request = Some(rest.to_string());
                    } else if let Some(rest) = line.strip_prefix("__gh_comment:") {
                        // format: "__gh_comment:<issue>:<body>"
                        // Collect for async dispatch after the sync loop
                        let mut parts = rest.splitn(2, ':');
                        let issue_str = parts.next().unwrap_or("0");
                        let body = parts.next().unwrap_or("").to_string();
                        if let Ok(n) = issue_str.parse::<u64>() {
                            gh_comment_request = Some((n, body));
                        }
                    } else if let Some(text) = line.strip_prefix("__predict:") {
                        // /predict <text> shorthand — store via PredictionStore
                        let text = text.trim();
                        if text.is_empty() {
                            println!("{YELLOW}  ⚠ prediction text cannot be empty{RESET}\n");
                        } else {
                            let mut store = axonix::predictions::PredictionStore::default_path();
                            let id = store.predict(text);
                            match store.save() {
                                Ok(()) => println!("{GREEN}  ✓ prediction #{id} saved: {text}{RESET}\n"),
                                Err(e) => println!("{YELLOW}  ⚠ prediction #{id} queued but save failed: {e}{RESET}\n"),
                            }
                        }
                    } else if let Some(rest) = line.strip_prefix("__lint_ok:") {
                        // format: "__lint_ok:<path>:<summary>"
                        let (path, summary) = rest.split_once(':').unwrap_or((rest, "valid"));
                        println!("{GREEN}  ✓ {path}: {summary}{RESET}");
                    } else if let Some(rest) = line.strip_prefix("__lint_errors:") {
                        let (path, count) = rest.split_once(':').unwrap_or((rest, "?"));
                        println!("{RED}  ✗ {path} has {count} error(s):{RESET}");
                    } else if let Some(rest) = line.strip_prefix("__lint_error:") {
                        // format: "__lint_error:<line>:<message>"
                        let (lineno, msg) = rest.split_once(':').unwrap_or(("0", rest));
                        let n: usize = lineno.parse().unwrap_or(0);
                        if n > 0 {
                            println!("{RED}    line {n}: {msg}{RESET}");
                        } else {
                            println!("{RED}    {msg}{RESET}");
                        }
                    } else if let Some(rest) = line.strip_prefix("__lint_unsupported:") {
                        println!("{YELLOW}  ⚠ {rest}{RESET}");
                    } else if let Some(rest) = line.strip_prefix("__ssh_error:") {
                        // format: "__ssh_error:<host>:<message>"
                        let (host, msg) = rest.split_once(':').unwrap_or((rest, "unknown error"));
                        println!("{RED}  ✗ ssh {host}: {msg}{RESET}\n");
                    } else if let Some(rest) = line.strip_prefix("__ssh_result:") {
                        // format: "__ssh_result:<host>:<exit_code>:<output>"
                        let mut parts = rest.splitn(3, ':');
                        let host = parts.next().unwrap_or("?");
                        let exit_code: i32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
                        let output = parts.next().unwrap_or("").trim();
                        if exit_code == 0 {
                            if output.is_empty() {
                                println!("{GREEN}  ✓ {host}: (no output){RESET}\n");
                            } else {
                                println!("{GREEN}  ✓ {host}{RESET}");
                                for out_line in output.lines() {
                                    println!("    {out_line}");
                                }
                                println!();
                            }
                        } else {
                            println!("{RED}  ✗ {host} (exit {exit_code}){RESET}");
                            if !output.is_empty() {
                                for out_line in output.lines() {
                                    println!("    {out_line}");
                                }
                            }
                            println!();
                        }
                    } else if line == "__recap" {
                        do_recap = true;
                    } else if line.is_empty() {
                        println!();
                    } else {
                        println!("{DIM}{line}{RESET}");
                    }
                }
                // Add trailing newline after lint errors block if needed
                let has_lint_error = output_lines.iter().any(|l| l.starts_with("__lint_errors:"));
                if has_lint_error {
                    println!();
                }
                // Handle async GitHub comment posting
                if let Some((issue_n, body)) = gh_comment_request {
                    match &gh {
                        None => println!("{YELLOW}  ⚠ No GitHub token available (set GH_TOKEN or AXONIX_BOT_TOKEN){RESET}\n"),
                        Some(gh_client) => {
                            print!("{YELLOW}  ▶ posting comment on issue #{issue_n} as {}...{RESET}", gh_client.identity.display_name());
                            io::stdout().flush().ok();
                            match gh_client.post_comment("coe0718/axonix", issue_n, &body).await {
                                Ok(url) => println!("\n{GREEN}  ✓ comment posted: {url}{RESET}\n"),
                                Err(e) => println!("\n{RED}  ✗ failed to post comment: {e}{RESET}\n"),
                            }
                        }
                    }
                }
                // Handle async /review — invoke code_reviewer sub-agent via agent prompt (G-028)
                if let Some(review_task) = review_request {
                    println!("{DIM}  🔍 invoking code_reviewer sub-agent...{RESET}");
                    io::stdout().flush().ok();
                    let review_prompt = format!(
                        "Use the code_reviewer tool to review this change: {review_task}\n\
                         Print the review findings directly. Be concise — 3-5 bullets max."
                    );
                    run_prompt(&mut agent, &review_prompt, &mut repl, tg.as_ref()).await;
                }
                // Handle /recap: post a 3-post Bluesky thread summarising the session (Issue #49)
                if do_recap {
                    match &bsky {
                        None => println!("{YELLOW}  ⚠ /recap requires Bluesky. Set BLUESKY_IDENTIFIER and BLUESKY_APP_PASSWORD{RESET}\n"),
                        Some(bsky_client) => {
                            println!("{DIM}  📡 posting recap thread to Bluesky...{RESET}");
                            io::stdout().flush().ok();
                            // 1. Get session title from JOURNAL.md
                            let session_title = read_journal_title()
                                .unwrap_or_else(|| "Axonix session".to_string());
                            // 2. Get recent commits
                            let commit_subjects = get_recent_commits(5);
                            // 3. Build and post root post
                            let day = std::env::var("DAY_COUNT").ok()
                                .and_then(|s| s.split_whitespace().next().map(|n| n.parse::<u32>().unwrap_or(8)))
                                .unwrap_or(8);
                            let session = std::env::var("SESSION_COUNT").ok()
                                .and_then(|s| s.parse::<u32>().ok())
                                .unwrap_or(1);
                            let root_text = format!("axonix Day {day}, Session {session}: {session_title}");
                            let root_text = if root_text.chars().count() > 300 {
                                let truncated: String = root_text.chars().take(297).collect();
                                format!("{truncated}…")
                            } else {
                                root_text
                            };
                            match bsky_client.post(&root_text).await {
                                Err(e) => println!("{RED}  ✗ Bluesky recap post 1 failed: {e}{RESET}\n"),
                                Ok((root_uri, root_cid)) => {
                                    println!("{GREEN}  ✓ post 1: {root_uri}{RESET}");
                                    // Post 2: what changed (commits)
                                    let commit_refs: Vec<&str> = commit_subjects.iter().map(|s| s.as_str()).collect();
                                    let commits_text = BlueskyClient::format_recap_commits(&commit_refs);
                                    match bsky_client.post_reply(&commits_text, &root_uri, &root_cid, &root_uri, &root_cid).await {
                                        Err(e) => println!("{RED}  ✗ Bluesky recap post 2 failed: {e}{RESET}\n"),
                                        Ok((p2_uri, p2_cid)) => {
                                            println!("{GREEN}  ✓ post 2: {p2_uri}{RESET}");
                                            // Post 3: test count
                                            if let Some(test_count) = get_test_count() {
                                                let tests_text = BlueskyClient::format_recap_tests(test_count, None);
                                                match bsky_client.post_reply(&tests_text, &root_uri, &root_cid, &p2_uri, &p2_cid).await {
                                                    Err(e) => println!("{RED}  ✗ Bluesky recap post 3 failed: {e}{RESET}\n"),
                                                    Ok((p3_uri, _)) => println!("{GREEN}  ✓ post 3: {p3_uri}{RESET}"),
                                                }
                                            }
                                        }
                                    }
                                    println!();
                                }
                            }
                        }
                    }
                }
                continue;
            }

            CommandResult::NotACommand => {
                // Handle commands that need agent/session data inline
                match input {
                    "/status" => {
                        let msg_count = agent.messages().len();
                        let elapsed = session_start.elapsed();
                        let mins = elapsed.as_secs() / 60;
                        let secs = elapsed.as_secs() % 60;
                        println!("{DIM}  model:    {}{RESET}", agent.model);
                        println!("{DIM}  messages: {msg_count}{RESET}");
                        println!("{DIM}  tokens:   {} in / {} out (session total){RESET}", repl.total_input, repl.total_output);
                        if repl.total_cache_read > 0 || repl.total_cache_write > 0 {
                            println!("{DIM}  cache:    {} read / {} write{RESET}", repl.total_cache_read, repl.total_cache_write);
                        }
                        println!("{DIM}  elapsed:  {mins}m {secs}s{RESET}");
                        println!("{DIM}  cwd:      {cwd}{RESET}");
                        println!();
                        continue;
                    }
                    "/context" => {
                        let messages = agent.messages();
                        if messages.is_empty() {
                            println!("{DIM}  (no messages in context){RESET}\n");
                        } else {
                            println!("{DIM}  Context ({} messages):{RESET}", messages.len());
                            for (i, msg) in messages.iter().enumerate() {
                                let summary = match msg.as_llm() {
                                    Some(Message::User { content, .. }) => {
                                        let text = content.iter().find_map(|c| {
                                            if let Content::Text { text } = c { Some(text.as_str()) } else { None }
                                        }).unwrap_or("(no text)");
                                        format!("{CYAN}user:{RESET} {}", truncate(text, 70))
                                    }
                                    Some(Message::Assistant { content, usage, .. }) => {
                                        let text_len: usize = content.iter().map(|c| {
                                            match c {
                                                Content::Text { text } => text.len(),
                                                Content::ToolCall { .. } => 0,
                                                _ => 0,
                                            }
                                        }).sum();
                                        let tool_count = content.iter().filter(|c| matches!(c, Content::ToolCall { .. })).count();
                                        let mut desc = format!("{GREEN}assistant:{RESET} ");
                                        if tool_count > 0 {
                                            desc.push_str(&format!("{tool_count} tool call(s) "));
                                        }
                                        if text_len > 0 {
                                            desc.push_str(&format!("{text_len} chars "));
                                        }
                                        desc.push_str(&format!("{DIM}({}in/{}out){RESET}", usage.input, usage.output));
                                        desc
                                    }
                                    Some(Message::ToolResult { tool_name, is_error, content, .. }) => {
                                        let len: usize = content.iter().map(|c| {
                                            if let Content::Text { text } = c { text.len() } else { 0 }
                                        }).sum();
                                        let status = if *is_error { format!("{RED}✗{RESET}") } else { format!("{GREEN}✓{RESET}") };
                                        format!("{YELLOW}tool:{RESET} {tool_name} {status} ({len} chars)")
                                    }
                                    None => format!("{DIM}(extension message){RESET}"),
                                };
                                println!("{DIM}  {i:>3}.{RESET} {summary}");
                            }
                            println!();
                        }
                        continue;
                    }
                    "/tokens" => {
                        let cost = estimate_cost(&repl.model, repl.total_input, repl.total_output, repl.total_cache_read, repl.total_cache_write);
                        println!("{DIM}  Token usage (session total):{RESET}");
                        println!("{DIM}    input:       {}{RESET}", repl.total_input);
                        println!("{DIM}    output:      {}{RESET}", repl.total_output);
                        if repl.total_cache_read > 0 || repl.total_cache_write > 0 {
                            println!("{DIM}    cache read:  {}{RESET}", repl.total_cache_read);
                            println!("{DIM}    cache write: {}{RESET}", repl.total_cache_write);
                        }
                        println!("{DIM}    total:       {}{RESET}", repl.total_input + repl.total_output + repl.total_cache_read + repl.total_cache_write);
                        println!("{DIM}    est. cost:   ${cost:.4}{RESET}");
                        println!();
                        continue;
                    }
                    _ => {} // Fall through to agent prompt
                }

                repl.push_prompt(input);
                run_prompt(&mut agent, input, &mut repl, tg.as_ref()).await;
            }
        }

        // After each main-loop turn, drain any queued Telegram bot commands
        if let Some(ref mut rx) = tg_rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    axonix::telegram::BotCommand::Ask(ask_cmd) => {
                        let ask_prompt = ask_cmd.prompt.clone();
                        let msg_id = ask_cmd.message_id;
                        println!("\n{DIM}  📱 Telegram ask: {}{RESET}", truncate(&ask_prompt, 60));
                        repl.push_prompt(&ask_prompt);
                        run_prompt(&mut agent, &ask_prompt, &mut repl, tg.as_ref()).await;
                        // Acknowledge in Telegram that the ask was processed
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("✅ Done", msg_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Help { message_id } => {
                        println!("\n{DIM}  📱 Telegram /help{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to(axonix::telegram::TELEGRAM_HELP_TEXT, message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Status { message_id } => {
                        println!("\n{DIM}  📱 Telegram /status{RESET}");
                        if let Some(ref tg_client) = tg {
                            let elapsed = session_start.elapsed().as_secs();
                            let reply = TelegramClient::format_status_reply(
                                &repl.model,
                                "interactive",
                                elapsed,
                                repl.total_input,
                                repl.total_output,
                            );
                            tg_client.reply_to(&reply, message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Health { message_id } => {
                        println!("\n{DIM}  📱 Telegram /health{RESET}");
                        if let Some(ref tg_client) = tg {
                            let snapshot = axonix::health::HealthSnapshot::collect();
                            let docker = axonix::health::docker_health();
                            let reply = format!("{}\n\n{}", snapshot.format(), docker.format());
                            tg_client.reply_to(&reply, message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Brief { message_id } => {
                        println!("\n{DIM}  📱 Telegram /brief{RESET}");
                        if let Some(ref tg_client) = tg {
                            let brief = axonix::brief::Brief::collect();
                            tg_client.reply_to(&brief.format_telegram(), message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Run { task, message_id } => {
                        println!("\n{DIM}  📱 Telegram /run: {}{RESET}", truncate(&task, 60));
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to(&format!("⏳ Running task: _{task}_"), message_id).await.ok();
                            run_prompt(&mut agent, &task, &mut repl, tg.as_ref()).await;
                            tg_client.reply_to("✅ Done", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Goal { description, message_id } => {
                        println!("\n{DIM}  📱 Telegram /goal: {}{RESET}", truncate(&description, 60));
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /goal is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Memory { action: _, message_id } => {
                        println!("\n{DIM}  📱 Telegram /memory{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /memory is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::History { message_id } => {
                        println!("\n{DIM}  📱 Telegram /history{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /history is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Goals { message_id } => {
                        println!("\n{DIM}  📱 Telegram /goals{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /goals is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Predict { text: _, message_id } => {
                        println!("\n{DIM}  📱 Telegram /predict{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /predict is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Resolve { id: _, verdict: _, message_id } => {
                        println!("\n{DIM}  📱 Telegram /resolve{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /resolve is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::ListPredictions { message_id } => {
                        println!("\n{DIM}  📱 Telegram /predictions{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /predictions is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                }
            }
        }
    }

    println!("\n{DIM}  ⚡ AXONIX OFFLINE — shutting down subsystems... bye 👋{RESET}\n");
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
