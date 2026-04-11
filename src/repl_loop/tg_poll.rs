//! Telegram inbound poll: spawns a background task that polls for bot commands.
//!
//! Returns a channel receiver that the main loop can drain after each turn.

use axonix::telegram::{BotCommand, TelegramClient};
use tokio::sync::mpsc;

/// Spawn the Telegram polling background task.
///
/// Returns `Some(rx)` if `tg` is `Some`, `None` otherwise.
pub fn spawn_telegram_poll(tg: Option<&TelegramClient>) -> Option<mpsc::Receiver<BotCommand>> {
    let tg_client = tg?;
    let (tx, rx) = mpsc::channel::<BotCommand>(16);
    let tg_poll = tg_client.clone();
    tokio::spawn(async move {
        let mut offset: i64 = 0;
        loop {
            match tg_poll.get_updates(offset).await {
                Ok(updates) => {
                    if !updates.is_empty() {
                        offset = updates
                            .iter()
                            .map(|u| u.update_id)
                            .max()
                            .unwrap_or(offset)
                            + 1;
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
}
