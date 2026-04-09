//! REPL state types: `ReplState`, `CommandResult`, and `HISTORY_LIMIT`.

use std::collections::VecDeque;
use crate::memory::MemoryStore;
use crate::predictions::PredictionStore;
use crate::ssh::HostRegistry;

/// All mutable state for an interactive REPL session.
pub struct ReplState {
    /// Current model name. May be changed via `/model`.
    pub model: String,
    /// Session-total input token count.
    pub total_input: u64,
    /// Session-total output token count.
    pub total_output: u64,
    /// Session-total cache-read token count.
    pub total_cache_read: u64,
    /// Session-total cache-write token count.
    pub total_cache_write: u64,
    /// The last user prompt (for `/retry`).
    pub last_prompt: Option<String>,
    /// Ordered history of all user prompts this session (oldest first).
    /// Capped at `HISTORY_LIMIT` entries. Uses VecDeque for O(1) front removal.
    pub history: VecDeque<String>,
    /// SSH host registry loaded from hosts.toml.
    pub ssh_hosts: HostRegistry,
    /// Persistent memory store (.axonix/memory.json).
    pub memory: MemoryStore,
    /// Persistent prediction store (.axonix/predictions.json).
    pub predictions: PredictionStore,
}

/// Maximum number of prompts kept in session history.
pub const HISTORY_LIMIT: usize = 50;

impl ReplState {
    /// Create a fresh REPL state with the given model.
    pub fn new(model: impl Into<String>) -> Self {
        let mut ssh_hosts = HostRegistry::new();
        ssh_hosts.load_defaults();
        Self {
            model: model.into(),
            total_input: 0,
            total_output: 0,
            total_cache_read: 0,
            total_cache_write: 0,
            last_prompt: None,
            history: VecDeque::new(),
            ssh_hosts,
            memory: MemoryStore::load_default(),
            predictions: PredictionStore::default_path(),
        }
    }

    /// Reset all token counters (called on `/clear` and `/model`).
    pub fn reset_tokens(&mut self) {
        self.total_input = 0;
        self.total_output = 0;
        self.total_cache_read = 0;
        self.total_cache_write = 0;
    }

    /// Record a user prompt in history and update `last_prompt`.
    /// Oldest entries are dropped when history exceeds `HISTORY_LIMIT`.
    /// Uses VecDeque::pop_front for O(1) removal from the front.
    pub fn push_prompt(&mut self, prompt: impl Into<String>) {
        let p = prompt.into();
        self.last_prompt = Some(p.clone());
        self.history.push_back(p);
        if self.history.len() > HISTORY_LIMIT {
            self.history.pop_front();
        }
    }

    /// Retrieve a history entry by 1-based index (as shown in `/history`).
    /// Returns `None` if index is out of range.
    pub fn history_entry(&self, n: usize) -> Option<&str> {
        if n == 0 || n > self.history.len() {
            None
        } else {
            Some(&self.history[n - 1])
        }
    }
}

/// What the REPL loop should do after `handle_command` returns.
#[derive(Debug, PartialEq)]
pub enum CommandResult {
    /// A normal command was handled. Continue the loop.
    Handled(Vec<String>),
    /// The user typed `/quit` or `/exit`. Break the loop.
    Quit,
    /// Not a `/command` — treat input as a prompt for the agent.
    NotACommand,
    /// A model switch was requested. Contains the new model name.
    SwitchModel(String),
    /// `/clear` — reset conversation.
    Clear,
    /// `/retry` or `/retry N` — re-run a prompt from history.
    /// Carries the prompt text to replay.
    Retry(String),
    /// `/issues [N]` — fetch open GitHub issues sorted by reactions.
    /// Carries the limit (default 10, max 30).
    FetchIssues(u8),
    /// `/archive-journal` — archive old journal entries to docs/archive/JOURNAL.md.
    ArchiveJournal,
    /// `/memory-search <query>` — search stored observations by keyword.
    MemorySearch(String),
    /// `/memory recent` — show the 5 most recently stored hot memories from axonix.db.
    ShowRecentMemories,
}
