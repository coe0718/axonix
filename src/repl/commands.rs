//! Main `handle_command` dispatch — calls sub-module helpers for complex commands.

use crate::lint::{lint_file, LintResult};
use crate::render::truncate;
use super::types::{CommandResult, ReplState};
use super::memory;
use super::predict;
use super::ssh_cmd;
use super::github_cmds;
use super::help_cmd;
use super::watch_cmd;
use super::misc_cmds;

/// Process a REPL input string. Returns a `CommandResult`.
///
/// This function is pure: it only mutates `state`, produces output lines,
/// and returns a result describing what the loop should do next.
/// No I/O is performed here — the caller renders the output lines.
pub fn handle_command(input: &str, state: &mut ReplState, skill_names: &[String]) -> CommandResult {
    match input {
        "/quit" | "/exit" => CommandResult::Quit,

        "/help" => CommandResult::Handled(help_cmd::help_lines(skill_names)),

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
                match &state.last_prompt {
                    Some(p) => CommandResult::Retry(p.clone()),
                    None => CommandResult::Handled(vec![
                        "  (nothing to retry — no prompts sent yet)".to_string(),
                        String::new(),
                    ]),
                }
            } else {
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
            let path = if s == "/save" {
                "conversation.md".to_string()
            } else {
                let p = s.trim_start_matches("/save ").trim().to_string();
                if p.is_empty() { "conversation.md".to_string() } else { p }
            };
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
            ssh_cmd::handle_ssh(arg, state)
        }

        s if s == "/comment" || s.starts_with("/comment ") => {
            let arg = if s == "/comment" {
                ""
            } else {
                s.trim_start_matches("/comment ").trim()
            };
            github_cmds::handle_comment(arg)
        }

        s if s == "/respond" || s.starts_with("/respond ") => {
            let arg = if s == "/respond" {
                ""
            } else {
                s.trim_start_matches("/respond ").trim()
            };
            github_cmds::handle_respond(arg)
        }

        s if s == "/review" || s.starts_with("/review ") => {
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
            let arg = if s == "/summary" {
                ""
            } else {
                s.trim_start_matches("/summary ").trim()
            };
            misc_cmds::handle_summary(arg)
        }

        "/recap" => misc_cmds::handle_recap(),

        "/failures" => misc_cmds::handle_failures(),

        s if s == "/memory" || s.starts_with("/memory ") => {
            let arg = if s == "/memory" {
                ""
            } else {
                s.trim_start_matches("/memory ").trim()
            };
            memory::handle_memory(arg, state)
        }

        s if s == "/predict" || s.starts_with("/predict ") => {
            let arg = if s == "/predict" {
                ""
            } else {
                s.trim_start_matches("/predict ").trim()
            };
            predict::handle_predict(arg, state)
        }

        s if s == "/issues" || s.starts_with("/issues ") => {
            let arg = if s == "/issues" {
                ""
            } else {
                s.trim_start_matches("/issues ").trim()
            };

            let limit: u8 = if arg.is_empty() {
                10
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

        "/archive-journal" => misc_cmds::handle_archive_journal(),

        "/brief" => misc_cmds::handle_brief(),

        s if s == "/search" || s.starts_with("/search ") => {
            let arg = if s == "/search" {
                ""
            } else {
                s.trim_start_matches("/search ").trim()
            };
            misc_cmds::handle_search(arg)
        }

        s if s == "/memory-search" || s.starts_with("/memory-search ") => {
            let arg = if s == "/memory-search" {
                ""
            } else {
                s.trim_start_matches("/memory-search ").trim()
            };
            if arg.is_empty() {
                CommandResult::Handled(vec![
                    "  Usage: /memory-search <query>".to_string(),
                    "  Example: /memory-search rust borrow checker".to_string(),
                    String::new(),
                ])
            } else {
                CommandResult::MemorySearch(arg.to_string())
            }
        }

        s if s.starts_with('/') => {
            if matches!(s, "/status" | "/context" | "/tokens") {
                CommandResult::NotACommand
            } else if s == "/watch" || s.starts_with("/watch ") {
                watch_cmd::handle_watch()
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
