//! CLI dispatch mode functions: one function per non-interactive CLI mode.
//!
//! Extracted from main.rs (G-123) to keep the binary entry point focused.
//! Each function handles one CLI flag/mode and returns after completing.

use axonix::bluesky::BlueskyClient;
use axonix::brief::Brief;
use axonix::cli::CliArgs;
use axonix::render::*;
use axonix::telegram::TelegramClient;
use crate::session_helpers::format_session_summary_telegram;

/// Handle --brief and --brief-telegram modes.
pub async fn run_brief_mode(cli_args: &CliArgs, tg: &Option<TelegramClient>) {
    let brief = Brief::collect();
    print!("{}", brief.format_terminal());
    // Log this brief run to axonix.db sessions table (G-079).
    // Done after display so the DB write never delays or affects output.
    brief.log_to_db();
    if cli_args.brief_telegram {
        match tg {
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
}

/// Handle --health mode.
pub async fn run_health_mode(tg: &Option<TelegramClient>) {
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
}

/// Handle --watch mode (health watch loop with Telegram alerts).
pub async fn run_watch_mode(tg: &Option<TelegramClient>) {
    match tg {
        None => {
            eprintln!("{RED}error:{RESET} --watch requires Telegram. Set TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID.");
            std::process::exit(1);
        }
        Some(tg_client) => {
            eprintln!("{DIM}  axonix --watch — health monitor active{RESET}");
            eprintln!("{DIM}  Press Ctrl+C to stop{RESET}");
            let config = axonix::watch::WatchConfig::default();
            axonix::watch::run_watch(config, tg_client).await;
        }
    }
}

/// Handle --listen mode (always-on Telegram listener daemon).
pub async fn run_listen_mode(tg: &Option<TelegramClient>, api_key: &str, model: &str) {
    match tg {
        None => {
            eprintln!("{RED}error:{RESET} --listen requires Telegram. Set TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID.");
            std::process::exit(1);
        }
        Some(tg_client) => {
            eprintln!("{DIM}  axonix --listen — personal assistant daemon active{RESET}");
            eprintln!("{DIM}  Press Ctrl+C to stop{RESET}");
            let config = axonix::listener::ListenerConfig::default();
            match axonix::listener::run_listener(&config, tg_client, api_key, model).await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{RED}  ✗ listener error: {e}{RESET}");
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Handle --session-summary-telegram mode.
pub async fn run_session_summary_telegram_mode(tg: &Option<TelegramClient>) {
    let summary = axonix::cycle_summary::CycleSummary::default_path();
    let msg = format_session_summary_telegram(&summary);
    match tg {
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
}

/// Handle --bluesky-post mode.
pub async fn run_bluesky_post_mode(bsky: &Option<BlueskyClient>, post_text: &str) {
    let post_text = post_text.trim();
    if post_text.is_empty() {
        eprintln!("{RED}error:{RESET} --bluesky-post requires a non-empty string.");
        std::process::exit(1);
    }
    match bsky {
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
}
