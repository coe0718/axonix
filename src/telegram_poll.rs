//! Background Telegram polling for non-interactive (cron/piped) sessions.
//!
//! Extracted from main.rs (G-123) to keep the binary entry point focused.

use axonix::telegram::TelegramClient;

/// Spawn a background Telegram poll task for non-interactive (cron/piped) sessions.
///
/// Handles `/help`, `/status`, and queued `/ask` commands while the main prompt
/// is running. `/ask` prompts are queued and returned via the receiver; callers
/// should drain the channel after the main prompt completes.
///
/// Returns `None` if Telegram is not configured.
pub fn spawn_telegram_cron_poll(
    tg: &Option<TelegramClient>,
    model: &str,
) -> Option<tokio::sync::mpsc::Receiver<axonix::telegram::AskCommand>> {
    let tg_client = tg.as_ref()?;
    let (ask_tx, ask_rx) = tokio::sync::mpsc::channel::<axonix::telegram::AskCommand>(8);
    let tg_poll = tg_client.clone();
    let model_clone = model.to_string();
    tokio::spawn(async move {
        let mut offset: i64 = 0;
        let start = std::time::Instant::now();
        loop {
            match tg_poll.get_updates(offset).await {
                Ok(updates) => {
                    if !updates.is_empty() {
                        offset = updates.iter().map(|u| u.update_id).max().unwrap_or(offset) + 1;
                        let commands = tg_poll.extract_commands(&updates);
                        for cmd in commands {
                            match cmd {
                                axonix::telegram::BotCommand::Help { message_id } => {
                                    tg_poll.reply_to(axonix::telegram::TELEGRAM_HELP_TEXT, message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::Status { message_id } => {
                                    let elapsed = start.elapsed().as_secs();
                                    let reply = TelegramClient::format_status_reply(
                                        &model_clone,
                                        "cron",
                                        elapsed,
                                        0, // token counts not available in bg task
                                        0,
                                    );
                                    tg_poll.reply_to(&reply, message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::Health { message_id } => {
                                    let snapshot = axonix::health::HealthSnapshot::collect();
                                    let docker = axonix::health::docker_health();
                                    let reply = format!("{}\n\n{}", snapshot.format(), docker.format());
                                    tg_poll.reply_to(&reply, message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::Brief { message_id } => {
                                    let brief = axonix::brief::Brief::collect();
                                    tg_poll.reply_to(&brief.format_telegram(), message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::Ask(ask_cmd) => {
                                    // Queue for processing after main prompt completes
                                    if ask_tx.send(ask_cmd).await.is_err() {
                                        return; // receiver dropped (session ended)
                                    }
                                }
                                axonix::telegram::BotCommand::Run { task, message_id } => {
                                    tg_poll.reply_to(&format!("⏳ Running task: _{task}_"), message_id).await.ok();
                                    // In cron mode, queue as an ask
                                    if ask_tx.send(axonix::telegram::AskCommand { prompt: task, message_id }).await.is_err() {
                                        return;
                                    }
                                }
                                axonix::telegram::BotCommand::Goal { description: _, message_id } => {
                                    tg_poll.reply_to("⚠️ /goal is handled by the listener daemon.", message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::Memory { action: _, message_id } => {
                                    tg_poll.reply_to("⚠️ /memory is handled by the listener daemon.", message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::History { message_id } => {
                                    tg_poll.reply_to("⚠️ /history is handled by the listener daemon.", message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::Goals { message_id } => {
                                    tg_poll.reply_to("⚠️ /goals is handled by the listener daemon.", message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::Predict { text: _, message_id } => {
                                    tg_poll.reply_to("⚠️ /predict is handled by the listener daemon.", message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::Resolve { id: _, verdict: _, message_id } => {
                                    tg_poll.reply_to("⚠️ /resolve is handled by the listener daemon.", message_id).await.ok();
                                }
                                axonix::telegram::BotCommand::ListPredictions { message_id } => {
                                    tg_poll.reply_to("⚠️ /predictions is handled by the listener daemon.", message_id).await.ok();
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });
    Some(ask_rx)
}
