//! Main `handle_command` dispatch — calls sub-module helpers for complex commands.

use crate::lint::{lint_file, LintResult};
use crate::render::truncate;
use super::types::{CommandResult, ReplState};
use super::memory;
use super::predict;
use super::ssh_cmd;
use super::github_cmds;

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
                "    /memory recent      Show 5 most recent semantic memories".to_string(),
                "    /predict add <text> Log a prediction about a future outcome".to_string(),
                "    /predict open       Show open (unresolved) predictions".to_string(),
                "    /predict list       Show all predictions with outcomes".to_string(),
                "    /watch             Show current health vs thresholds".to_string(),
                "    /review <desc>     Invoke code_reviewer sub-agent on recent changes".to_string(),
                "    /summary [text]    Show or update cycle summary (persisted to next session)".to_string(),
                "    /recap             Post session recap thread to Bluesky (title, commits, tests)".to_string(),
                "    /failures          Show logged failure patterns across sessions".to_string(),
                "    /archive-journal   Archive old journal entries to docs/archive/JOURNAL.md".to_string(),
                "    /memory-search <q>  Search stored observations by keyword".to_string(),
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

            use crate::cycle_summary::CycleSummary;
            let mut cs = CycleSummary::default_path();

            if arg.is_empty() {
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
            CommandResult::Handled(vec!["__recap".to_string()])
        }

        "/failures" => {
            use crate::failure_patterns::FailurePatternStore;
            let mut store = FailurePatternStore::default_path();
            let summary = store.failure_summary();
            let mut lines: Vec<String> = summary.lines().map(|l| format!("  {l}")).collect();
            // G-118: check for threshold-3 breaches and surface them inline.
            let newly_breached = store.types_newly_at_threshold(3);
            if !newly_breached.is_empty() {
                lines.push(String::new());
                lines.push("  ⚠️  Threshold alert (G-118): the following failure types have 3+ occurrences:".to_string());
                for label in &newly_breached {
                    lines.push(format!("     • {label}"));
                }
                lines.push("  Consider reviewing and addressing these patterns.".to_string());
            }
            lines.push(String::new());
            CommandResult::Handled(lines)
        }

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

        "/archive-journal" => CommandResult::ArchiveJournal,

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
                let snap = crate::health::HealthSnapshot::collect();
                let config = crate::watch::WatchConfig::default();
                let watch_state = crate::watch::AlertState::default_for_repl();
                let alerts = crate::watch::evaluate_thresholds(&snap, &config, &watch_state);

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
