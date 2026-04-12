//! Command dispatch for the Telegram listener.
//!
//! [`dispatch_command`] handles a single [`BotCommand`] received from Telegram,
//! mutating the shared `DispatchCtx` in place.

use crate::conversation_memory::ConversationMemory;
use crate::telegram::{TelegramClient, BotCommand, MemoryAction, TELEGRAM_HELP_TEXT};
use crate::health::HealthSnapshot;
use crate::db::AxonixDb;
use yoagent::{AgentEvent, StreamDelta};
use yoagent::agent::Agent;

use super::config::{ListenerConfig, ListenerStats, select_model_for_command};
use super::prompt::{
    build_listener_system_prompt, make_listener_agent, get_last_commit_message,
    format_history_reply,
};
use super::handlers::{
    append_goal_to_backlog, append_prediction, resolve_prediction,
    compute_prediction_accuracy, run_mini_session, list_open_predictions,
};

/// Mutable context shared across command dispatches within a single listener loop.
pub(super) struct DispatchCtx<'a> {
    pub agent: &'a mut Agent,
    pub agent_turn_count: &'a mut usize,
    pub mem: &'a mut ConversationMemory,
    pub stats: &'a mut ListenerStats,
    pub config: &'a ListenerConfig,
    pub tg: &'a TelegramClient,
    pub model: &'a str,
    pub haiku_model: &'a str,
    pub api_key: &'a str,
}

/// Dispatch a single recognised [`BotCommand`] to its handler.
///
/// Mutates `ctx` in place (memory, stats, agent turn count).
pub(super) async fn dispatch_command(cmd: BotCommand, ctx: &mut DispatchCtx<'_>) {
    match cmd {
        BotCommand::Ask(ask_cmd) => {
            // Send "processing" acknowledgement
            let _ = ctx.tg.reply_to("⏳ Processing...", ask_cmd.message_id).await;

            // Rebuild agent if it has processed too many turns (context hygiene)
            if *ctx.agent_turn_count > 50 {
                let refresh_goal = crate::brief::parse_active_goals().into_iter().next();
                let refresh_title = refresh_goal.as_deref().unwrap_or("");
                let refresh_ctx = crate::brief::collect_memory_context(refresh_title);
                let fresh_prompt = build_listener_system_prompt(ctx.mem, refresh_goal.as_deref(), &refresh_ctx);
                let refresh_ask_model = select_model_for_command("/ask", ctx.model, ctx.haiku_model);
                *ctx.agent = make_listener_agent(ctx.api_key, &refresh_ask_model, &fresh_prompt);
                *ctx.agent_turn_count = 0;
            }

            // Run the agent — prompt() returns a streaming event receiver
            let mut rx = ctx.agent.prompt(&ask_cmd.prompt).await;
            let mut response_text = String::new();
            let mut had_error = false;

            while let Some(event) = rx.recv().await {
                match event {
                    AgentEvent::MessageUpdate {
                        delta: StreamDelta::Text { delta },
                        ..
                    } => {
                        response_text.push_str(&delta);
                    }
                    AgentEvent::InputRejected { reason } => {
                        eprintln!("  ⚠ listener: input rejected: {reason}");
                        had_error = true;
                    }
                    _ => {}
                }
            }

            if had_error || response_text.is_empty() {
                let msg = if had_error {
                    "⚠️ Request was rejected by the agent.".to_string()
                } else {
                    "⚠️ No response from agent.".to_string()
                };
                let _ = ctx.tg.reply_to(&msg, ask_cmd.message_id).await;
                ctx.stats.errors += 1;
            } else {
                // Truncate to max_response_chars
                let reply = if response_text.chars().count() > ctx.config.max_response_chars {
                    let truncated: String = response_text
                        .chars()
                        .take(ctx.config.max_response_chars)
                        .collect();
                    format!("{truncated}\n_(truncated)_")
                } else {
                    response_text.clone()
                };

                let _ = ctx.tg.reply_to(&reply, ask_cmd.message_id).await;

                // Record turn in memory
                ctx.mem.push("user", &ask_cmd.prompt, "telegram");
                ctx.mem.push("assistant", &response_text, "telegram");
                if let Err(e) = ctx.mem.save() {
                    eprintln!("  ⚠ listener: failed to save memory: {e}");
                }

                *ctx.agent_turn_count += 2; // user + assistant
                ctx.stats.messages_handled += 1;
            }
        }
        BotCommand::Health { message_id } => {
            let snap = HealthSnapshot::collect();
            let reply = snap.format_compact();
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Brief { message_id } => {
            let brief = crate::brief::Brief::collect();
            let reply = brief.format_telegram();
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Goal { description, message_id } => {
            match append_goal_to_backlog(&description) {
                Ok(()) => {
                    let reply = format!("✅ Goal added to backlog:\n_{description}_");
                    let _ = ctx.tg.reply_to(&reply, message_id).await;
                }
                Err(e) => {
                    let reply = format!("⚠️ Failed to add goal: {e}");
                    let _ = ctx.tg.reply_to(&reply, message_id).await;
                    ctx.stats.errors += 1;
                }
            }
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Run { task, message_id } => {
            let _ = ctx.tg.reply_to(&format!("⏳ Running task: _{task}_"), message_id).await;
            // /run uses Sonnet — needs full reasoning and code capability
            let run_model = select_model_for_command("/run", ctx.model, ctx.haiku_model);
            match run_mini_session(&task, ctx.api_key, &run_model).await {
                Ok(result) => {
                    let reply = TelegramClient::format_response(&result);
                    for chunk in &reply {
                        let _ = ctx.tg.reply_to(chunk, message_id).await;
                    }
                }
                Err(e) => {
                    let _ = ctx.tg.reply_to(&format!("⚠️ Task failed: {e}"), message_id).await;
                    ctx.stats.errors += 1;
                }
            }
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Status { message_id } => {
            let active_goal = crate::brief::parse_active_goals().into_iter().next();
            let last_commit = get_last_commit_message();
            // Compute prediction accuracy for G-117
            let accuracy = compute_prediction_accuracy();
            let reply = TelegramClient::format_enhanced_status_reply(
                ctx.model,
                ctx.stats.uptime_secs,
                active_goal.as_deref(),
                last_commit.as_deref(),
                accuracy.as_deref(),
            );
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Help { message_id } => {
            let _ = ctx.tg.reply_to(TELEGRAM_HELP_TEXT, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Memory { action, message_id } => {
            let day = std::env::var("DAY_COUNT")
                .ok()
                .and_then(|s| s.split_whitespace().next().map(|n| n.to_string()))
                .unwrap_or_else(|| "?".to_string());
            let session_label = format!("Day {day}");
            let reply = match action {
                MemoryAction::Add { text, category } => {
                    match AxonixDb::open_default() {
                        Ok(db) => match db.sobs_insert(&text, &category, "", "", &session_label, "") {
                            Ok(_) => format!("✅ Observation stored (category: {category})"),
                            Err(e) => format!("❌ Failed to store observation: {e}"),
                        },
                        Err(e) => format!("❌ DB unavailable: {e}"),
                    }
                }
                MemoryAction::Search { query } => {
                    match AxonixDb::open_default() {
                        Ok(db) => match db.sobs_search(&query, 5) {
                            Ok(results) if results.is_empty() => "No observations found.".to_string(),
                            Ok(results) => results
                                .iter()
                                .map(|o| format!("[{}] {}\n  _{}_", o.category, o.content, o.created_at))
                                .collect::<Vec<_>>()
                                .join("\n\n"),
                            Err(e) => format!("❌ Search failed: {e}"),
                        },
                        Err(e) => format!("❌ DB unavailable: {e}"),
                    }
                }
                MemoryAction::List => {
                    match AxonixDb::open_default() {
                        Ok(db) => match db.sobs_list(5) {
                            Ok(results) if results.is_empty() => "No observations recorded yet.".to_string(),
                            Ok(results) => results
                                .iter()
                                .map(|o| format!("[{}] {}\n  _{}_", o.category, o.content, o.created_at))
                                .collect::<Vec<_>>()
                                .join("\n\n"),
                            Err(e) => format!("❌ List failed: {e}"),
                        },
                        Err(e) => format!("❌ DB unavailable: {e}"),
                    }
                }
            };
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::History { message_id } => {
            let reply = format_history_reply(ctx.mem, 5);
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Goals { message_id } => {
            let active = crate::brief::parse_active_goals();
            let backlog = crate::brief::parse_backlog_goals();
            let mut lines: Vec<String> = Vec::new();
            lines.push("*Active Goals:*".to_string());
            if active.is_empty() {
                lines.push("  _(none)_".to_string());
            } else {
                for g in &active {
                    lines.push(format!("  • {g}"));
                }
            }
            lines.push("*Next Backlog:*".to_string());
            if let Some(first) = backlog.into_iter().next() {
                lines.push(format!("  • {first}"));
            } else {
                lines.push("  _(empty)_".to_string());
            }
            let reply = lines.join("\n");
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Predict { text, message_id } => {
            let reply = match append_prediction(&text) {
                Ok(id) => format!("✅ Prediction #{id} saved: {text}"),
                Err(e) => format!("❌ Failed to save prediction: {e}"),
            };
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::Resolve { id, verdict, message_id } => {
            let reply = match resolve_prediction(id, verdict) {
                Ok(text) => {
                    let label = if verdict { "✅ correct" } else { "❌ wrong" };
                    format!("Prediction #{id} marked {label}:\n_{text}_")
                }
                Err(e) => format!("⚠️ Could not resolve prediction: {e}"),
            };
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
        BotCommand::ListPredictions { message_id } => {
            let reply = list_open_predictions();
            let _ = ctx.tg.reply_to(&reply, message_id).await;
            ctx.stats.messages_handled += 1;
        }
    }
}
