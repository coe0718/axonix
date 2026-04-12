//! Session initialization helpers: build system prompt, seed DB, configure git identity.
//!
//! Extracted from main.rs (G-166) to keep the binary entry point under 300 lines.

use yoagent::agent::Agent;
use yoagent::skills::SkillSet;

use axonix::render::{YELLOW, DIM, RESET};
use axonix::github::GitHubClient;

use crate::agent_setup::{build_system_prompt, make_agent};

/// Components produced by `initialize_session()`.
pub struct SessionComponents {
    pub agent: Agent,
    pub system_prompt: String,
}

/// Load memory and predictions, build the system prompt, construct the agent,
/// configure git identity inside Docker, and seed journal observations.
///
/// Returns the constructed agent and the system prompt string.
pub fn initialize_session(
    api_key: &str,
    model: &str,
    skills: SkillSet,
    gh_full: Option<&GitHubClient>,
) -> SessionComponents {
    // Load memory and predictions early so we can inject context into the system prompt (G-024).
    let startup_memory = axonix::memory::MemoryStore::load_default();
    let startup_predictions = axonix::predictions::PredictionStore::default_path();

    // Grab the first active goal title for memory context injection.
    let active_goal_title = axonix::brief::parse_active_goals()
        .into_iter()
        .next()
        .unwrap_or_default();

    let system_prompt = build_system_prompt(&startup_memory, &startup_predictions, &active_goal_title);
    let agent = make_agent(api_key, model, skills, &system_prompt);

    // Configure git identity inside Docker only (Issue #20).
    if let Some(gh_client) = gh_full {
        if std::path::Path::new("/.dockerenv").exists() {
            let cwd_str = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| ".".to_string());
            if let Err(e) = gh_client.configure_git_identity(&cwd_str) {
                eprintln!("{YELLOW}warning:{RESET} git identity config failed: {e}");
            }
        }
    }

    // Seed observations table from JOURNAL.md (G-089).
    let journal_path = std::path::Path::new("JOURNAL.md");
    if journal_path.exists() {
        match axonix::db::AxonixDb::open_default() {
            Ok(db) => {
                match db.seed_from_journal(journal_path) {
                    Ok(n) => {
                        if n > 0 {
                            eprintln!("{DIM}  seeded {n} journal entries into memory{RESET}");
                        }
                    }
                    Err(e) => eprintln!("{YELLOW}warning:{RESET} journal seed failed: {e}"),
                }
            }
            Err(e) => eprintln!("{YELLOW}warning:{RESET} db open failed for journal seed: {e}"),
        }
    }

    SessionComponents { agent, system_prompt }
}
