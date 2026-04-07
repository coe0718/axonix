//! Prompt runner: streaming agent event loop with Telegram forwarding.
//!
//! Extracted from main.rs (G-123) to keep the binary entry point focused.

use std::io::{self, Write};
use yoagent::agent::Agent;
use yoagent::*;
use axonix::render::*;
use axonix::repl::ReplState;
use axonix::telegram::TelegramClient;

pub async fn run_prompt(agent: &mut Agent, input: &str, repl: &mut ReplState, tg: Option<&TelegramClient>) {
    let prompt_start = std::time::Instant::now();
    let mut rx = agent.prompt(input).await;
    let mut last_usage = Usage::default();
    let mut in_text = false;
    let mut in_thinking = false;
    // Collect full text response for Telegram forwarding
    let mut response_text = String::new();

    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::ToolExecutionStart {
                tool_name, args, ..
            } => {
                if in_thinking {
                    println!("{RESET}");
                    in_thinking = false;
                }
                if in_text {
                    println!();
                    in_text = false;
                }
                let summary = match tool_name.as_str() {
                    "bash" => {
                        let cmd = args
                            .get("command")
                            .and_then(|v| v.as_str())
                            .unwrap_or("...");
                        format!("$ {}", truncate(cmd, 80))
                    }
                    "read_file" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("read {}", path)
                    }
                    "write_file" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("write {}", path)
                    }
                    "edit_file" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("edit {}", path)
                    }
                    "list_files" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                        format!("ls {}", path)
                    }
                    "search" => {
                        let pat = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("search '{}'", truncate(pat, 60))
                    }
                    "code_reviewer" => {
                        let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("...");
                        format!("🔍 code review: {}", truncate(task, 60))
                    }
                    "community_responder" => {
                        let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("...");
                        format!("💬 community: {}", truncate(task, 60))
                    }
                    "implementer" => {
                        let task = args.get("task").and_then(|v| v.as_str()).unwrap_or("...");
                        format!("⚙️  implementing: {}", truncate(task, 60))
                    }
                    _ => tool_name.clone(),
                };
                print!("{YELLOW}  ▶ {summary}{RESET}");
                io::stdout().flush().ok();
            }
            AgentEvent::ToolExecutionEnd { is_error, .. } => {
                if is_error {
                    println!(" {RED}✗{RESET}");
                } else {
                    println!(" {GREEN}✓{RESET}");
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Thinking { delta },
                ..
            } => {
                if in_text {
                    println!();
                    in_text = false;
                }
                if !in_thinking {
                    print!("\n{DIM}{MAGENTA}  💭 ");
                    in_thinking = true;
                }
                print!("{DIM}{MAGENTA}{delta}");
                io::stdout().flush().ok();
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Text { delta },
                ..
            } => {
                if in_thinking {
                    println!("{RESET}");
                    in_thinking = false;
                }
                if !in_text {
                    println!();
                    in_text = true;
                }
                response_text.push_str(&delta);
                print!("{}", delta);
                io::stdout().flush().ok();
            }
            AgentEvent::InputRejected { reason } => {
                println!("{RED}  ✗ Input rejected: {reason}{RESET}");
            }
            AgentEvent::ProgressMessage { text, .. } => {
                if in_thinking {
                    println!("{RESET}");
                    in_thinking = false;
                }
                if in_text {
                    println!();
                    in_text = false;
                }
                println!("{DIM}  ℹ {text}{RESET}");
            }
            AgentEvent::TurnEnd { message, .. } => {
                if let AgentMessage::Llm(Message::Assistant {
                    stop_reason: StopReason::Error,
                    error_message,
                    ..
                }) = &message
                {
                    if in_thinking {
                        println!("{RESET}");
                        in_thinking = false;
                    }
                    if in_text {
                        println!();
                        in_text = false;
                    }
                    let err_msg = error_message
                        .as_deref()
                        .unwrap_or("unknown error");
                    println!("{RED}  ✗ API error: {err_msg}{RESET}");
                }
            }
            AgentEvent::AgentEnd { messages } => {
                for msg in &messages {
                    if let AgentMessage::Llm(Message::Assistant { usage, .. }) = msg {
                        last_usage.input += usage.input;
                        last_usage.output += usage.output;
                        last_usage.cache_read += usage.cache_read;
                        last_usage.cache_write += usage.cache_write;
                    }
                }
            }
            _ => {}
        }
    }

    if in_thinking {
        println!("{RESET}");
    }

    if in_text {
        println!();
    }
    repl.total_input += last_usage.input;
    repl.total_output += last_usage.output;
    repl.total_cache_read += last_usage.cache_read;
    repl.total_cache_write += last_usage.cache_write;
    print_usage(&last_usage, prompt_start.elapsed());
    println!();

    // Forward response to Telegram if connected and response is non-empty
    if let Some(tg) = tg {
        let response = response_text.trim();
        if !response.is_empty() {
            let chunks = TelegramClient::format_response(response);
            for chunk in chunks {
                tg.send_message(&chunk).await.ok();
            }
        }
    }
}
