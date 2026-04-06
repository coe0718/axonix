//! Main poll loop for the always-on Telegram listener daemon.
//!
//! This module contains [`run_listener`], which polls Telegram continuously,
//! dispatches commands to the appropriate handlers, and manages the agent context.

use crate::conversation_memory::ConversationMemory;
use crate::telegram::{TelegramClient, BotCommand, MemoryAction, TELEGRAM_HELP_TEXT};
use crate::health::HealthSnapshot;
use crate::db::AxonixDb;
use yoagent::{AgentEvent, StreamDelta};

use super::config::{
    ListenerConfig, AckedIssues, ListenerStats, DEFAULT_HAIKU_MODEL,
    select_model_for_command, parse_rate_limit_env, local_hour,
};
use super::prompt::{
    build_listener_system_prompt, make_listener_agent, get_last_commit_message,
    format_history_reply,
};
use super::handlers::{
    append_goal_to_backlog, append_prediction, resolve_prediction,
    compute_prediction_accuracy, run_mini_session, list_open_predictions,
};

/// Run the always-on Telegram listener loop.
///
/// Polls Telegram every `config.poll_interval_secs` seconds.
/// Handles `/ask <text>` commands with a short-context agent response.
/// Records every conversation turn to ConversationMemory.
/// Runs until Ctrl+C (i.e. until the process is killed).
#[allow(clippy::too_many_lines)]
pub async fn run_listener(
    config: &ListenerConfig,
    tg: &TelegramClient,
    api_key: &str,
    model: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Derive Haiku model for lightweight commands (LISTENER_HAIKU_MODEL env var or default)
    let haiku_model = std::env::var("LISTENER_HAIKU_MODEL")
        .unwrap_or_else(|_| DEFAULT_HAIKU_MODEL.to_string());

    // Load conversation memory from config path or default
    let memory_path = config
        .memory_path
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(crate::conversation_memory::default_conversation_memory_path);
    let mut mem = ConversationMemory::load(&memory_path);

    // Build system prompt and agent (refreshed if context grows too large)
    // /ask uses Haiku — cheap, fast responses
    let active_goal = crate::brief::parse_active_goals().into_iter().next();
    let goal_title_ref = active_goal.as_deref().unwrap_or("");
    let mem_ctx = crate::brief::collect_memory_context(goal_title_ref);
    let system_prompt = build_listener_system_prompt(&mem, active_goal.as_deref(), &mem_ctx);
    let ask_model = select_model_for_command("/ask", model, &haiku_model);
    let mut agent = make_listener_agent(api_key, &ask_model, &system_prompt);
    let mut agent_turn_count: usize = 0;

    let mut stats = ListenerStats::new();
    let mut offset: i64 = 0;
    let start_time = std::time::Instant::now();

    // Proactive work state
    let mut last_github_poll = std::time::Instant::now()
        - std::time::Duration::from_secs(config.github_poll_interval_secs);
    let mut last_brief_hour: Option<u8> = None;
    let acked_path = config
        .acked_issues_path
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(AckedIssues::default_path);
    let mut acked_issues = AckedIssues::load(&acked_path);

    // Load GitHub token once (prefer AXONIX_BOT_TOKEN, fall back to GH_TOKEN)
    let gh_token = std::env::var("AXONIX_BOT_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .ok();

    // Per-user rate limiting state.
    // Since there is only one authorised operator chat, we use a single global
    // bucket keyed on 0i64 rather than extracting chat_id from every BotCommand variant.
    let mut rate_map: std::collections::HashMap<i64, (u32, std::time::Instant)> =
        std::collections::HashMap::new();
    let (rate_max, rate_window) = parse_rate_limit_env(
        config.rate_limit_max,
        config.rate_limit_window_secs,
    );

    loop {
        // Update uptime
        stats.uptime_secs = start_time.elapsed().as_secs();

        // Poll for updates
        let updates = match tg.get_updates(offset).await {
            Ok(u) => u,
            Err(e) => {
                eprintln!("  ⚠ listener: get_updates error: {e}");
                stats.errors += 1;
                tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
                continue;
            }
        };

        // Advance offset to acknowledge received updates
        if let Some(last) = updates.last() {
            offset = last.update_id + 1;
        }

        // Extract all recognised commands from the batch
        let commands = tg.extract_commands(&updates);

        for cmd in commands {
            // Rate limiting check (global bucket — single authorised operator).
            let now = std::time::Instant::now();
            let entry = rate_map.entry(0i64).or_insert((0, now));
            if now.duration_since(entry.1) > std::time::Duration::from_secs(rate_window) {
                // Window has expired — reset the counter.
                *entry = (0, now);
            }
            entry.0 += 1;
            if entry.0 > rate_max {
                let msg = format!(
                    "⏳ Slow down! You can send {} commands per {}s. Try again shortly.",
                    rate_max, rate_window
                );
                let _ = tg.send_message(&msg).await;
                continue;
            }

            match cmd {
                BotCommand::Ask(ask_cmd) => {
                    // Send "processing" acknowledgement
                    let _ = tg.reply_to("⏳ Processing...", ask_cmd.message_id).await;

                    // Rebuild agent if it has processed too many turns (context hygiene)
                    if agent_turn_count > 50 {
                        let refresh_goal = crate::brief::parse_active_goals().into_iter().next();
                        let refresh_title = refresh_goal.as_deref().unwrap_or("");
                        let refresh_ctx = crate::brief::collect_memory_context(refresh_title);
                        let fresh_prompt = build_listener_system_prompt(&mem, refresh_goal.as_deref(), &refresh_ctx);
                        let refresh_ask_model = select_model_for_command("/ask", model, &haiku_model);
                        agent = make_listener_agent(api_key, &refresh_ask_model, &fresh_prompt);
                        agent_turn_count = 0;
                    }

                    // Run the agent — prompt() returns a streaming event receiver
                    let mut rx = agent.prompt(&ask_cmd.prompt).await;
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
                        let _ = tg.reply_to(&msg, ask_cmd.message_id).await;
                        stats.errors += 1;
                    } else {
                        // Truncate to max_response_chars
                        let reply = if response_text.chars().count() > config.max_response_chars {
                            let truncated: String = response_text
                                .chars()
                                .take(config.max_response_chars)
                                .collect();
                            format!("{truncated}\n_(truncated)_")
                        } else {
                            response_text.clone()
                        };

                        let _ = tg.reply_to(&reply, ask_cmd.message_id).await;

                        // Record turn in memory
                        mem.push("user", &ask_cmd.prompt, "telegram");
                        mem.push("assistant", &response_text, "telegram");
                        if let Err(e) = mem.save() {
                            eprintln!("  ⚠ listener: failed to save memory: {e}");
                        }

                        agent_turn_count += 2; // user + assistant
                        stats.messages_handled += 1;
                    }
                }
                BotCommand::Health { message_id } => {
                    let snap = HealthSnapshot::collect();
                    let reply = snap.format_compact();
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::Brief { message_id } => {
                    let brief = crate::brief::Brief::collect();
                    let reply = brief.format_telegram();
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::Goal { description, message_id } => {
                    match append_goal_to_backlog(&description) {
                        Ok(()) => {
                            let reply = format!("✅ Goal added to backlog:\n_{description}_");
                            let _ = tg.reply_to(&reply, message_id).await;
                        }
                        Err(e) => {
                            let reply = format!("⚠️ Failed to add goal: {e}");
                            let _ = tg.reply_to(&reply, message_id).await;
                            stats.errors += 1;
                        }
                    }
                    stats.messages_handled += 1;
                }
                BotCommand::Run { task, message_id } => {
                    let _ = tg.reply_to(&format!("⏳ Running task: _{task}_"), message_id).await;
                    // /run uses Sonnet — needs full reasoning and code capability
                    let run_model = select_model_for_command("/run", model, &haiku_model);
                    match run_mini_session(&task, api_key, &run_model).await {
                        Ok(result) => {
                            let reply = TelegramClient::format_response(&result);
                            for chunk in &reply {
                                let _ = tg.reply_to(chunk, message_id).await;
                            }
                        }
                        Err(e) => {
                            let _ = tg.reply_to(&format!("⚠️ Task failed: {e}"), message_id).await;
                            stats.errors += 1;
                        }
                    }
                    stats.messages_handled += 1;
                }
                BotCommand::Status { message_id } => {
                    let active_goal = crate::brief::parse_active_goals().into_iter().next();
                    let last_commit = get_last_commit_message();
                    // Compute prediction accuracy for G-117
                    let accuracy = compute_prediction_accuracy();
                    let reply = TelegramClient::format_enhanced_status_reply(
                        model,
                        stats.uptime_secs,
                        active_goal.as_deref(),
                        last_commit.as_deref(),
                        accuracy.as_deref(),
                    );
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::Help { message_id } => {
                    let _ = tg.reply_to(TELEGRAM_HELP_TEXT, message_id).await;
                    stats.messages_handled += 1;
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
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::History { message_id } => {
                    let reply = format_history_reply(&mem, 5);
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
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
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::Predict { text, message_id } => {
                    let reply = match append_prediction(&text) {
                        Ok(id) => format!("✅ Prediction #{id} saved: {text}"),
                        Err(e) => format!("❌ Failed to save prediction: {e}"),
                    };
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::Resolve { id, verdict, message_id } => {
                    let reply = match resolve_prediction(id, verdict) {
                        Ok(text) => {
                            let label = if verdict { "✅ correct" } else { "❌ wrong" };
                            format!("Prediction #{id} marked {label}:\n_{text}_")
                        }
                        Err(e) => format!("⚠️ Could not resolve prediction: {e}"),
                    };
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
                BotCommand::ListPredictions { message_id } => {
                    let reply = list_open_predictions();
                    let _ = tg.reply_to(&reply, message_id).await;
                    stats.messages_handled += 1;
                }
            }
        }

        // Log stats summary every 100 messages
        if stats.messages_handled > 0 && stats.messages_handled % 100 == 0 {
            eprintln!("  {}", stats.format());
        }

        // GitHub issue polling — runs every github_poll_interval_secs
        if last_github_poll.elapsed().as_secs() >= config.github_poll_interval_secs {
            last_github_poll = std::time::Instant::now();
            if let Some(ref token) = gh_token {
                let gh = crate::github::GitHubClient::new(
                    token,
                    crate::github::GitHubIdentity::Bot,
                );
                match gh.list_issues("coe0718/axonix", 20).await {
                    Ok(issues) => {
                        let new_issues: Vec<_> = issues
                            .iter()
                            .filter(|i| i.labels.iter().any(|l| l == "agent-input"))
                            .filter(|i| !acked_issues.contains(u64::from(i.number)))
                            .collect();
                        for issue in &new_issues {
                            let day = std::env::var("DAY_COUNT")
                                .ok()
                                .and_then(|s| {
                                    s.split_whitespace().next().map(|n| n.to_string())
                                })
                                .unwrap_or_else(|| "?".to_string());
                            let session = std::env::var("SESSION_COUNT")
                                .ok()
                                .unwrap_or_else(|| "?".to_string());
                            let ack_msg = format!(
                                "Picked up in Day {day} Session {session} — I'll look at this in the next available cron window.",
                            );
                            match gh
                                .post_comment(
                                    "coe0718/axonix",
                                    u64::from(issue.number),
                                    &ack_msg,
                                )
                                .await
                            {
                                Ok(_) => {
                                    acked_issues.insert(u64::from(issue.number));
                                    let _ = acked_issues.save();
                                    eprintln!(
                                        "  ✓ acknowledged issue #{}",
                                        issue.number
                                    );
                                }
                                Err(e) => {
                                    eprintln!(
                                        "  ⚠ listener: failed to ack issue #{}: {e}",
                                        issue.number
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  ⚠ listener: github poll error: {e}");
                    }
                }
            }
        }

        // Daily morning brief — sends once per day at daily_brief_hour
        let current_hour = local_hour();
        let should_send_brief = current_hour == config.daily_brief_hour
            && last_brief_hour != Some(current_hour);
        if should_send_brief {
            last_brief_hour = Some(current_hour);
            let brief = crate::brief::Brief::collect();
            let msg = brief.format_telegram();
            match tg.send_message(&msg).await {
                Ok(_) => eprintln!("  ✓ listener: daily brief sent"),
                Err(e) => eprintln!("  ⚠ listener: failed to send daily brief: {e}"),
            }
        }

        // Sleep between polls
        tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
    }
}
