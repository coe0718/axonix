//! Interactive REPL loop for axonix.
//!
//! Extracted from main.rs (G-123) to keep the binary entry point focused.
//! Split into sub-modules (G-148) to keep each file ≤ 300 lines.
//! Owns the full interactive session: banner, input loop, command dispatch, AI calls.

mod banner;
mod cmd_dispatch;
mod input_loop;
mod tg_drain;
mod tg_poll;

use yoagent::skills::SkillSet;

use axonix::bluesky::BlueskyClient;
use axonix::github::GitHubClient;
use axonix::repl::ReplState;
use axonix::telegram::TelegramClient;

/// Context needed to run the interactive REPL.
pub struct ReplContext<'a> {
    pub api_key: &'a str,
    pub model: String,
    pub skills: SkillSet,
    pub system_prompt: String,
    pub tg: Option<TelegramClient>,
    pub gh: Option<GitHubClient>,
    pub bsky: Option<BlueskyClient>,
}

/// Run the interactive REPL loop. Returns when the user quits or stdin closes.
pub async fn run_repl_loop(agent: yoagent::Agent, ctx: ReplContext<'_>) {
    let ReplContext { api_key, model, skills, system_prompt, tg, gh, bsky } = ctx;

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());

    // Initialize REPL state early so memory is available for banner display.
    let repl = ReplState::new(&model);

    // Collect skill names before banner display
    let skill_names: Vec<String> = if skills.is_empty() {
        vec![]
    } else {
        skills.skills().iter().map(|s| s.name.clone()).collect()
    };

    // Print startup banner and connection status
    banner::print_startup_info(
        &model,
        skills.len(),
        !skill_names.is_empty(),
        &cwd,
        repl.memory.len(),
        tg.is_some(),
        gh.as_ref(),
        bsky.is_some(),
    );

    let session_start = std::time::Instant::now();

    // Handle Ctrl+C gracefully
    {
        use axonix::render::{DIM, RESET};
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let _flag = flag.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            eprintln!("\n{DIM}  ⚡ signal received — emergency shutdown — bye 👋{RESET}\n");
            std::process::exit(0);
        });
    }

    // Spawn Telegram inbound poll background task
    let tg_rx = tg_poll::spawn_telegram_poll(tg.as_ref());

    // Run the main input loop
    input_loop::run_input_loop(
        agent,
        api_key,
        &model,
        skills,
        &system_prompt,
        tg.as_ref(),
        gh.as_ref(),
        bsky.as_ref(),
        repl,
        tg_rx,
        session_start,
        &skill_names,
        &cwd,
    )
    .await;
}
