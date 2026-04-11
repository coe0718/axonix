//! Startup banner and status display for the REPL.

use axonix::github::GitHubClient;
use axonix::render::*;

/// Print the REPL startup info: model, skills, memory, optional integrations.
pub fn print_startup_info(
    model: &str,
    skill_count: usize,
    skill_names_non_empty: bool,
    cwd: &str,
    memory_len: usize,
    tg_connected: bool,
    gh: Option<&GitHubClient>,
    bsky_connected: bool,
) {
    axonix::cli::print_banner();
    println!("{DIM}  model: {model}{RESET}");
    if skill_names_non_empty {
        println!("{DIM}  skills: {skill_count} loaded{RESET}");
    }
    println!("{DIM}  cwd:   {cwd}{RESET}");
    if memory_len > 0 {
        println!("{DIM}  memory: {memory_len} facts loaded — /memory list to view{RESET}");
    }
    if tg_connected {
        println!("{DIM}  telegram: connected — send /ask <prompt> to chat with me{RESET}");
    }
    if let Some(gh_client) = gh {
        println!(
            "{DIM}  github:   {} — use /comment <n> <text> to post issue comments{RESET}",
            gh_client.identity.display_name()
        );
    }
    if bsky_connected {
        println!("{DIM}  bluesky:  connected — use --bluesky-post <text> to post{RESET}");
    }
    println!("{DIM}  Type /help for commands{RESET}\n");
}
