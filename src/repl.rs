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
                    let preview = if prompt.len() > 72 {
                        format!("{}…", &prompt[..72])
                    } else {
                        prompt.clone()
                    };
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

        s if s.starts_with('/') => {
            // /status, /context, /tokens are handled by main (they need agent/session data)
            // Everything else is truly unknown
            if matches!(s, "/status" | "/context" | "/tokens") {
                CommandResult::NotACommand // caller handles these
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
}
