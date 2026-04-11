//! Drains queued Telegram bot commands after each REPL turn.

use axonix::brief::Brief;
use axonix::health::{docker_health, HealthSnapshot};
use axonix::render::*;
use axonix::telegram::{BotCommand, TelegramClient, TELEGRAM_HELP_TEXT};
use axonix::repl::ReplState;
use tokio::sync::mpsc;
use yoagent::Agent;

use crate::prompt_runner::run_prompt;

/// Drain all pending Telegram bot commands from the channel.
///
/// Called after each REPL turn. Processes `Ask`, `Help`, `Status`, `Health`,
/// `Brief`, and `Run` commands inline; others are forwarded to the listener
/// daemon with a notice message.
pub async fn drain_telegram_commands(
    tg_rx: &mut Option<mpsc::Receiver<BotCommand>>,
    agent: &mut Agent,
    repl: &mut ReplState,
    tg: Option<&TelegramClient>,
    session_start: std::time::Instant,
) {
    let rx = match tg_rx.as_mut() {
        Some(r) => r,
        None => return,
    };
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            BotCommand::Ask(ask_cmd) => {
                let ask_prompt = ask_cmd.prompt.clone();
                let msg_id = ask_cmd.message_id;
                println!("\n{DIM}  📱 Telegram ask: {}{RESET}", truncate(&ask_prompt, 60));
                repl.push_prompt(&ask_prompt);
                run_prompt(agent, &ask_prompt, repl, tg).await;
                if let Some(tg_client) = tg {
                    tg_client.reply_to("✅ Done", msg_id).await.ok();
                }
            }
            BotCommand::Help { message_id } => {
                println!("\n{DIM}  📱 Telegram /help{RESET}");
                if let Some(tg_client) = tg {
                    tg_client.reply_to(TELEGRAM_HELP_TEXT, message_id).await.ok();
                }
            }
            BotCommand::Status { message_id } => {
                println!("\n{DIM}  📱 Telegram /status{RESET}");
                if let Some(tg_client) = tg {
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
            BotCommand::Health { message_id } => {
                println!("\n{DIM}  📱 Telegram /health{RESET}");
                if let Some(tg_client) = tg {
                    let snapshot = HealthSnapshot::collect();
                    let docker = docker_health();
                    let reply = format!("{}\n\n{}", snapshot.format(), docker.format());
                    tg_client.reply_to(&reply, message_id).await.ok();
                }
            }
            BotCommand::Brief { message_id } => {
                println!("\n{DIM}  📱 Telegram /brief{RESET}");
                if let Some(tg_client) = tg {
                    let brief = Brief::collect();
                    tg_client.reply_to(&brief.format_telegram(), message_id).await.ok();
                }
            }
            BotCommand::Run { task, message_id } => {
                println!("\n{DIM}  📱 Telegram /run: {}{RESET}", truncate(&task, 60));
                if let Some(tg_client) = tg {
                    tg_client
                        .reply_to(&format!("⏳ Running task: _{task}_"), message_id)
                        .await
                        .ok();
                    run_prompt(agent, &task, repl, tg).await;
                    tg_client.reply_to("✅ Done", message_id).await.ok();
                }
            }
            BotCommand::Goal { description, message_id } => {
                println!("\n{DIM}  📱 Telegram /goal: {}{RESET}", truncate(&description, 60));
                if let Some(tg_client) = tg {
                    tg_client
                        .reply_to("⚠️ /goal is handled by the listener daemon.", message_id)
                        .await
                        .ok();
                }
            }
            BotCommand::Memory { action: _, message_id } => {
                println!("\n{DIM}  📱 Telegram /memory{RESET}");
                if let Some(tg_client) = tg {
                    tg_client
                        .reply_to("⚠️ /memory is handled by the listener daemon.", message_id)
                        .await
                        .ok();
                }
            }
            BotCommand::History { message_id } => {
                println!("\n{DIM}  📱 Telegram /history{RESET}");
                if let Some(tg_client) = tg {
                    tg_client
                        .reply_to("⚠️ /history is handled by the listener daemon.", message_id)
                        .await
                        .ok();
                }
            }
            BotCommand::Goals { message_id } => {
                println!("\n{DIM}  📱 Telegram /goals{RESET}");
                if let Some(tg_client) = tg {
                    tg_client
                        .reply_to("⚠️ /goals is handled by the listener daemon.", message_id)
                        .await
                        .ok();
                }
            }
            BotCommand::Predict { text: _, message_id } => {
                println!("\n{DIM}  📱 Telegram /predict{RESET}");
                if let Some(tg_client) = tg {
                    tg_client
                        .reply_to("⚠️ /predict is handled by the listener daemon.", message_id)
                        .await
                        .ok();
                }
            }
            BotCommand::Resolve { id: _, verdict: _, message_id } => {
                println!("\n{DIM}  📱 Telegram /resolve{RESET}");
                if let Some(tg_client) = tg {
                    tg_client
                        .reply_to("⚠️ /resolve is handled by the listener daemon.", message_id)
                        .await
                        .ok();
                }
            }
            BotCommand::ListPredictions { message_id } => {
                println!("\n{DIM}  📱 Telegram /predictions{RESET}");
                if let Some(tg_client) = tg {
                    tg_client
                        .reply_to("⚠️ /predictions is handled by the listener daemon.", message_id)
                        .await
                        .ok();
                }
            }
        }
    }
}
