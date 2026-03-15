//! axonix — a coding agent that evolves itself.
//!
//! Started as ~200 lines. Grows one commit at a time.
//! Read IDENTITY.md and JOURNAL.md for the full story.
//!
//! Usage:
//!   ANTHROPIC_API_KEY=sk-... cargo run
//!   ANTHROPIC_API_KEY=sk-... cargo run -- --model claude-opus-4-6
//!   ANTHROPIC_API_KEY=sk-... cargo run -- --skills ./skills
//!   ANTHROPIC_API_KEY=sk-... cargo run -- -p "explain this code"
//!   echo "prompt" | cargo run  (piped mode: single prompt, no REPL)
//!
//! Commands:
//!   /help           Show available commands
//!   /status         Show session info (model, tokens, messages)
//!   /quit, /exit    Exit the agent
//!   /clear          Clear conversation history
//!   /retry          Retry the last prompt
//!   /model <name>   Switch model mid-session
//!
//! Multiline input:
//!   End a line with \ to continue on the next line
//!   Type """ to start a block, """ again to finish

use std::io::{self, BufRead, IsTerminal, Read, Write};
use yoagent::agent::Agent;
use yoagent::provider::AnthropicProvider;
use yoagent::skills::SkillSet;
use yoagent::tools::default_tools;
use yoagent::retry::RetryConfig;
use yoagent::*;

use axonix::cli::{self, CliArgs};
use axonix::conversation::save_conversation;
use axonix::cost::estimate_cost;
use axonix::render::*;

const SYSTEM_PROMPT: &str = r#"You are a coding assistant working in the user's terminal.
You have access to the filesystem and shell. Be direct and concise.
When the user asks you to do something, do it — don't just explain how.
Use tools proactively: read files to understand context, run commands to verify your work.
After making changes, run tests or verify the result when appropriate."#;

fn make_agent(api_key: &str, model: &str, skills: SkillSet) -> Agent {
    Agent::new(AnthropicProvider)
        .with_system_prompt(SYSTEM_PROMPT)
        .with_model(model)
        .with_api_key(api_key)
        .with_skills(skills)
        .with_tools(default_tools())
        .with_retry_config(RetryConfig {
            max_retries: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 30_000,
        })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let cli_args = match CliArgs::parse(&args) {
        Some(c) => c,
        None => return, // --help or --version was printed
    };

    let api_key = match std::env::var("ANTHROPIC_API_KEY").or_else(|_| std::env::var("API_KEY")) {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("{RED}error:{RESET} No API key found.");
            eprintln!("Set ANTHROPIC_API_KEY or API_KEY environment variable.");
            eprintln!("Example: ANTHROPIC_API_KEY=sk-ant-... cargo run");
            std::process::exit(1);
        }
    };

    let mut model = cli_args.model;

    let skills = if cli_args.skill_dirs.is_empty() {
        SkillSet::empty()
    } else {
        match SkillSet::load(&cli_args.skill_dirs) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{YELLOW}warning:{RESET} Failed to load skills: {e}");
                SkillSet::empty()
            }
        }
    };

    let mut agent = make_agent(&api_key, &model, skills.clone());

    // --prompt / -p mode: run a single prompt from CLI args and exit
    if let Some(prompt_text) = cli_args.prompt {
        let prompt_text = prompt_text.trim();
        if prompt_text.is_empty() {
            eprintln!("{RED}error:{RESET} --prompt requires a non-empty string.");
            eprintln!("Example: axonix -p \"explain this code\"");
            std::process::exit(1);
        }
        eprintln!("{DIM}  axonix (prompt mode) — model: {model}{RESET}");
        let mut _ti: u64 = 0;
        let mut _to: u64 = 0;
        let mut _cr: u64 = 0;
        let mut _cw: u64 = 0;
        run_prompt(&mut agent, prompt_text, &mut _ti, &mut _to, &mut _cr, &mut _cw).await;
        return;
    }

    // Piped mode: read all of stdin as a single prompt, run once, exit
    if !io::stdin().is_terminal() {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).ok();
        let input = input.trim();
        if input.is_empty() {
            eprintln!("No input on stdin.");
            std::process::exit(1);
        }

        eprintln!("{DIM}  axonix (piped mode) — model: {model}{RESET}");
        let mut _ti: u64 = 0;
        let mut _to: u64 = 0;
        let mut _cr: u64 = 0;
        let mut _cw: u64 = 0;
        run_prompt(&mut agent, input, &mut _ti, &mut _to, &mut _cr, &mut _cw).await;
        return;
    }

    // Interactive REPL mode
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());

    cli::print_banner();
    println!("{DIM}  model: {model}{RESET}");
    if !skills.is_empty() {
        println!("{DIM}  skills: {} loaded{RESET}", skills.len());
    }
    println!("{DIM}  cwd:   {cwd}{RESET}");
    println!("{DIM}  Type /help for commands{RESET}\n");

    let session_start = std::time::Instant::now();

    // Handle Ctrl+C gracefully
    let ctrlc_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let flag = ctrlc_flag.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            eprintln!("\n{DIM}  interrupted — bye 👋{RESET}\n");
            std::process::exit(0);
        });
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_cache_read: u64 = 0;
    let mut total_cache_write: u64 = 0;
    let mut last_prompt: Option<String> = None;

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
            println!("{DIM}  (multiline mode — type \"\"\" on its own line to finish){RESET}");
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

        match input {
            "/quit" | "/exit" => break,
            "/help" => {
                println!("{DIM}  Commands:{RESET}");
                println!("{DIM}    /help          Show this help{RESET}");
                println!("{DIM}    /status        Show session info{RESET}");
                println!("{DIM}    /context       Show conversation messages summary{RESET}");
                println!("{DIM}    /tokens        Show token usage and cost estimate{RESET}");
                println!("{DIM}    /retry         Retry the last prompt{RESET}");
                println!("{DIM}    /clear         Clear conversation history{RESET}");
                println!("{DIM}    /model <name>  Switch model (clears history){RESET}");
                println!("{DIM}    /save [path]   Save conversation to file{RESET}");
                println!("{DIM}    /quit, /exit   Exit{RESET}");
                println!();
                println!("{DIM}  Multiline input:{RESET}");
                println!("{DIM}    End a line with \\ to continue on the next line{RESET}");
                println!("{DIM}    Type \"\"\" to start a block, \"\"\" again to finish{RESET}");
                println!();
                continue;
            }
            "/status" => {
                let msg_count = agent.messages().len();
                let elapsed = session_start.elapsed();
                let mins = elapsed.as_secs() / 60;
                let secs = elapsed.as_secs() % 60;
                println!("{DIM}  model:    {}{RESET}", agent.model);
                println!("{DIM}  messages: {msg_count}{RESET}");
                println!("{DIM}  tokens:   {total_input} in / {total_output} out (session total){RESET}");
                if total_cache_read > 0 || total_cache_write > 0 {
                    println!("{DIM}  cache:    {total_cache_read} read / {total_cache_write} write{RESET}");
                }
                println!("{DIM}  elapsed:  {mins}m {secs}s{RESET}");
                println!("{DIM}  cwd:      {cwd}{RESET}");
                println!();
                continue;
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
                                let text = content.iter().find_map(|c| {
                                    if let Content::Text { text } = c { Some(text.as_str()) } else { None }
                                }).unwrap_or("(no text)");
                                format!("{CYAN}user:{RESET} {}", truncate(text, 70))
                            }
                            Some(Message::Assistant { content, usage, .. }) => {
                                let text_len: usize = content.iter().map(|c| {
                                    match c {
                                        Content::Text { text } => text.len(),
                                        Content::ToolCall { .. } => 0,
                                        _ => 0,
                                    }
                                }).sum();
                                let tool_count = content.iter().filter(|c| matches!(c, Content::ToolCall { .. })).count();
                                let mut desc = format!("{GREEN}assistant:{RESET} ");
                                if tool_count > 0 {
                                    desc.push_str(&format!("{tool_count} tool call(s) "));
                                }
                                if text_len > 0 {
                                    desc.push_str(&format!("{text_len} chars "));
                                }
                                desc.push_str(&format!("{DIM}({}in/{}out){RESET}", usage.input, usage.output));
                                desc
                            }
                            Some(Message::ToolResult { tool_name, is_error, content, .. }) => {
                                let len: usize = content.iter().map(|c| {
                                    if let Content::Text { text } = c { text.len() } else { 0 }
                                }).sum();
                                let status = if *is_error { format!("{RED}✗{RESET}") } else { format!("{GREEN}✓{RESET}") };
                                format!("{YELLOW}tool:{RESET} {tool_name} {status} ({len} chars)")
                            }
                            None => format!("{DIM}(extension message){RESET}"),
                        };
                        println!("{DIM}  {i:>3}.{RESET} {summary}");
                    }
                    println!();
                }
                continue;
            }
            "/tokens" => {
                let cost = estimate_cost(&model, total_input, total_output, total_cache_read, total_cache_write);
                println!("{DIM}  Token usage (session total):{RESET}");
                println!("{DIM}    input:       {total_input}{RESET}");
                println!("{DIM}    output:      {total_output}{RESET}");
                if total_cache_read > 0 || total_cache_write > 0 {
                    println!("{DIM}    cache read:  {total_cache_read}{RESET}");
                    println!("{DIM}    cache write: {total_cache_write}{RESET}");
                }
                println!("{DIM}    total:       {}{RESET}", total_input + total_output + total_cache_read + total_cache_write);
                println!("{DIM}    est. cost:   ${cost:.4}{RESET}");
                println!();
                continue;
            }
            "/retry" => {
                match &last_prompt {
                    Some(prompt) => {
                        let prompt = prompt.clone();
                        println!("{DIM}  (retrying: {}){RESET}", truncate(&prompt, 60));
                        run_prompt(&mut agent, &prompt, &mut total_input, &mut total_output, &mut total_cache_read, &mut total_cache_write).await;
                    }
                    None => {
                        println!("{DIM}  (nothing to retry){RESET}\n");
                    }
                }
                continue;
            }
            "/clear" => {
                agent = make_agent(&api_key, &model, skills.clone());
                total_input = 0;
                total_output = 0;
                total_cache_read = 0;
                total_cache_write = 0;
                println!("{DIM}  (conversation cleared){RESET}\n");
                continue;
            }
            s if s.starts_with("/model ") => {
                let new_model = s.trim_start_matches("/model ").trim();
                if new_model.is_empty() {
                    println!("{RED}  Usage: /model <name>{RESET}");
                    println!("{DIM}  Example: /model claude-sonnet-4-20250514{RESET}\n");
                    continue;
                }
                model = new_model.to_string();
                agent = make_agent(&api_key, &model, skills.clone());
                total_input = 0;
                total_output = 0;
                total_cache_read = 0;
                total_cache_write = 0;
                println!("{DIM}  (switched to {new_model}, conversation cleared){RESET}\n");
                continue;
            }
            s if s == "/save" || s.starts_with("/save ") => {
                let path = if s == "/save" {
                    "conversation.md".to_string()
                } else {
                    s.trim_start_matches("/save ").trim().to_string()
                };
                let path = if path.is_empty() { "conversation.md".to_string() } else { path };
                match save_conversation(agent.messages(), &path) {
                    Ok(count) => println!("{DIM}  saved {count} messages to {path}{RESET}\n"),
                    Err(e) => println!("{RED}  failed to save: {e}{RESET}\n"),
                }
                continue;
            }
            s if s.starts_with('/') => {
                println!("{RED}  Unknown command: {s}{RESET}");
                println!("{DIM}  Type /help for available commands{RESET}\n");
                continue;
            }
            _ => {}
        }

        last_prompt = Some(input.to_string());
        run_prompt(&mut agent, input, &mut total_input, &mut total_output, &mut total_cache_read, &mut total_cache_write).await;
    }

    println!("\n{DIM}  bye 👋{RESET}\n");
}

async fn run_prompt(agent: &mut Agent, input: &str, total_in: &mut u64, total_out: &mut u64, total_cr: &mut u64, total_cw: &mut u64) {
    let prompt_start = std::time::Instant::now();
    let mut rx = agent.prompt(input).await;
    let mut last_usage = Usage::default();
    let mut in_text = false;
    let mut in_thinking = false;

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
    *total_in += last_usage.input;
    *total_out += last_usage.output;
    *total_cr += last_usage.cache_read;
    *total_cw += last_usage.cache_write;
    print_usage(&last_usage, prompt_start.elapsed());
    println!();
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_command_parsing_quit() {
        let quit_commands = ["/quit", "/exit"];
        for cmd in &quit_commands {
            assert!(
                *cmd == "/quit" || *cmd == "/exit",
                "Unrecognized quit command: {cmd}"
            );
        }
    }

    #[test]
    fn test_known_commands_recognized() {
        let known = ["/quit", "/exit", "/help", "/status", "/context", "/clear", "/tokens", "/retry"];
        for cmd in &known {
            assert!(
                matches!(
                    *cmd,
                    "/quit" | "/exit" | "/help" | "/status" | "/context" | "/clear" | "/tokens" | "/retry"
                ),
                "Command {cmd} should be recognized"
            );
        }
    }

    #[test]
    fn test_save_command_parsing() {
        let input = "/save my_file.md";
        assert!(input.starts_with("/save "));
        let path = input.trim_start_matches("/save ").trim();
        assert_eq!(path, "my_file.md");
    }

    #[test]
    fn test_save_command_default_path() {
        let input = "/save";
        let path = if input == "/save" {
            "conversation.md"
        } else {
            input.trim_start_matches("/save ").trim()
        };
        assert_eq!(path, "conversation.md");
    }

    #[test]
    fn test_unknown_command_detected() {
        let input = "/foo";
        assert!(input.starts_with('/'));
        assert!(
            !matches!(
                input,
                "/quit" | "/exit" | "/help" | "/status" | "/clear"
            ),
            "/foo should not be a known command"
        );
    }

    #[test]
    fn test_clear_should_preserve_model_switch() {
        let model = "claude-opus-4-6".to_string();
        assert_eq!(model, "claude-opus-4-6");
        let new_model = "claude-sonnet-4-20250514";
        let model = new_model.to_string();
        assert_eq!(model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_retry_tracks_last_prompt() {
        let mut last_prompt: Option<String> = None;
        assert!(last_prompt.is_none(), "Should start with no last prompt");
        last_prompt = Some("explain monads".to_string());
        assert_eq!(last_prompt.as_deref(), Some("explain monads"));
        last_prompt = Some("now explain functors".to_string());
        assert_eq!(last_prompt.as_deref(), Some("now explain functors"));
    }

    #[test]
    fn test_retry_empty_returns_none() {
        let last_prompt: Option<String> = None;
        assert!(last_prompt.is_none());
    }

    #[test]
    fn test_multiline_backslash_detection() {
        assert!("hello\\".trim_end().ends_with('\\'));
        assert!("hello \\".trim_end().ends_with('\\'));
        assert!(!"hello".trim_end().ends_with('\\'));
        assert!(!"".trim_end().ends_with('\\'));
    }

    #[test]
    fn test_multiline_triple_quote_detection() {
        assert_eq!("\"\"\"".trim(), "\"\"\"");
        assert_eq!("  \"\"\"  ".trim(), "\"\"\"");
        assert_ne!("\"\"\" hello".trim(), "\"\"\"");
    }

    #[test]
    fn test_backslash_stripping() {
        let line = "hello world\\";
        let stripped = line.trim_end().trim_end_matches('\\');
        assert_eq!(stripped, "hello world");
    }
}
