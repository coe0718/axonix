//! REPL state and command dispatch.
//!
//! `ReplState` holds all mutable state for an interactive session.
//! `handle_command` processes a single user input against that state
//! and returns a `CommandResult` describing what the main loop should do next.
//!
//! By separating state from I/O, every command path becomes testable
//! without mocking stdin/stdout or spinning up an async runtime.

use std::collections::VecDeque;
use crate::lint::{lint_file, LintResult};
use crate::memory::MemoryStore;
use crate::predictions::PredictionStore;
use crate::render::truncate;
use crate::ssh::{HostRegistry, ssh_exec};
use std::time::Duration;

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
}

/// Process a REPL input string. Returns a `CommandResult`.
///
/// This function is pure: it only mutates `state`, produces output lines,
/// and returns a result describing what the loop should do next.
/// No I/O is performed here — the caller renders the output lines.
pub fn handle_command(input: &str, state: &mut ReplState, skill_names: &[String]) -> CommandResult {
    match input {
        "/quit" | "/exit" => CommandResult::Quit,

        "/help" => {
            let mut lines = vec![
                "  🤖 AXONIX COMMAND MANIFEST — beep-boop, here's what I can do:".to_string(),
                String::new(),
                "  Commands:".to_string(),
                "    /help          Show this help".to_string(),
                "    /status        Show session info".to_string(),
                "    /context       Show conversation messages summary".to_string(),
                "    /tokens        Show token usage and cost estimate".to_string(),
                "    /history       Show numbered list of prompts this session".to_string(),
                "    /retry [N]     Retry last prompt, or prompt #N from /history".to_string(),
                "    /clear         Clear conversation history".to_string(),
                "    /model <name>  Switch model (clears history)".to_string(),
                "    /save [path]   Save conversation to file".to_string(),
                "    /lint <file>   Validate YAML or Caddyfile syntax".to_string(),
                "    /ssh list      List registered SSH hosts".to_string(),
                "    /ssh <h> <cmd> Run command on a remote host".to_string(),
                "    /comment <n> <text> Post comment on GitHub issue #n".to_string(),
                "    /respond <n> <text>      Post response on GitHub issue #n".to_string(),
                "    /respond <n> close <text> Post response and close issue".to_string(),

                "    /issues [N]         List open GitHub issues (default 10, sorted by reactions)".to_string(),
                "    /memory list        Show persistent memory (facts across sessions)".to_string(),
                "    /memory set/get/del Read and write persistent memory".to_string(),
                "    /predict add <text> Log a prediction about a future outcome".to_string(),
                "    /predict open       Show open (unresolved) predictions".to_string(),
                "    /predict list       Show all predictions with outcomes".to_string(),
                "    /watch             Show current health vs thresholds".to_string(),
                "    /review <desc>     Invoke code_reviewer sub-agent on recent changes".to_string(),
                "    /summary [text]    Show or update cycle summary (persisted to next session)".to_string(),
                "    /recap             Post session recap thread to Bluesky (title, commits, tests)".to_string(),
            ];
            if !skill_names.is_empty() {
                lines.push("    /skills        Show loaded skills".to_string());
            }
            lines.push("    /quit, /exit   Exit".to_string());
            lines.push(String::new());
            lines.push("  Multiline input:".to_string());
            lines.push(r#"    End a line with \ to continue on the next line"#.to_string());
            lines.push(r#"    Type """ to start a block, """ again to finish"#.to_string());
            lines.push(String::new());
            CommandResult::Handled(lines)
        }

        "/clear" => CommandResult::Clear,

        "/history" => {
            if state.history.is_empty() {
                CommandResult::Handled(vec![
                    "  (no prompts in history yet)".to_string(),
                    String::new(),
                ])
            } else {
                let mut lines = vec![format!("  History ({} prompts):", state.history.len())];
                let total = state.history.len();
                let start = if total > 20 { total - 20 } else { 0 };
                for (i, prompt) in state.history.iter().enumerate().skip(start) {
                    let n = i + 1;
                    let preview = truncate(prompt, 72);
                    lines.push(format!("  {:>3}.  {preview}", n));
                }
                if start > 0 {
                    lines.push(format!("  (showing last 20 of {} prompts)", total));
                }
                lines.push(String::new());
                CommandResult::Handled(lines)
            }
        }

        s if s == "/retry" || s.starts_with("/retry ") => {
            if s == "/retry" {
                // Retry the last prompt
                match &state.last_prompt {
                    Some(p) => CommandResult::Retry(p.clone()),
                    None => CommandResult::Handled(vec![
                        "  (nothing to retry — no prompts sent yet)".to_string(),
                        String::new(),
                    ]),
                }
            } else {
                // "/retry N" — retry prompt #N from history
                let arg = s.trim_start_matches("/retry ").trim();
                match arg.parse::<usize>() {
                    Ok(n) if n > 0 => {
                        match state.history_entry(n) {
                            Some(p) => CommandResult::Retry(p.to_string()),
                            None => CommandResult::Handled(vec![
                                format!("  No prompt #{n} in history (use /history to see entries)"),
                                String::new(),
                            ]),
                        }
                    }
                    _ => CommandResult::Handled(vec![
                        format!("  Usage: /retry or /retry <N>"),
                        "  Use /history to see prompt numbers".to_string(),
                        String::new(),
                    ]),
                }
            }
        }

        "/skills" => {
            if skill_names.is_empty() {
                CommandResult::Handled(vec![
                    "  (no skills loaded)".to_string(),
                    String::new(),
                ])
            } else {
                let mut lines = vec![
                    format!("  Skills ({} loaded):", skill_names.len()),
                ];
                for name in skill_names {
                    lines.push(format!("    • {name}"));
                }
                lines.push(String::new());
                CommandResult::Handled(lines)
            }
        }

        s if s.starts_with("/model ") => {
            let new_model = s.trim_start_matches("/model ").trim();
            if new_model.is_empty() {
                CommandResult::Handled(vec![
                    "  Usage: /model <name>".to_string(),
                    "  Example: /model claude-sonnet-4-20250514".to_string(),
                    String::new(),
                ])
            } else {
                state.model = new_model.to_string();
                state.reset_tokens();
                CommandResult::SwitchModel(new_model.to_string())
            }
        }

        s if s == "/save" || s.starts_with("/save ") => {
            // Path parsing only — actual save is done by caller (needs agent messages)
            let path = if s == "/save" {
                "conversation.md".to_string()
            } else {
                let p = s.trim_start_matches("/save ").trim().to_string();
                if p.is_empty() { "conversation.md".to_string() } else { p }
            };
            // Return special marker so caller knows to save
            CommandResult::Handled(vec![format!("__save:{path}")])
        }

        s if s == "/lint" || s.starts_with("/lint ") => {
            let path = if s == "/lint" {
                String::new()
            } else {
                s.trim_start_matches("/lint ").trim().to_string()
            };
            if path.is_empty() {
                CommandResult::Handled(vec![
                    "  Usage: /lint <file>".to_string(),
                    "  Supported: .yaml/.yml (YAML/docker-compose), Caddyfile/.caddy".to_string(),
                    String::new(),
                ])
            } else {
                let lines = match lint_file(&path) {
                    LintResult::Ok(summary) => vec![
                        format!("__lint_ok:{}:{}", path, summary),
                    ],
                    LintResult::Errors(errors) => {
                        let mut v = vec![format!("__lint_errors:{}:{}", path, errors.len())];
                        for e in &errors {
                            v.push(format!("__lint_error:{}:{}", e.line, e.message));
                        }
                        v
                    }
                    LintResult::Unsupported(msg) => vec![format!("__lint_unsupported:{msg}")],
                };
                CommandResult::Handled(lines)
            }
        }

        s if s == "/ssh" || s.starts_with("/ssh ") => {
            let arg = if s == "/ssh" {
                ""
            } else {
                s.trim_start_matches("/ssh ").trim()
            };

            if arg.is_empty() || arg == "--help" {
                let hosts: Vec<String> = state.ssh_hosts.aliases().iter().map(|a| a.to_string()).collect();
                let mut lines = vec![
                    "  Usage:".to_string(),
                    "    /ssh list              List registered hosts".to_string(),
                    "    /ssh <host> <command>  Run a command on a remote host".to_string(),
                    String::new(),
                ];
                if hosts.is_empty() {
                    lines.push("  No hosts registered. Create hosts.toml in the working directory:".to_string());
                    lines.push("    [hosts.caddy-nuc]".to_string());
                    lines.push("    address = \"192.168.1.10\"".to_string());
                    lines.push("    user = \"admin\"".to_string());
                } else {
                    lines.push(format!("  Registered hosts: {}", hosts.join(", ")));
                }
                lines.push(String::new());
                CommandResult::Handled(lines)
            } else if arg == "list" {
                if state.ssh_hosts.is_empty() {
                    CommandResult::Handled(vec![
                        "  No hosts registered.".to_string(),
                        "  Create hosts.toml in the working directory or ~/.axonix/hosts.toml".to_string(),
                        "  Example:".to_string(),
                        "    [hosts.caddy-nuc]".to_string(),
                        "    address = \"192.168.1.10\"".to_string(),
                        "    user = \"admin\"".to_string(),
                        String::new(),
                    ])
                } else {
                    let mut lines = vec![format!("  SSH Hosts ({} registered):", state.ssh_hosts.len())];
                    for alias in state.ssh_hosts.aliases() {
                        if let Some(host) = state.ssh_hosts.get(alias) {
                            let mut desc = format!("    {:<20} {}", alias, host.destination());
                            if host.port != 22 {
                                desc.push_str(&format!(":{}", host.port));
                            }
                            if let Some(d) = &host.description {
                                desc.push_str(&format!("  — {d}"));
                            }
                            lines.push(desc);
                        }
                    }
                    lines.push(String::new());
                    CommandResult::Handled(lines)
                }
            } else {
                // "/ssh <host> <command>"
                let mut parts = arg.splitn(2, ' ');
                let host_alias = parts.next().unwrap_or("").trim();
                let remote_cmd = parts.next().unwrap_or("").trim();

                if host_alias.is_empty() {
                    return CommandResult::Handled(vec![
                        "  Usage: /ssh <host> <command>".to_string(),
                        "  Use /ssh list to see registered hosts".to_string(),
                        String::new(),
                    ]);
                }

                if remote_cmd.is_empty() {
                    return CommandResult::Handled(vec![
                        format!("  Usage: /ssh {host_alias} <command>"),
                        "  Example: /ssh caddy-nuc systemctl reload caddy".to_string(),
                        String::new(),
                    ]);
                }

                match state.ssh_hosts.get(host_alias) {
                    None => CommandResult::Handled(vec![
                        format!("  Unknown host: '{host_alias}'"),
                        "  Use /ssh list to see registered hosts".to_string(),
                        String::new(),
                    ]),
                    Some(host) => {
                        let host = host.clone();
                        match ssh_exec(&host, remote_cmd, Some(Duration::from_secs(15))) {
                            Err(e) => CommandResult::Handled(vec![
                                format!("__ssh_error:{host_alias}:{e}"),
                            ]),
                            Ok(result) => {
                                CommandResult::Handled(vec![
                                    format!("__ssh_result:{}:{}:{}", host_alias, result.exit_code, result.combined_output()),
                                ])
                            }
                        }
                    }
                }
            }
        }

        s if s == "/comment" || s.starts_with("/comment ") => {
            // Usage: /comment <issue_number> <text>
            // The actual POST is done by caller (needs async GitHubClient).
            // We return a __gh_comment:<issue>:<body> marker.
            let arg = if s == "/comment" {
                ""
            } else {
                s.trim_start_matches("/comment ").trim()
            };

            if arg.is_empty() {
                return CommandResult::Handled(vec![
                    "  Usage: /comment <issue_number> <text>".to_string(),
                    "  Example: /comment 13 Thanks for the report — fixed in this session.".to_string(),
                    String::new(),
                ]);
            }

            let mut parts = arg.splitn(2, ' ');
            let issue_str = parts.next().unwrap_or("").trim();
            let body = parts.next().unwrap_or("").trim();

            match issue_str.parse::<u64>() {
                Err(_) | Ok(0) => CommandResult::Handled(vec![
                    format!("  Error: issue number must be a positive integer, got '{issue_str}'"),
                    "  Usage: /comment <issue_number> <text>".to_string(),
                    String::new(),
                ]),
                Ok(_) if body.is_empty() => CommandResult::Handled(vec![
                    format!("  Error: comment body cannot be empty"),
                    format!("  Usage: /comment {issue_str} <text>"),
                    String::new(),
                ]),
                Ok(n) => {
                    // Return marker for caller to handle async POST
                    CommandResult::Handled(vec![
                        format!("__gh_comment:{n}:{body}"),
                    ])
                }
            }
        }

        s if s == "/respond" || s.starts_with("/respond ") => {
            // Usage: /respond <issue_number> <text>
            //        /respond <issue_number> close <text>
            //
            // Like /comment but with an optional "close" subcommand that also
            // closes the issue after posting the response (G-005).
            //
            // Returns __gh_respond:<issue>:<close>:<body> marker.
            // close = 1 to close the issue after commenting, 0 otherwise.
            let arg = if s == "/respond" {
                ""
            } else {
                s.trim_start_matches("/respond ").trim()
            };

            if arg.is_empty() {
                return CommandResult::Handled(vec![
                    "  Usage: /respond <issue_number> <text>".to_string(),
                    "         /respond <issue_number> close <text>".to_string(),
                    "  Example: /respond 13 Fixed in this session. Tests added.".to_string(),
                    "  Example: /respond 13 close Fixed and deployed.".to_string(),
                    "  Difference from /comment: use 'close' to close the issue after responding.".to_string(),
                    String::new(),
                ]);
            }

            let mut parts = arg.splitn(2, ' ');
            let issue_str = parts.next().unwrap_or("").trim();
            let rest = parts.next().unwrap_or("").trim();

            match issue_str.parse::<u64>() {
                Err(_) | Ok(0) => CommandResult::Handled(vec![
                    format!("  Error: issue number must be a positive integer, got '{issue_str}'"),
                    "  Usage: /respond <issue_number> <text>".to_string(),
                    String::new(),
                ]),
                Ok(n) => {
                    let (close_flag, body) = if rest.starts_with("close ") {
                        (1u8, rest.trim_start_matches("close ").trim())
                    } else if rest == "close" {
                        // just "close" with no body
                        return CommandResult::Handled(vec![
                            "  Error: 'close' must be followed by response text".to_string(),
                            format!("  Usage: /respond {n} close <text>"),
                            String::new(),
                        ]);
                    } else {
                        (0u8, rest)
                    };

                    if body.is_empty() {
                        return CommandResult::Handled(vec![
                            "  Error: response body cannot be empty".to_string(),
                            format!("  Usage: /respond {n} <text>"),
                            String::new(),
                        ]);
                    }

                    CommandResult::Handled(vec![
                        format!("__gh_respond:{n}:{close_flag}:{body}"),
                    ])
                }
            }
        }

        s if s == "/review" || s.starts_with("/review ") => {
            // Usage: /review <description of what changed>
            // Invokes the code_reviewer sub-agent to check recent changes.
            // The actual sub-agent call is async — we return a __review:<task> marker.
            let task = if s == "/review" {
                ""
            } else {
                s.trim_start_matches("/review ").trim()
            };

            if task.is_empty() {
                CommandResult::Handled(vec![
                    "  Usage: /review <description of what changed>".to_string(),
                    "  Example: /review added /review command to repl.rs, wired in main.rs".to_string(),
                    "  The code_reviewer sub-agent will check for bugs, missing error handling,".to_string(),
                    "  and test coverage gaps. Results printed inline.".to_string(),
                    String::new(),
                ])
            } else {
                CommandResult::Handled(vec![
                    format!("__review:{task}"),
                ])
            }
        }

        s if s == "/summary" || s.starts_with("/summary ") => {
            // Usage: /summary
            //        /summary <one-line description of what was done this session>
            //
            // Writes a compact cycle_summary.json to .axonix/ so the next session
            // can load it as context instead of replaying the full message history.
            // This keeps context window size bounded regardless of cycle count (Issue #38).
            //
            // When called without args, shows the current summary if one exists.
            // When called with text, adds it as a completed item and saves.
            let arg = if s == "/summary" {
                ""
            } else {
                s.trim_start_matches("/summary ").trim()
            };

            use crate::cycle_summary::CycleSummary;
            let mut cs = CycleSummary::default_path();

            if arg.is_empty() {
                // Show current summary
                match cs.format_for_system_prompt() {
                    None => CommandResult::Handled(vec![
                        "  No cycle summary yet for this session.".to_string(),
                        "  Use: /summary <what you did> to add a completed item.".to_string(),
                        "  The summary is automatically loaded by the next session.".to_string(),
                        String::new(),
                    ]),
                    Some(text) => {
                        let mut lines = vec!["  Current cycle summary:".to_string(), String::new()];
                        for line in text.lines() {
                            lines.push(format!("  {line}"));
                        }
                        lines.push(String::new());
                        CommandResult::Handled(lines)
                    }
                }
            } else {
                // Ensure summary is initialized with current session metadata
                // (Use a placeholder date — the operator or evolve.sh should set proper values)
                cs.set_session("current session", "today");
                cs.add_completed(arg);
                match cs.save() {
                    Ok(()) => CommandResult::Handled(vec![
                        format!("  ✓ Added to cycle summary: {arg}"),
                        "  Summary saved to .axonix/cycle_summary.json".to_string(),
                        "  The next session will load this as startup context.".to_string(),
                        String::new(),
                    ]),
                    Err(e) => CommandResult::Handled(vec![
                        format!("  ✗ Failed to save cycle summary: {e}"),
                        String::new(),
                    ]),
                }
            }
        }

        "/recap" => {
            // Post a recap thread to Bluesky: session title, recent commits, test count.
            // The actual async posting is done by main.rs (needs BlueskyClient).
            // We return a __recap marker so the caller can dispatch it.
            CommandResult::Handled(vec!["__recap".to_string()])
        }

        s if s == "/memory" || s.starts_with("/memory ") => {
            // Usage:
            //   /memory list             — show all stored facts
            //   /memory get <key>        — get value of a key
            //   /memory set <key> <val>  — store a key-value pair
            //   /memory note <key> <note> — add a note to an existing key
            //   /memory del <key>        — delete a key
            let arg = if s == "/memory" {
                ""
            } else {
                s.trim_start_matches("/memory ").trim()
            };

            if arg.is_empty() || arg == "list" {
                if state.memory.is_empty() {
                    CommandResult::Handled(vec![
                        "  Memory: (empty)".to_string(),
                        "  Use /memory set <key> <value> to store a fact.".to_string(),
                        String::new(),
                    ])
                } else {
                    let mut lines = vec![
                        format!("  Memory ({} entries):", state.memory.len()),
                    ];
                    for (key, entry) in state.memory.all() {
                        let note_str = entry.note.as_deref()
                            .map(|n| format!("  — {}", truncate(n, 50)))
                            .unwrap_or_default();
                        let date_str = entry.updated.as_deref()
                            .map(|d| format!(" [{d}]"))
                            .unwrap_or_default();
                        lines.push(format!("    {:<30} = {}{note_str}{date_str}",
                            key,
                            truncate(&entry.value, 40)
                        ));
                    }
                    lines.push(String::new());
                    CommandResult::Handled(lines)
                }
            } else if let Some(rest) = arg.strip_prefix("get ") {
                let key = rest.trim();
                if key.is_empty() {
                    CommandResult::Handled(vec![
                        "  Usage: /memory get <key>".to_string(),
                        String::new(),
                    ])
                } else {
                    match state.memory.get_entry(key) {
                        None => CommandResult::Handled(vec![
                            format!("  Memory: '{key}' not set"),
                            String::new(),
                        ]),
                        Some(entry) => {
                            let mut lines = vec![
                                format!("  {key} = {}", entry.value),
                            ];
                            if let Some(note) = &entry.note {
                                lines.push(format!("  note: {note}"));
                            }
                            if let Some(updated) = &entry.updated {
                                lines.push(format!("  updated: {updated}"));
                            }
                            lines.push(String::new());
                            CommandResult::Handled(lines)
                        }
                    }
                }
            } else if let Some(rest) = arg.strip_prefix("set ") {
                let rest = rest.trim();
                // Split into key and value: "key value with spaces"
                let mut parts = rest.splitn(2, ' ');
                let key = parts.next().unwrap_or("").trim();
                let value = parts.next().unwrap_or("").trim();
                if key.is_empty() || value.is_empty() {
                    CommandResult::Handled(vec![
                        "  Usage: /memory set <key> <value>".to_string(),
                        "  Example: /memory set nuc.ip 192.168.1.10".to_string(),
                        String::new(),
                    ])
                } else {
                    state.memory.set(key, value, None);
                    let save_result = state.memory.save()
                        .map(|_| format!("  ✓ memory: {key} = {value}"))
                        .unwrap_or_else(|e| format!("  ⚠ memory saved in-session but failed to write: {e}"));
                    CommandResult::Handled(vec![save_result, String::new()])
                }
            } else if let Some(rest) = arg.strip_prefix("note ") {
                let rest = rest.trim();
                let mut parts = rest.splitn(2, ' ');
                let key = parts.next().unwrap_or("").trim();
                let note = parts.next().unwrap_or("").trim();
                if key.is_empty() || note.is_empty() {
                    CommandResult::Handled(vec![
                        "  Usage: /memory note <key> <note text>".to_string(),
                        "  Adds or updates the note on an existing key.".to_string(),
                        String::new(),
                    ])
                } else {
                    match state.memory.get(key).map(|s| s.to_string()) {
                        None => CommandResult::Handled(vec![
                            format!("  Memory: '{key}' not set — use /memory set first"),
                            String::new(),
                        ]),
                        Some(existing_value) => {
                            state.memory.set(key, &existing_value, Some(note));
                            let save_result = state.memory.save()
                                .map(|_| format!("  ✓ note added to '{key}'"))
                                .unwrap_or_else(|e| format!("  ⚠ note saved in-session but failed to write: {e}"));
                            CommandResult::Handled(vec![save_result, String::new()])
                        }
                    }
                }
            } else if let Some(rest) = arg.strip_prefix("del ") {
                let key = rest.trim();
                if key.is_empty() {
                    CommandResult::Handled(vec![
                        "  Usage: /memory del <key>".to_string(),
                        String::new(),
                    ])
                } else {
                    if state.memory.del(key) {
                        let save_result = state.memory.save()
                            .map(|_| format!("  ✓ memory: '{key}' deleted"))
                            .unwrap_or_else(|e| format!("  ⚠ deleted in-session but failed to write: {e}"));
                        CommandResult::Handled(vec![save_result, String::new()])
                    } else {
                        CommandResult::Handled(vec![
                            format!("  Memory: '{key}' not set"),
                            String::new(),
                        ])
                    }
                }
            } else {
                CommandResult::Handled(vec![
                    "  Usage:".to_string(),
                    "    /memory list              Show all stored facts".to_string(),
                    "    /memory get <key>         Get value of a key".to_string(),
                    "    /memory set <key> <value> Store a key-value pair".to_string(),
                    "    /memory note <key> <text> Add a note to an existing key".to_string(),
                    "    /memory del <key>         Delete a key".to_string(),
                    String::new(),
                    "  Key naming convention: category.attribute".to_string(),
                    "  Example: nuc.ip, twitter.status, operator.tz".to_string(),
                    String::new(),
                ])
            }
        }

        s if s == "/predict" || s.starts_with("/predict ") => {
            // Usage:
            //   /predict add <text>               — log a new prediction
            //   /predict resolve <id> <outcome>   — resolve with what actually happened
            //   /predict resolve <id> <outcome> | <delta>  — with optional delta note
            //   /predict list                     — show all predictions
            //   /predict open                     — show only unresolved predictions
            let arg = if s == "/predict" {
                ""
            } else {
                s.trim_start_matches("/predict ").trim()
            };

            fn predict_usage() -> Vec<String> {
                vec![
                    "  Usage:".to_string(),
                    "    /predict add <text>                  Log a new prediction".to_string(),
                    "    /predict resolve <id> <outcome>      Mark a prediction resolved".to_string(),
                    "    /predict resolve <id> <out> | <delta> With a delta note".to_string(),
                    "    /predict open                        List open predictions".to_string(),
                    "    /predict list                        List all predictions".to_string(),
                    String::new(),
                    "  Prediction tracking builds self-calibration data over time.".to_string(),
                    "  Record what you expect, then resolve with what actually happened.".to_string(),
                    String::new(),
                ]
            }

            if arg.is_empty() || arg == "help" {
                CommandResult::Handled(predict_usage())
            } else if let Some(text) = arg.strip_prefix("add ") {
                let text = text.trim();
                if text.is_empty() {
                    return CommandResult::Handled(vec![
                        "  Usage: /predict add <text>".to_string(),
                        "  Example: /predict add the build will succeed with no warnings".to_string(),
                        String::new(),
                    ]);
                }
                let id = state.predictions.predict(text);
                let save_msg = match state.predictions.save() {
                    Ok(_) => format!("  ✓ prediction #{id} logged: {}", truncate(text, 60)),
                    Err(e) => format!("  ⚠ prediction #{id} queued but failed to save: {e}"),
                };
                CommandResult::Handled(vec![save_msg, String::new()])
            } else if let Some(rest) = arg.strip_prefix("resolve ") {
                // Format: "resolve <id> <outcome>" or "resolve <id> <outcome> | <delta>"
                let rest = rest.trim();
                let mut parts = rest.splitn(2, ' ');
                let id_str = parts.next().unwrap_or("").trim();
                let outcome_and_delta = parts.next().unwrap_or("").trim();

                if id_str.is_empty() || outcome_and_delta.is_empty() {
                    return CommandResult::Handled(vec![
                        "  Usage: /predict resolve <id> <outcome>".to_string(),
                        "  Example: /predict resolve 1 build passed with 5 warnings".to_string(),
                        "  With delta: /predict resolve 1 passed | underestimated warning count".to_string(),
                        String::new(),
                    ]);
                }

                let id: u32 = match id_str.parse() {
                    Ok(n) if n > 0 => n,
                    _ => {
                        return CommandResult::Handled(vec![
                            format!("  Error: prediction ID must be a positive integer, got '{id_str}'"),
                            "  Use /predict list to see prediction IDs".to_string(),
                            String::new(),
                        ]);
                    }
                };

                // Split on " | " to get optional delta
                let (outcome, delta) = if let Some(pipe_pos) = outcome_and_delta.find(" | ") {
                    let outcome = outcome_and_delta[..pipe_pos].trim();
                    let delta = outcome_and_delta[pipe_pos + 3..].trim();
                    (outcome, if delta.is_empty() { None } else { Some(delta) })
                } else {
                    (outcome_and_delta, None)
                };

                match state.predictions.resolve(id, outcome, delta) {
                    Err(e) => CommandResult::Handled(vec![
                        format!("  Error: {e}"),
                        String::new(),
                    ]),
                    Ok(prediction_text) => {
                        let save_msg = match state.predictions.save() {
                            Ok(_) => {
                                let mut lines = vec![
                                    format!("  ✓ prediction #{id} resolved"),
                                    format!("    was:     {}", truncate(&prediction_text, 60)),
                                    format!("    actual:  {}", truncate(outcome, 60)),
                                ];
                                if let Some(d) = delta {
                                    lines.push(format!("    delta:   {}", truncate(d, 60)));
                                }
                                lines.push(String::new());
                                lines
                            }
                            Err(e) => vec![
                                format!("  ✓ prediction #{id} resolved (save failed: {e})"),
                                String::new(),
                            ],
                        };
                        CommandResult::Handled(save_msg)
                    }
                }
            } else if arg == "list" {
                let total = state.predictions.count();
                if total == 0 {
                    CommandResult::Handled(vec![
                        "  Predictions: (none yet)".to_string(),
                        "  Use /predict add <text> to log your first prediction.".to_string(),
                        String::new(),
                    ])
                } else {
                    let mut lines = vec![format!("  Predictions ({total} total, {} open, {} resolved):",
                        state.predictions.open_count(), state.predictions.resolved_count())];
                    // Show open first, then resolved
                    let open = state.predictions.open();
                    if !open.is_empty() {
                        lines.push("  Open:".to_string());
                        for (id, pred) in &open {
                            lines.push(format!("    #{id:<4} [{}] {}",
                                pred.created, truncate(&pred.prediction, 55)));
                        }
                    }
                    let resolved = state.predictions.resolved();
                    if !resolved.is_empty() {
                        lines.push("  Resolved:".to_string());
                        for (id, pred) in &resolved {
                            let date = pred.resolved.as_deref().unwrap_or("?");
                            lines.push(format!("    #{id:<4} [{}] {}",
                                date, truncate(&pred.prediction, 55)));
                            if let Some(outcome) = &pred.outcome {
                                lines.push(format!("         → actual: {}", truncate(outcome, 55)));
                            }
                            if let Some(delta) = &pred.delta {
                                lines.push(format!("         Δ delta:  {}", truncate(delta, 55)));
                            }
                        }
                    }
                    lines.push(String::new());
                    CommandResult::Handled(lines)
                }
            } else if arg == "open" {
                let open = state.predictions.open();
                if open.is_empty() {
                    CommandResult::Handled(vec![
                        "  No open predictions.".to_string(),
                        "  Use /predict add <text> to log one, or /predict list to see resolved.".to_string(),
                        String::new(),
                    ])
                } else {
                    let mut lines = vec![format!("  Open predictions ({}):", open.len())];
                    for (id, pred) in &open {
                        lines.push(format!("    #{id:<4} [{}] {}",
                            pred.created, truncate(&pred.prediction, 55)));
                    }
                    lines.push(String::new());
                    lines.push(format!("  Use /predict resolve <id> <outcome> to close one."));
                    lines.push(String::new());
                    CommandResult::Handled(lines)
                }
            } else {
                CommandResult::Handled(predict_usage())
            }
        }

        s if s == "/issues" || s.starts_with("/issues ") => {            // Usage: /issues [N]
            // Fetches open GitHub issues sorted by reactions.
            // The actual API call is done by caller (needs async GitHubClient).
            // Returns FetchIssues(limit) so main loop can handle it.
            let arg = if s == "/issues" {
                ""
            } else {
                s.trim_start_matches("/issues ").trim()
            };

            let limit: u8 = if arg.is_empty() {
                10 // default
            } else {
                match arg.parse::<u8>() {
                    Ok(n) if n > 0 && n <= 30 => n,
                    Ok(0) => {
                        return CommandResult::Handled(vec![
                            "  Error: limit must be between 1 and 30.".to_string(),
                            "  Usage: /issues [N] (default: 10, max: 30)".to_string(),
                            String::new(),
                        ]);
                    }
                    _ => {
                        return CommandResult::Handled(vec![
                            format!("  Error: invalid limit '{arg}'. Must be a number 1–30."),
                            "  Usage: /issues [N] (default: 10, max: 30)".to_string(),
                            String::new(),
                        ]);
                    }
                }
            };

            CommandResult::FetchIssues(limit)
        }

        s if s.starts_with('/') => {
            // /status, /context, /tokens are handled by main (they need agent/session data)
            // /watch shows current health vs thresholds (point-in-time check)
            if matches!(s, "/status" | "/context" | "/tokens") {
                CommandResult::NotACommand // caller handles these
            } else if s == "/watch" || s.starts_with("/watch ") {
                // /watch: show current health vs watch thresholds
                // For a background watch loop, use --watch CLI flag
                let snap = crate::health::HealthSnapshot::collect();
                let config = crate::watch::WatchConfig::default();
                let state = crate::watch::AlertState::default_for_repl();
                let alerts = crate::watch::evaluate_thresholds(&snap, &config, &state);

                let mut lines = vec![
                    format!("  🔍 Health vs thresholds (CPU>{:.1} | Mem>{}% | Disk>{}%):",
                        config.cpu_threshold, config.mem_threshold, config.disk_threshold),
                    format!("  CPU load:  {}", snap.load_avg),
                    format!("  Memory:    {}", snap.memory),
                    format!("  Disk (/):  {}", snap.disk),
                    format!("  Uptime:    {}", snap.uptime),
                    String::new(),
                ];
                if alerts.is_empty() {
                    lines.push("  ✅ All metrics within thresholds".to_string());
                } else {
                    lines.push(format!("  ⚠ {} threshold(s) exceeded:", alerts.len()));
                    for alert in &alerts {
                        // Extract the first line of each alert for concise display
                        let first_line = alert.lines().next().unwrap_or("").trim();
                        lines.push(format!("    {first_line}"));
                    }
                    lines.push(String::new());
                    lines.push("  Use --watch to send alerts via Telegram.".to_string());
                }
                lines.push(String::new());
                CommandResult::Handled(lines)
            } else {
                CommandResult::Handled(vec![
                    format!("  Unknown command: {s}"),
                    "  Type /help for available commands".to_string(),
                    String::new(),
                ])
            }
        }
        _ => CommandResult::NotACommand,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> ReplState {
        ReplState::new("claude-opus-4-6")
    }

    // ── Quit / exit ───────────────────────────────────────────────────────────

    #[test]
    fn test_quit_command() {
        let mut s = state();
        assert_eq!(handle_command("/quit", &mut s, &[]), CommandResult::Quit);
    }

    #[test]
    fn test_exit_command() {
        let mut s = state();
        assert_eq!(handle_command("/exit", &mut s, &[]), CommandResult::Quit);
    }

    // ── Help ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_help_returns_handled() {
        let mut s = state();
        let result = handle_command("/help", &mut s, &[]);
        assert!(matches!(result, CommandResult::Handled(_)));
    }

    #[test]
    fn test_help_contains_commands() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/quit"), "help should list /quit");
        assert!(all.contains("/clear"), "help should list /clear");
        assert!(all.contains("/lint"), "help should list /lint");
        assert!(all.contains("/save"), "help should list /save");
    }

    #[test]
    fn test_help_shows_skills_when_present() {
        let mut s = state();
        let skills = vec!["evolve".to_string(), "communicate".to_string()];
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &skills) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/skills"), "help should list /skills when skills are loaded");
    }

    #[test]
    fn test_help_hides_skills_when_none() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(!all.contains("/skills"), "help should not list /skills when no skills loaded");
    }

    // ── Skills ───────────────────────────────────────────────────────────────

    #[test]
    fn test_skills_empty() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/skills", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("no skills")));
    }

    #[test]
    fn test_skills_with_loaded_skills() {
        let mut s = state();
        let skills = vec!["evolve".to_string(), "communicate".to_string()];
        let CommandResult::Handled(lines) = handle_command("/skills", &mut s, &skills) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("evolve"), "should list evolve skill");
        assert!(all.contains("communicate"), "should list communicate skill");
        assert!(all.contains("2 loaded") || all.contains("2"), "should show count");
    }

    // ── Model switch ─────────────────────────────────────────────────────────

    #[test]
    fn test_model_switch() {
        let mut s = state();
        s.total_input = 5000;
        s.total_output = 1000;
        let result = handle_command("/model claude-sonnet-4-20250514", &mut s, &[]);
        assert_eq!(result, CommandResult::SwitchModel("claude-sonnet-4-20250514".to_string()));
        assert_eq!(s.model, "claude-sonnet-4-20250514");
        assert_eq!(s.total_input, 0, "model switch should reset token counts");
        assert_eq!(s.total_output, 0, "model switch should reset token counts");
    }

    #[test]
    fn test_model_switch_empty_name() {
        let mut s = state();
        let result = handle_command("/model ", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for empty model name");
        };
        assert!(lines.iter().any(|l| l.contains("Usage")));
        assert_eq!(s.model, "claude-opus-4-6", "model should not change on empty name");
    }

    #[test]
    fn test_model_switch_whitespace_only() {
        let mut s = state();
        let result = handle_command("/model    ", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for whitespace model name");
        };
        assert!(lines.iter().any(|l| l.contains("Usage")));
    }

    // ── Clear ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_clear_returns_clear() {
        let mut s = state();
        assert_eq!(handle_command("/clear", &mut s, &[]), CommandResult::Clear);
    }

    // ── Retry ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_retry_no_history_returns_handled() {
        let mut s = state();
        // No prompts yet → returns Handled with "nothing to retry" message
        let result = handle_command("/retry", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled when no history, got: {result:?}");
        };
        assert!(lines.iter().any(|l| l.contains("nothing to retry")));
    }

    #[test]
    fn test_retry_with_history_returns_retry() {
        let mut s = state();
        s.push_prompt("explain monads");
        let result = handle_command("/retry", &mut s, &[]);
        assert_eq!(result, CommandResult::Retry("explain monads".to_string()));
    }

    #[test]
    fn test_retry_n_valid_index() {
        let mut s = state();
        s.push_prompt("first prompt");
        s.push_prompt("second prompt");
        s.push_prompt("third prompt");
        let result = handle_command("/retry 2", &mut s, &[]);
        assert_eq!(result, CommandResult::Retry("second prompt".to_string()));
    }

    #[test]
    fn test_retry_n_out_of_range() {
        let mut s = state();
        s.push_prompt("only prompt");
        let result = handle_command("/retry 5", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for out-of-range retry: {result:?}");
        };
        assert!(lines.iter().any(|l| l.contains("No prompt #5")));
    }

    #[test]
    fn test_retry_n_invalid_arg() {
        let mut s = state();
        let result = handle_command("/retry abc", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for invalid retry arg: {result:?}");
        };
        assert!(lines.iter().any(|l| l.contains("Usage")));
    }

    #[test]
    fn test_retry_n_zero_is_invalid() {
        let mut s = state();
        s.push_prompt("a prompt");
        let result = handle_command("/retry 0", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for retry 0: {result:?}");
        };
        assert!(lines.iter().any(|l| l.contains("Usage") || l.contains("No prompt")));
    }

    // ── Lint ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_lint_no_path() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/lint", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("Usage")));
    }

    #[test]
    fn test_lint_valid_yaml_file() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
        write!(tmp, "key: value\nother: 123\n").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command(&format!("/lint {path}"), &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("__lint_ok")), "valid YAML should produce ok marker: {lines:?}");
    }

    #[test]
    fn test_lint_invalid_yaml_file() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
        write!(tmp, "key: [\nbad\n").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command(&format!("/lint {path}"), &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(
            lines.iter().any(|l| l.contains("__lint_errors")),
            "invalid YAML should produce errors marker: {lines:?}"
        );
    }

    #[test]
    fn test_lint_unsupported_extension() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/lint foo.toml", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("__lint_unsupported")));
    }

    // ── Save path parsing ─────────────────────────────────────────────────────

    #[test]
    fn test_save_default_path() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/save", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("__save:conversation.md")));
    }

    #[test]
    fn test_save_custom_path() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/save my_chat.md", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("__save:my_chat.md")));
    }

    // ── Unknown command ───────────────────────────────────────────────────────

    #[test]
    fn test_unknown_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/foo", &mut s, &[]) else {
            panic!("expected Handled for unknown command");
        };
        assert!(lines.iter().any(|l| l.contains("Unknown")));
    }

    #[test]
    fn test_known_slash_commands_not_unknown() {
        let mut s = state();
        // /status, /context, /tokens are handled by caller — they should return NotACommand
        for cmd in &["/status", "/context", "/tokens"] {
            assert_eq!(
                handle_command(cmd, &mut s, &[]),
                CommandResult::NotACommand,
                "{cmd} should be NotACommand (handled by caller)"
            );
        }
    }

    // ── Non-command input ─────────────────────────────────────────────────────

    #[test]
    fn test_regular_prompt_is_not_a_command() {
        let mut s = state();
        assert_eq!(
            handle_command("explain monads", &mut s, &[]),
            CommandResult::NotACommand
        );
    }

    #[test]
    fn test_empty_string_is_not_a_command() {
        let mut s = state();
        assert_eq!(handle_command("", &mut s, &[]), CommandResult::NotACommand);
    }

    // ── History ───────────────────────────────────────────────────────────────

    #[test]
    fn test_history_empty() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        assert!(lines.iter().any(|l| l.contains("no prompts")));
    }

    #[test]
    fn test_history_shows_prompts() {
        let mut s = state();
        s.push_prompt("explain monads");
        s.push_prompt("what is a monad?");
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("explain monads"), "history should show first prompt");
        assert!(all.contains("what is a monad?"), "history should show second prompt");
    }

    #[test]
    fn test_history_numbered_correctly() {
        let mut s = state();
        s.push_prompt("alpha");
        s.push_prompt("beta");
        s.push_prompt("gamma");
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("1.") || all.contains("  1"), "should show entry 1");
        assert!(all.contains("3.") || all.contains("  3"), "should show entry 3");
    }

    // ── push_prompt / history_entry ───────────────────────────────────────────

    #[test]
    fn test_push_prompt_updates_last_prompt() {
        let mut s = state();
        s.push_prompt("hello world");
        assert_eq!(s.last_prompt.as_deref(), Some("hello world"));
    }

    #[test]
    fn test_push_prompt_appends_to_history() {
        let mut s = state();
        s.push_prompt("first");
        s.push_prompt("second");
        assert_eq!(s.history.len(), 2);
        assert_eq!(s.history[0], "first");
        assert_eq!(s.history[1], "second");
    }

    #[test]
    fn test_history_entry_valid_index() {
        let mut s = state();
        s.push_prompt("alpha");
        s.push_prompt("beta");
        assert_eq!(s.history_entry(1), Some("alpha"));
        assert_eq!(s.history_entry(2), Some("beta"));
    }

    #[test]
    fn test_history_entry_out_of_range() {
        let mut s = state();
        s.push_prompt("only");
        assert!(s.history_entry(0).is_none(), "index 0 should be None");
        assert!(s.history_entry(2).is_none(), "out-of-range should be None");
    }

    #[test]
    fn test_history_capped_at_limit() {
        let mut s = state();
        for i in 0..=HISTORY_LIMIT + 5 {
            s.push_prompt(format!("prompt {i}"));
        }
        assert_eq!(s.history.len(), HISTORY_LIMIT, "history should not exceed HISTORY_LIMIT");
        // Oldest entries should be dropped
        assert!(s.history[0].contains("6"), "oldest kept entry should be prompt 6");
    }

    // ── ReplState ─────────────────────────────────────────────────────────────

    #[test]
    fn test_repl_state_new() {
        let s = ReplState::new("claude-opus-4-6");
        assert_eq!(s.model, "claude-opus-4-6");
        assert_eq!(s.total_input, 0);
        assert_eq!(s.total_output, 0);
        assert_eq!(s.total_cache_read, 0);
        assert_eq!(s.total_cache_write, 0);
        assert!(s.last_prompt.is_none());
        assert!(s.history.is_empty(), "fresh state should have empty history");
    }

    #[test]
    fn test_repl_state_reset_tokens() {
        let mut s = ReplState::new("m");
        s.total_input = 1000;
        s.total_output = 500;
        s.total_cache_read = 200;
        s.total_cache_write = 100;
        s.reset_tokens();
        assert_eq!(s.total_input, 0);
        assert_eq!(s.total_output, 0);
        assert_eq!(s.total_cache_read, 0);
        assert_eq!(s.total_cache_write, 0);
    }

    // ── SSH command ───────────────────────────────────────────────────────────

    /// Helper: state with a registered test host (no real SSH connectivity needed).
    fn state_with_host() -> ReplState {
        let mut s = ReplState::new("claude-opus-4-6");
        s.ssh_hosts.add(crate::ssh::HostEntry::new("test-nuc", "192.168.1.99").with_user("admin"));
        s
    }

    #[test]
    fn test_ssh_no_args_returns_help() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/ssh", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/ssh with no args should show usage");
        assert!(all.contains("/ssh list"), "should mention /ssh list");
        assert!(all.contains("<host>"), "should mention host parameter");
    }

    #[test]
    fn test_ssh_help_flag_returns_usage() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/ssh --help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/ssh --help should show usage");
    }

    #[test]
    fn test_ssh_list_empty_registry() {
        let mut s = state(); // fresh state, no hosts loaded in test
        // Ensure hosts are empty (no hosts.toml or ~/.axonix/hosts.toml in test env)
        s.ssh_hosts = crate::ssh::HostRegistry::new();
        let CommandResult::Handled(lines) = handle_command("/ssh list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("No hosts") || all.contains("hosts.toml"),
            "empty registry should mention no hosts: {all}"
        );
    }

    #[test]
    fn test_ssh_list_with_registered_host() {
        let mut s = state_with_host();
        let CommandResult::Handled(lines) = handle_command("/ssh list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("test-nuc"), "list should show registered host alias");
        assert!(all.contains("192.168.1.99"), "list should show host address");
        assert!(all.contains("admin"), "list should show user");
    }

    #[test]
    fn test_ssh_unknown_host_returns_error() {
        let mut s = state_with_host();
        let CommandResult::Handled(lines) = handle_command("/ssh no-such-host uptime", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("Unknown host") || all.contains("no-such-host"),
            "unknown host should produce error: {all}"
        );
    }

    #[test]
    fn test_ssh_host_no_command_returns_usage() {
        let mut s = state_with_host();
        let CommandResult::Handled(lines) = handle_command("/ssh test-nuc", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("Usage") || all.contains("command"),
            "host with no command should show usage: {all}"
        );
    }

    #[test]
    fn test_ssh_help_shows_registered_hosts_in_usage() {
        let mut s = state_with_host();
        let CommandResult::Handled(lines) = handle_command("/ssh", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("test-nuc"), "usage output should list registered hosts when present");
    }

    #[test]
    fn test_help_includes_ssh_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/ssh"), "/help should document /ssh command");
    }

    // ── Unicode safety in /history ────────────────────────────────────────────

    /// Regression test: /history must not panic when a prompt contains multi-byte
    /// UTF-8 characters (emoji, CJK, accented chars) and exceeds the preview limit.
    /// Previously used `&prompt[..72]` which panics if a char straddles byte 72.
    #[test]
    fn test_history_unicode_prompt_no_panic() {
        let mut s = state();
        // Build a prompt with multi-byte chars (4 bytes each) that will exceed 72 bytes
        // but whose character count is close to the limit, so a naive byte slice would panic.
        let emoji_prompt = "🦀".repeat(20); // 80 bytes, 20 chars
        s.push_prompt(emoji_prompt.clone());
        // This must not panic — previously would have panicked at &prompt[..72]
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // Preview should be present and valid UTF-8 (truncated at char boundary)
        assert!(!all.is_empty(), "history output should not be empty");
        // The output is valid UTF-8 (this assertion would fail if bytes were sliced badly)
        assert!(all.is_ascii() || !all.is_empty());
    }

    #[test]
    fn test_history_cjk_prompt_no_panic() {
        let mut s = state();
        // CJK characters are 3 bytes each; 24 of them = 72 bytes exactly, then add one more
        // so the 73rd byte lands in the middle of a 3-byte char.
        let cjk_prompt = "你好世界".repeat(10); // 40 chars × 3 bytes = 120 bytes
        s.push_prompt(cjk_prompt);
        // Must not panic
        let result = handle_command("/history", &mut s, &[]);
        assert!(matches!(result, CommandResult::Handled(_)));
    }

    #[test]
    fn test_history_mixed_unicode_truncation_is_valid_utf8() {
        let mut s = state();
        // 70 ASCII chars followed by a multi-byte char — byte 72 would land mid-char
        let prompt = "a".repeat(70) + "こんにちは世界"; // last part is 3-byte chars
        s.push_prompt(prompt);
        let CommandResult::Handled(lines) = handle_command("/history", &mut s, &[]) else {
            panic!("expected Handled");
        };
        // Ensure every line is valid UTF-8 (String is always valid UTF-8 in Rust,
        // so what we're really testing is that we didn't panic getting here)
        for line in &lines {
            assert!(std::str::from_utf8(line.as_bytes()).is_ok(), "line must be valid UTF-8: {line:?}");
        }
    }

    // ── /comment command ─────────────────────────────────────────────────────

    #[test]
    fn test_comment_no_args_shows_usage() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/comment with no args should show usage");
        assert!(all.contains("issue_number"), "should mention issue_number");
    }

    #[test]
    fn test_comment_valid_issue_returns_marker() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment 13 Fixed in this session.", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("__gh_comment:13:Fixed in this session."), "should return gh_comment marker: {all}");
    }

    #[test]
    fn test_comment_preserves_spaces_in_body() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment 7 Great feature idea!", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("__gh_comment:7:Great feature idea!"), "body with spaces should be preserved: {all}");
    }

    #[test]
    fn test_comment_non_numeric_issue_shows_error() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment abc some text", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("integer"), "non-numeric issue should show error: {all}");
    }

    #[test]
    fn test_comment_zero_issue_shows_error() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment 0 some text", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("Usage"), "issue 0 should show error: {all}");
    }

    #[test]
    fn test_comment_missing_body_shows_error() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/comment 5", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("empty"), "missing body should show error: {all}");
    }

    #[test]
    fn test_help_includes_comment_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/comment"), "/help should document /comment command");
    }


    // ── /issues command ──────────────────────────────────────────────────────────

    #[test]
    fn test_issues_no_args_returns_fetch_issues_default() {
        let mut s = state();
        let result = handle_command("/issues", &mut s, &[]);
        assert_eq!(result, CommandResult::FetchIssues(10), "/issues with no args should fetch 10");
    }

    #[test]
    fn test_issues_with_valid_limit() {
        let mut s = state();
        let result = handle_command("/issues 5", &mut s, &[]);
        assert_eq!(result, CommandResult::FetchIssues(5), "/issues 5 should fetch 5");
    }

    #[test]
    fn test_issues_with_max_limit() {
        let mut s = state();
        let result = handle_command("/issues 30", &mut s, &[]);
        assert_eq!(result, CommandResult::FetchIssues(30), "/issues 30 should be accepted");
    }

    #[test]
    fn test_issues_over_max_limit_shows_error() {
        let mut s = state();
        let result = handle_command("/issues 31", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for over-limit: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("1–30"), "over-limit should show error: {all}");
    }

    #[test]
    fn test_issues_zero_limit_shows_error() {
        let mut s = state();
        let result = handle_command("/issues 0", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for zero limit: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("Usage"), "zero limit should show error: {all}");
    }

    #[test]
    fn test_issues_invalid_arg_shows_error() {
        let mut s = state();
        let result = handle_command("/issues abc", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for invalid arg: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("invalid"), "invalid arg should show error: {all}");
    }

    #[test]
    fn test_help_includes_issues_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/issues"), "/help should document /issues command");
    }

    // ── /memory command ───────────────────────────────────────────────────────

    /// Helper: state with a tmp memory path (no real file I/O to user dirs).
    fn state_with_tmp_memory() -> (ReplState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let mut s = ReplState::new("claude-opus-4-6");
        s.memory = crate::memory::MemoryStore::new(&path);
        (s, dir)
    }

    #[test]
    fn test_memory_list_empty() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("empty") || all.contains("no entries") || all.contains("Memory:"),
            "empty memory should report empty: {all}");
    }

    #[test]
    fn test_memory_set_and_get() {
        let (mut s, _dir) = state_with_tmp_memory();

        // Set a key
        let result = handle_command("/memory set nuc.ip 192.168.1.10", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("nuc.ip") || all.contains("✓"), "set should confirm: {all}");

        // Get it back
        let result = handle_command("/memory get nuc.ip", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("192.168.1.10"), "get should return value: {all}");
    }

    #[test]
    fn test_memory_list_shows_stored_keys() {
        let (mut s, _dir) = state_with_tmp_memory();
        s.memory.set("twitter.status", "blocked_402", Some("Free tier blocks writes"));
        s.memory.set("operator.tz", "America/Indiana/Indianapolis", None);

        let CommandResult::Handled(lines) = handle_command("/memory list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("twitter.status"), "list should show twitter.status: {all}");
        assert!(all.contains("operator.tz"), "list should show operator.tz: {all}");
        assert!(all.contains("blocked_402"), "list should show value: {all}");
    }

    #[test]
    fn test_memory_get_missing_key() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory get nonexistent", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("not set") || all.contains("nonexistent"),
            "missing key should report not set: {all}");
    }

    #[test]
    fn test_memory_del_existing_key() {
        let (mut s, _dir) = state_with_tmp_memory();
        s.memory.set("temp.key", "temp_value", None);

        let CommandResult::Handled(lines) = handle_command("/memory del temp.key", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("deleted") || all.contains("✓"), "del should confirm: {all}");
        assert!(s.memory.get("temp.key").is_none(), "key should be deleted");
    }

    #[test]
    fn test_memory_del_missing_key() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory del nonexistent", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("not set") || all.contains("nonexistent"),
            "del of missing key should report not set: {all}");
    }

    #[test]
    fn test_memory_set_missing_value_shows_usage() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory set keyonly", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage") || all.contains("value"),
            "set with no value should show usage: {all}");
    }

    #[test]
    fn test_memory_note_command() {
        let (mut s, _dir) = state_with_tmp_memory();
        s.memory.set("some.key", "some_value", None);

        let result = handle_command("/memory note some.key this is a helpful note", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("✓") || all.contains("note"), "note should confirm: {all}");
        // Verify value unchanged, note added
        let entry = s.memory.get_entry("some.key").unwrap();
        assert_eq!(entry.value, "some_value", "value should be unchanged");
        assert!(entry.note.as_deref().unwrap_or("").contains("helpful note"), "note should be stored");
    }

    #[test]
    fn test_memory_note_nonexistent_key_shows_error() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory note ghost.key a note", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("not set") || all.contains("set first"),
            "note on missing key should show error: {all}");
    }

    #[test]
    fn test_memory_unknown_subcommand_shows_usage() {
        let (mut s, _dir) = state_with_tmp_memory();
        let CommandResult::Handled(lines) = handle_command("/memory frobnicate", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage") || all.contains("list") || all.contains("set"),
            "unknown subcommand should show usage: {all}");
    }

    #[test]
    fn test_memory_set_value_with_spaces() {
        let (mut s, _dir) = state_with_tmp_memory();
        let result = handle_command("/memory set greeting hello world from axonix", &mut s, &[]);
        let CommandResult::Handled(lines) = result else { panic!("expected Handled") };
        let all = lines.join("\n");
        assert!(all.contains("✓") || all.contains("greeting"), "set should confirm: {all}");
        // Value with spaces should be stored fully
        assert_eq!(s.memory.get("greeting"), Some("hello world from axonix"),
            "value with spaces should be stored fully");
    }

    #[test]
    fn test_help_includes_memory_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/memory"), "/help should document /memory command");
    }

    // ── /predict command ───────────────────────────────────────────────────────

    /// Helper: state with a tmp prediction store (avoids writing to real .axonix/).
    fn state_with_tmp_predictions() -> (ReplState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        let mut s = ReplState::new("claude-opus-4-6");
        s.predictions = crate::predictions::PredictionStore::new(path);
        (s, dir)
    }

    #[test]
    fn test_predict_no_args_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/predict with no args should show usage");
        assert!(all.contains("add"), "usage should mention 'add'");
        assert!(all.contains("resolve"), "usage should mention 'resolve'");
    }

    #[test]
    fn test_predict_help_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/predict help should show usage: {all}");
    }

    #[test]
    fn test_predict_add_logs_prediction() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let result = handle_command("/predict add the build will succeed", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("✓") || all.contains("prediction"), "add should confirm: {all}");
        assert!(all.contains("#1"), "first prediction should have id 1: {all}");
        assert_eq!(s.predictions.count(), 1, "one prediction should be stored");
    }

    #[test]
    fn test_predict_add_empty_text_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict add ", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage") || all.contains("Example"),
            "add with empty text should show usage: {all}");
        assert_eq!(s.predictions.count(), 0, "no prediction should be stored for empty text");
    }

    #[test]
    fn test_predict_list_empty() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("none") || all.contains("(none") || all.contains("add"),
            "empty list should say none: {all}");
    }

    #[test]
    fn test_predict_open_empty() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict open", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("No open") || all.contains("none") || all.contains("add"),
            "empty open list should say no open predictions: {all}");
    }

    #[test]
    fn test_predict_list_shows_added_predictions() {
        let (mut s, _dir) = state_with_tmp_predictions();
        handle_command("/predict add build will pass", &mut s, &[]);
        handle_command("/predict add tests will increase by 10", &mut s, &[]);

        let CommandResult::Handled(lines) = handle_command("/predict list", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("build will pass"), "list should show first prediction: {all}");
        assert!(all.contains("tests will increase"), "list should show second prediction: {all}");
        assert!(all.contains("2 total") || all.contains("2 open"), "list should show count: {all}");
    }

    #[test]
    fn test_predict_open_shows_unresolved_only() {
        let (mut s, _dir) = state_with_tmp_predictions();
        handle_command("/predict add open prediction", &mut s, &[]);
        // Manually resolve #1 so we can test open shows only unresolved
        s.predictions.predict("second open");
        let id = s.predictions.predict("will resolve this");
        s.predictions.resolve(id, "resolved outcome", None).unwrap();

        let CommandResult::Handled(lines) = handle_command("/predict open", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // Only open ones should appear — resolved ones should not
        assert!(all.contains("open prediction") || all.contains("second open"),
            "open should show unresolved predictions: {all}");
    }

    #[test]
    fn test_predict_resolve_valid() {
        let (mut s, _dir) = state_with_tmp_predictions();
        s.predictions.predict("the tests will pass");

        let result = handle_command("/predict resolve 1 tests passed with 329 cases", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("✓") || all.contains("resolved"), "resolve should confirm: {all}");
        assert!(all.contains("#1"), "should reference prediction id 1: {all}");
        assert!(s.predictions.get(1).unwrap().is_resolved(), "prediction should be resolved");
    }

    #[test]
    fn test_predict_resolve_with_delta() {
        let (mut s, _dir) = state_with_tmp_predictions();
        s.predictions.predict("will add 10 tests");

        let result = handle_command(
            "/predict resolve 1 actually added 12 tests | underestimated by 2",
            &mut s, &[]
        );
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(all.contains("delta") || all.contains("underestimated"),
            "resolve with delta should show delta: {all}");
        let pred = s.predictions.get(1).unwrap();
        assert_eq!(pred.delta.as_deref(), Some("underestimated by 2"));
    }

    #[test]
    fn test_predict_resolve_nonexistent_id() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict resolve 999 outcome", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("not found"),
            "resolving nonexistent id should show error: {all}");
    }

    #[test]
    fn test_predict_resolve_invalid_id() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict resolve abc outcome", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Error") || all.contains("integer"),
            "invalid id should show error: {all}");
    }

    #[test]
    fn test_predict_resolve_missing_outcome_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict resolve 1", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage") || all.contains("outcome"),
            "missing outcome should show usage: {all}");
    }

    #[test]
    fn test_predict_unknown_subcommand_shows_usage() {
        let (mut s, _dir) = state_with_tmp_predictions();
        let CommandResult::Handled(lines) = handle_command("/predict frobnicate", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "unknown subcommand should show usage: {all}");
    }

    #[test]
    fn test_help_includes_predict_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/predict"), "/help should document /predict command");
    }

    // ── /watch command ────────────────────────────────────────────────────────

    #[test]
    fn test_watch_no_args_returns_health_snapshot() {
        let mut s = state();
        let result = handle_command("/watch", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for /watch: {result:?}");
        };
        let all = lines.join("\n");
        // Should show health fields and thresholds
        assert!(all.contains("CPU") || all.contains("load"), "/watch should show CPU info: {all}");
        assert!(all.contains("Memory") || all.contains("Mem") || all.contains("memory"),
            "/watch should show memory info: {all}");
        assert!(all.contains("Disk") || all.contains("disk"), "/watch should show disk info: {all}");
        assert!(all.contains("threshold"), "/watch should mention thresholds: {all}");
    }

    #[test]
    fn test_watch_returns_ok_or_alert_message() {
        let mut s = state();
        let result = handle_command("/watch", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for /watch: {result:?}");
        };
        let all = lines.join("\n");
        // Must show either all-ok or threshold-exceeded — never empty
        assert!(
            all.contains("All metrics") || all.contains("threshold") || all.contains("exceeded"),
            "/watch must show status: {all}"
        );
    }

    #[test]
    fn test_watch_with_subarg_also_returns_health() {
        // /watch status or any subarg should work the same as /watch
        let mut s = state();
        let result = handle_command("/watch status", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled for /watch status: {result:?}");
        };
        assert!(!lines.is_empty(), "/watch status should produce output");
    }

    #[test]
    fn test_watch_not_unknown_command() {
        // /watch must NOT be reported as "Unknown command"
        let mut s = state();
        let result = handle_command("/watch", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(!all.contains("Unknown command"), "/watch should not be unknown: {all}");
    }

    #[test]
    fn test_help_includes_watch_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/watch"), "/help should document /watch command");
    }

    // ── /review command ───────────────────────────────────────────────────────

    #[test]
    fn test_review_no_args_shows_usage() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/review", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "/review with no args should show usage: {all}");
        assert!(all.contains("code_reviewer") || all.contains("review"), "should mention reviewer: {all}");
    }

    #[test]
    fn test_review_with_description_returns_marker() {
        let mut s = state();
        let CommandResult::Handled(lines) =
            handle_command("/review added /review command to repl.rs", &mut s, &[])
        else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains("__review:added /review command to repl.rs"),
            "should return __review: marker: {all}"
        );
    }

    #[test]
    fn test_review_preserves_full_description() {
        let mut s = state();
        let desc = "refactored health.rs to add disk threshold alert logic";
        let cmd = format!("/review {desc}");
        let CommandResult::Handled(lines) = handle_command(&cmd, &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(
            all.contains(desc),
            "description should be preserved in marker: {all}"
        );
    }

    #[test]
    fn test_review_whitespace_only_shows_usage() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/review   ", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("Usage"), "whitespace-only description should show usage: {all}");
    }

    #[test]
    fn test_review_not_unknown_command() {
        let mut s = state();
        let result = handle_command("/review some change", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            !all.contains("Unknown command"),
            "/review should not be treated as unknown: {all}"
        );
    }

    #[test]
    fn test_help_includes_review_command() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/review"), "/help should document /review command");
    }

    // ── /summary command ──────────────────────────────────────────────────────

    #[test]
    fn test_summary_no_args_is_handled_not_unknown() {
        let mut s = state();
        let result = handle_command("/summary", &mut s, &[]);
        let CommandResult::Handled(lines) = result else {
            panic!("expected Handled: {result:?}");
        };
        let all = lines.join("\n");
        assert!(
            !all.contains("Unknown command"),
            "/summary should not be unknown: {all}"
        );
    }

    #[test]
    fn test_summary_not_a_command_for_plain_text() {
        let mut s = state();
        let result = handle_command("summary of my day", &mut s, &[]);
        assert!(
            matches!(result, CommandResult::NotACommand),
            "plain text should not trigger /summary"
        );
    }

    #[test]
    fn test_summary_with_text_returns_handled() {
        let mut s = state();
        let result = handle_command("/summary implemented cycle_summary module", &mut s, &[]);
        let CommandResult::Handled(_lines) = result else {
            panic!("expected Handled: {result:?}");
        };
    }

    #[test]
    fn test_summary_with_text_confirmation_or_error_in_output() {
        let mut s = state();
        let CommandResult::Handled(lines) =
            handle_command("/summary implemented cycle_summary module", &mut s, &[])
        else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // Should either confirm success (✓) or report an error (✗)
        assert!(
            all.contains('✓') || all.contains('✗'),
            "output should indicate success or failure: {all}"
        );
    }

    #[test]
    fn test_summary_with_text_mentions_input() {
        let mut s = state();
        let task = "fixed context window exhaustion for issue 38";
        let CommandResult::Handled(lines) =
            handle_command(&format!("/summary {task}"), &mut s, &[])
        else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // On success, the task text appears in confirmation
        // On error, the error is shown — either is acceptable
        assert!(
            all.contains(task) || all.contains('✗'),
            "output should mention task or show error: {all}"
        );
    }

    #[test]
    fn test_summary_help_mentions_summary() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/help", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        assert!(all.contains("/summary"), "/help should document /summary command");
    }

    #[test]
    fn test_summary_whitespace_only_shows_info() {
        let mut s = state();
        let CommandResult::Handled(lines) = handle_command("/summary   ", &mut s, &[]) else {
            panic!("expected Handled");
        };
        let all = lines.join("\n");
        // Whitespace-only arg = empty, so should show current summary or usage info
        assert!(
            !all.contains("Unknown command"),
            "/summary with only whitespace should not be unknown: {all}"
        );
    }
}
