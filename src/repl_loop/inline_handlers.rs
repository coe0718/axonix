//! Inline command handlers for `/status`, `/context`, and `/tokens`.
//!
//! These handlers need live session state (agent messages, elapsed time, cwd)
//! and are invoked when `handle_command()` returns `CommandResult::NotACommand`
//! but the input matches one of the built-in slash commands.

use axonix::cost::estimate_cost;
use axonix::render::*;
use axonix::repl::ReplState;
use yoagent::*;

/// Handle a `CommandResult::NotACommand` for special inline commands that need
/// live session state (agent messages, session elapsed time, cwd).
///
/// Returns `true` if a built-in command was handled (caller should `continue`),
/// `false` if the input should be forwarded to the AI agent.
pub fn handle_not_a_command_inline(
    input: &str,
    agent: &yoagent::Agent,
    repl: &ReplState,
    session_start: std::time::Instant,
    cwd: &str,
) -> bool {
    match input {
        "/status" => {
            let msg_count = agent.messages().len();
            let elapsed = session_start.elapsed();
            let mins = elapsed.as_secs() / 60;
            let secs = elapsed.as_secs() % 60;
            println!("{DIM}  model:    {}{RESET}", agent.model);
            println!("{DIM}  messages: {msg_count}{RESET}");
            println!(
                "{DIM}  tokens:   {} in / {} out (session total){RESET}",
                repl.total_input, repl.total_output
            );
            if repl.total_cache_read > 0 || repl.total_cache_write > 0 {
                println!(
                    "{DIM}  cache:    {} read / {} write{RESET}",
                    repl.total_cache_read, repl.total_cache_write
                );
            }
            println!("{DIM}  elapsed:  {mins}m {secs}s{RESET}");
            println!("{DIM}  cwd:      {cwd}{RESET}");
            println!();
            true
        }
        "/context" => {
            let messages = agent.messages();
            if messages.is_empty() {
                println!("{DIM}  (no messages in context){RESET}\n");
            } else {
                println!("{DIM}  Context ({} messages):{RESET}", messages.len());
                for (i, msg) in messages.iter().enumerate() {
                    let summary = match msg.as_llm() {
                        Some(Message::User { content, .. }) => {
                            let text = content
                                .iter()
                                .find_map(|c| {
                                    if let Content::Text { text } = c {
                                        Some(text.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or("(no text)");
                            format!("{CYAN}user:{RESET} {}", truncate(text, 70))
                        }
                        Some(Message::Assistant { content, usage, .. }) => {
                            let text_len: usize = content
                                .iter()
                                .map(|c| match c {
                                    Content::Text { text } => text.len(),
                                    Content::ToolCall { .. } => 0,
                                    _ => 0,
                                })
                                .sum();
                            let tool_count = content
                                .iter()
                                .filter(|c| matches!(c, Content::ToolCall { .. }))
                                .count();
                            let mut desc = format!("{GREEN}assistant:{RESET} ");
                            if tool_count > 0 {
                                desc.push_str(&format!("{tool_count} tool call(s) "));
                            }
                            if text_len > 0 {
                                desc.push_str(&format!("{text_len} chars "));
                            }
                            desc.push_str(&format!(
                                "{DIM}({}in/{}out){RESET}",
                                usage.input, usage.output
                            ));
                            desc
                        }
                        Some(Message::ToolResult {
                            tool_name,
                            is_error,
                            content,
                            ..
                        }) => {
                            let len: usize = content
                                .iter()
                                .map(|c| {
                                    if let Content::Text { text } = c {
                                        text.len()
                                    } else {
                                        0
                                    }
                                })
                                .sum();
                            let status = if *is_error {
                                format!("{RED}✗{RESET}")
                            } else {
                                format!("{GREEN}✓{RESET}")
                            };
                            format!("{YELLOW}tool:{RESET} {tool_name} {status} ({len} chars)")
                        }
                        None => format!("{DIM}(extension message){RESET}"),
                    };
                    println!("{DIM}  {i:>3}.{RESET} {summary}");
                }
                println!();
            }
            true
        }
        "/tokens" => {
            let cost = estimate_cost(
                &repl.model,
                repl.total_input,
                repl.total_output,
                repl.total_cache_read,
                repl.total_cache_write,
            );
            println!("{DIM}  Token usage (session total):{RESET}");
            println!("{DIM}    input:       {}{RESET}", repl.total_input);
            println!("{DIM}    output:      {}{RESET}", repl.total_output);
            if repl.total_cache_read > 0 || repl.total_cache_write > 0 {
                println!("{DIM}    cache read:  {}{RESET}", repl.total_cache_read);
                println!("{DIM}    cache write: {}{RESET}", repl.total_cache_write);
            }
            println!(
                "{DIM}    total:       {}{RESET}",
                repl.total_input
                    + repl.total_output
                    + repl.total_cache_read
                    + repl.total_cache_write
            );
            println!("{DIM}    est. cost:   ${cost:.4}{RESET}");
            println!();
            true
        }
        _ => false,
    }
}
