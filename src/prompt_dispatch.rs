//! Non-interactive prompt dispatchers: --prompt mode and piped mode.
//!
//! Extracted from main.rs (G-123) to keep the binary entry point focused.
//! Both modes run a single prompt and exit.

use std::io::{self, Read};

use axonix::render::*;
use axonix::repl::{handle_command, CommandResult, ReplState};
use crate::prompt_runner::run_prompt;
use crate::telegram_poll::spawn_telegram_cron_poll;

/// Handle a slash command in non-interactive (prompt or piped) mode.
///
/// Returns true if the command was handled and the caller should return.
/// Returns false if the input is NotACommand (should fall through to AI).
pub fn dispatch_slash_command_noninteractive(input: &str, repl: &mut ReplState) -> bool {
    let cmd_result = handle_command(input, repl, &[]);
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
            true
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
            true
        }
        CommandResult::Handled(ref lines) => {
            for line in lines {
                println!("{line}");
            }
            true
        }
        CommandResult::NotACommand => false,
        _ => true, // Quit, Clear, SwitchModel, Retry, FetchIssues — return silently
    }
}

/// Handle --prompt / -p mode: run a single CLI prompt and exit.
pub async fn run_prompt_mode(
    agent: &mut yoagent::Agent,
    prompt_text: &str,
    tg: &Option<axonix::telegram::TelegramClient>,
    model: &str,
) {
    let prompt_text = prompt_text.trim();
    if prompt_text.is_empty() {
        eprintln!("{RED}error:{RESET} --prompt requires a non-empty string.");
        eprintln!("Example: axonix -p \"explain this code\"");
        std::process::exit(1);
    }
    eprintln!("{DIM}  axonix (prompt mode) — model: {model}{RESET}");
    let mut repl = ReplState::new(model);

    // Intercept slash-commands so they are dispatched locally instead of
    // being forwarded to the AI as natural-language prompts (Issue #102).
    if prompt_text.starts_with('/') {
        if dispatch_slash_command_noninteractive(prompt_text, &mut repl) {
            return;
        }
    }

    // Spawn Telegram poll during --prompt mode so /status, /help, /ask
    // are handled even during cron sessions (Issue #21 / G-015).
    let tg_prompt_rx = spawn_telegram_cron_poll(tg, model);

    run_prompt(agent, prompt_text, &mut repl, tg.as_ref()).await;

    // After main prompt: process any queued /ask commands from Telegram
    if let Some(mut rx) = tg_prompt_rx {
        while let Ok(ask_cmd) = rx.try_recv() {
            let ask_prompt = ask_cmd.prompt.clone();
            let msg_id = ask_cmd.message_id;
            eprintln!("{DIM}  📱 Telegram ask (queued): {}{RESET}", truncate(&ask_prompt, 60));
            repl.push_prompt(&ask_prompt);
            run_prompt(agent, &ask_prompt, &mut repl, tg.as_ref()).await;
            if let Some(ref tg_client) = tg {
                tg_client.reply_to("✅ Done", msg_id).await.ok();
            }
        }
    }
}

/// Handle piped mode: stdin is not a terminal; read all stdin as one prompt and exit.
pub async fn run_piped_mode(
    agent: &mut yoagent::Agent,
    tg: &Option<axonix::telegram::TelegramClient>,
    model: &str,
) {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).ok();
    let input_trimmed = input.trim();
    if input_trimmed.is_empty() {
        eprintln!("No input on stdin.");
        std::process::exit(1);
    }

    eprintln!("{DIM}  axonix (piped mode) — model: {model}{RESET}");
    let mut repl = ReplState::new(model);

    // Intercept slash-commands in piped mode too (Issue #102).
    if input_trimmed.starts_with('/') {
        if dispatch_slash_command_noninteractive(input_trimmed, &mut repl) {
            return;
        }
    }

    // Spawn Telegram poll during piped mode too (same fix as --prompt mode)
    let tg_piped_rx = spawn_telegram_cron_poll(tg, model);

    run_prompt(agent, input_trimmed, &mut repl, tg.as_ref()).await;

    // Process any queued /ask commands from Telegram
    if let Some(mut rx) = tg_piped_rx {
        while let Ok(ask_cmd) = rx.try_recv() {
            let ask_prompt = ask_cmd.prompt.clone();
            let msg_id = ask_cmd.message_id;
            eprintln!("{DIM}  📱 Telegram ask (queued): {}{RESET}", truncate(&ask_prompt, 60));
            repl.push_prompt(&ask_prompt);
            run_prompt(agent, &ask_prompt, &mut repl, tg.as_ref()).await;
            if let Some(ref tg_client) = tg {
                tg_client.reply_to("✅ Done", msg_id).await.ok();
            }
        }
    }
}
