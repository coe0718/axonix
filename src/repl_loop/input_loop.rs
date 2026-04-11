//! The main REPL input loop: reads stdin, dispatches commands, calls the agent.

use std::io::{self, BufRead, Write};

use axonix::bluesky::BlueskyClient;
use axonix::github::GitHubClient;
use axonix::render::*;
use axonix::repl::{handle_command, CommandResult, ReplState};
use axonix::telegram::TelegramClient;
use tokio::sync::mpsc;
use yoagent::skills::SkillSet;

use crate::agent_setup::make_agent;
use crate::prompt_runner::run_prompt;

use super::cmd_dispatch::{handle_handled_result, handle_not_a_command_inline};
use super::tg_drain::drain_telegram_commands;

/// Run the main REPL input loop until stdin closes or the user quits.
///
/// Reads lines from stdin (with multiline continuation support), dispatches
/// through `handle_command()`, and handles agent prompts. After each turn,
/// drains any queued Telegram bot commands.
#[allow(clippy::too_many_arguments)]
pub async fn run_input_loop(
    mut agent: yoagent::Agent,
    api_key: &str,
    _model_initial: &str,
    skills: SkillSet,
    system_prompt: &str,
    tg: Option<&TelegramClient>,
    gh: Option<&GitHubClient>,
    bsky: Option<&BlueskyClient>,
    mut repl: ReplState,
    mut tg_rx: Option<mpsc::Receiver<axonix::telegram::BotCommand>>,
    session_start: std::time::Instant,
    skill_names: &[String],
    cwd: &str,
) {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        print!("{BOLD}{GREEN}> {RESET}");
        io::stdout().flush().ok();

        let line = match lines.next() {
            Some(Ok(l)) => l,
            _ => break,
        };

        // Multiline input: backslash continuation or triple-quote blocks
        let input = if line.trim_end().ends_with('\\') {
            let mut buf = String::from(line.trim_end().trim_end_matches('\\'));
            buf.push('\n');
            loop {
                print!("{DIM}. {RESET}");
                io::stdout().flush().ok();
                match lines.next() {
                    Some(Ok(next)) => {
                        if next.trim_end().ends_with('\\') {
                            buf.push_str(next.trim_end().trim_end_matches('\\'));
                            buf.push('\n');
                        } else {
                            buf.push_str(&next);
                            break;
                        }
                    }
                    _ => break,
                }
            }
            buf
        } else if line.trim() == "\"\"\"" {
            let mut buf = String::new();
            println!("{DIM}  (multiline mode — type \"\"\"  on its own line to finish){RESET}");
            loop {
                print!("{DIM}. {RESET}");
                io::stdout().flush().ok();
                match lines.next() {
                    Some(Ok(next)) => {
                        if next.trim() == "\"\"\"" {
                            break;
                        }
                        if !buf.is_empty() {
                            buf.push('\n');
                        }
                        buf.push_str(&next);
                    }
                    _ => break,
                }
            }
            buf
        } else {
            line
        };

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // Dispatch through handle_command first, then handle agent-data commands inline
        let cmd_result = handle_command(input, &mut repl, skill_names);
        match cmd_result {
            CommandResult::Quit => break,

            CommandResult::Clear => {
                agent = make_agent(api_key, &repl.model, skills.clone(), system_prompt);
                repl.reset_tokens();
                println!("{DIM}  (conversation cleared){RESET}\n");
                continue;
            }

            CommandResult::SwitchModel(ref new_model) => {
                agent = make_agent(api_key, new_model, skills.clone(), system_prompt);
                println!("{DIM}  (switched to {new_model}, conversation cleared){RESET}\n");
                continue;
            }

            CommandResult::Retry(ref prompt) => {
                println!("{DIM}  (retrying: {}){RESET}", truncate(prompt, 60));
                let prompt = prompt.clone();
                run_prompt(&mut agent, &prompt, &mut repl, tg).await;
                continue;
            }

            CommandResult::FetchIssues(limit) => {
                match &gh {
                    None => {
                        println!("{YELLOW}  ⚠ No GitHub token available — set GH_TOKEN or AXONIX_BOT_TOKEN{RESET}\n");
                    }
                    Some(gh_client) => {
                        print!("{DIM}  fetching open issues for coe0718/axonix...{RESET}");
                        io::stdout().flush().ok();
                        match gh_client.list_issues("coe0718/axonix", limit).await {
                            Err(e) => {
                                println!("\n{RED}  ✗ failed to fetch issues: {e}{RESET}\n")
                            }
                            Ok(issues) => {
                                println!();
                                if issues.is_empty() {
                                    println!("{DIM}  (no open issues){RESET}\n");
                                } else {
                                    println!(
                                        "{DIM}  Open issues ({} shown, sorted by 👍):{RESET}",
                                        issues.len()
                                    );
                                    for issue in &issues {
                                        let label_str = if issue.labels.is_empty() {
                                            String::new()
                                        } else {
                                            format!(" [{}]", issue.labels.join(", "))
                                        };
                                        let reaction_str = if issue.reactions > 0 {
                                            format!(" 👍{}", issue.reactions)
                                        } else {
                                            String::new()
                                        };
                                        println!(
                                            "{DIM}  #{:<4}{RESET} {}{reaction_str}{YELLOW}{label_str}{RESET}",
                                            issue.number,
                                            truncate(&issue.title, 70)
                                        );
                                    }
                                    println!();
                                }
                            }
                        }
                    }
                }
                continue;
            }

            CommandResult::ArchiveJournal => {
                let archiver = axonix::journal_archive::JournalArchiver::default();
                match archiver.archive() {
                    Ok(result) if result.moved == 0 => {
                        println!(
                            "{DIM}  (journal has {} or fewer recent entries — no archiving needed){RESET}\n",
                            result.kept
                        );
                    }
                    Ok(result) => {
                        println!(
                            "{GREEN}  ✓ archived {} entries to {} ({} kept in JOURNAL.md){RESET}\n",
                            result.moved, result.archive_path, result.kept
                        );
                    }
                    Err(e) => {
                        println!("{RED}  ✗ journal archive failed: {e}{RESET}\n");
                    }
                }
                continue;
            }

            CommandResult::MemorySearch(ref query) => {
                let sep = "─".repeat(45);
                match axonix::db::AxonixDb::open_default() {
                    Err(e) => {
                        println!("{RED}  ✗ DB error: {e}{RESET}\n");
                    }
                    Ok(db) => match db.search_memory(query, 20) {
                        Err(e) => {
                            println!("{RED}  ✗ search error: {e}{RESET}\n");
                        }
                        Ok(results) if results.is_empty() => {
                            println!("  No observations match \"{query}\".\n");
                        }
                        Ok(results) => {
                            println!("  Memory search: \"{query}\"");
                            println!("  {sep}");
                            for row in &results {
                                let preview: String = row.text.chars().take(200).collect();
                                let date = &row.created_at[..10.min(row.created_at.len())];
                                println!("  [{}] — score: {}", row.key, row.score);
                                println!("    {preview}");
                                if !row.tags.is_empty() {
                                    println!("    tags: {}", row.tags);
                                }
                                println!("    created: {date}");
                                println!("  {sep}");
                            }
                            println!(
                                "  ({} result{})\n",
                                results.len(),
                                if results.len() == 1 { "" } else { "s" }
                            );
                        }
                    },
                }
                continue;
            }

            CommandResult::ShowRecentMemories => {
                let sep = "─".repeat(55);
                match axonix::db::AxonixDb::open_default() {
                    Err(e) => {
                        println!("{RED}  ✗ DB error: {e}{RESET}\n");
                    }
                    Ok(db) => match db.hot_memories_list(5) {
                        Err(e) => {
                            println!("{RED}  ✗ memory error: {e}{RESET}\n");
                        }
                        Ok(rows) if rows.is_empty() => {
                            println!("  No semantic memories stored yet.\n");
                        }
                        Ok(rows) => {
                            println!("  Recent semantic memories ({})", rows.len());
                            println!("  {sep}");
                            for row in &rows {
                                let date = &row.created_at[..10.min(row.created_at.len())];
                                let preview: String = row.content.chars().take(200).collect();
                                let topics = if row.topics.is_empty() || row.topics == "[]" {
                                    String::new()
                                } else {
                                    format!("  topics: {}", row.topics)
                                };
                                println!("  [{}] importance: {:.2}", date, row.importance);
                                println!("    {preview}");
                                if !topics.is_empty() {
                                    println!("{topics}");
                                }
                                println!("  {sep}");
                            }
                            println!(
                                "  ({} entr{})\n",
                                rows.len(),
                                if rows.len() == 1 { "y" } else { "ies" }
                            );
                        }
                    },
                }
                continue;
            }

            CommandResult::Handled(ref output_lines) => {
                handle_handled_result(
                    output_lines,
                    &mut agent,
                    &mut repl,
                    tg,
                    gh,
                    bsky,
                )
                .await;
                continue;
            }

            CommandResult::NotACommand => {
                if handle_not_a_command_inline(input, &agent, &repl, session_start, cwd) {
                    continue;
                }
                // Fall through to agent prompt
                repl.push_prompt(input);
                run_prompt(&mut agent, input, &mut repl, tg).await;
            }
        }

        // After each main-loop turn, drain any queued Telegram bot commands
        drain_telegram_commands(&mut tg_rx, &mut agent, &mut repl, tg, session_start).await;
    }

    println!("\n{DIM}  ⚡ AXONIX OFFLINE — shutting down subsystems... bye 👋{RESET}\n");
}
