//! Main poll loop for the always-on Telegram listener daemon.
//!
//! This module contains [`run_listener`], which polls Telegram continuously,
//! dispatches commands to the appropriate handlers, and manages the agent context.

use crate::conversation_memory::ConversationMemory;
use crate::telegram::TelegramClient;

use super::config::{
    ListenerConfig, AckedIssues, ListenerStats, DEFAULT_HAIKU_MODEL,
    select_model_for_command, parse_rate_limit_env, local_hour,
};
use super::prompt::{build_listener_system_prompt, make_listener_agent};
use super::dispatch::{DispatchCtx, dispatch_command};
use super::proactive::{poll_github_issues, send_daily_brief};

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

            let mut ctx = DispatchCtx {
                agent: &mut agent,
                agent_turn_count: &mut agent_turn_count,
                mem: &mut mem,
                stats: &mut stats,
                config,
                tg,
                model,
                haiku_model: &haiku_model,
                api_key,
            };
            dispatch_command(cmd, &mut ctx).await;
        }

        // Log stats summary every 100 messages
        if stats.messages_handled > 0 && stats.messages_handled % 100 == 0 {
            eprintln!("  {}", stats.format());
        }

        // GitHub issue polling — runs every github_poll_interval_secs
        if last_github_poll.elapsed().as_secs() >= config.github_poll_interval_secs {
            last_github_poll = std::time::Instant::now();
            if let Some(ref token) = gh_token {
                poll_github_issues(tg, token, &mut acked_issues).await;
            }
        }

        // Daily morning brief — sends once per day at daily_brief_hour
        let current_hour = local_hour();
        let should_send_brief = current_hour == config.daily_brief_hour
            && last_brief_hour != Some(current_hour);
        if should_send_brief {
            last_brief_hour = Some(current_hour);
            send_daily_brief(tg).await;
        }

        // Sleep between polls
        tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;
    }
}
