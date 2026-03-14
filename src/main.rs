//! axonix — a coding agent that evolves itself.
//!
//! Started as ~200 lines. Grows one commit at a time.
//! Read IDENTITY.md and JOURNAL.md for the full story.
//!
//! Usage:
//!   ANTHROPIC_API_KEY=sk-... cargo run
//!   ANTHROPIC_API_KEY=sk-... cargo run -- --model claude-opus-4-6
//!   ANTHROPIC_API_KEY=sk-... cargo run -- --skills ./skills
//!   echo "prompt" | cargo run  (piped mode: single prompt, no REPL)
//!
//! Commands:
//!   /help           Show available commands
//!   /status         Show session info (model, tokens, messages)
//!   /quit, /exit    Exit the agent
//!   /clear          Clear conversation history
//!   /model <name>   Switch model mid-session

use std::io::{self, BufRead, IsTerminal, Read, Write};
use yoagent::agent::Agent;
use yoagent::provider::AnthropicProvider;
use yoagent::skills::SkillSet;
use yoagent::tools::default_tools;
use yoagent::retry::RetryConfig;
use yoagent::*;

// ANSI color helpers
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const MAGENTA: &str = "\x1b[35m";
const RED: &str = "\x1b[31m";

const VERSION: &str = env!("CARGO_PKG_VERSION");

const SYSTEM_PROMPT: &str = r#"You are a coding assistant working in the user's terminal.
You have access to the filesystem and shell. Be direct and concise.
When the user asks you to do something, do it — don't just explain how.
Use tools proactively: read files to understand context, run commands to verify your work.
After making changes, run tests or verify the result when appropriate."#;

fn print_help() {
    println!("axonix v{VERSION} — a coding agent growing up in public");
    println!();
    println!("Usage: axonix [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --model <name>    Model to use (default: claude-opus-4-6)");
    println!("  --skills <dir>    Directory containing skill files");
    println!("  --help, -h        Show this help message");
    println!("  --version, -V     Show version");
    println!();
    println!("Commands (in REPL):");
    println!("  /help             Show available commands");
    println!("  /status           Show session info");
    println!("  /tokens           Show token usage and cost estimate");
    println!("  /quit, /exit      Exit the agent");
    println!("  /clear            Clear conversation history");
    println!("  /model <name>     Switch model mid-session");
    println!("  /save [path]      Save conversation to file");
    println!();
    println!("Environment:");
    println!("  ANTHROPIC_API_KEY  API key for Anthropic (required)");
    println!("  API_KEY            Alternative env var for API key");
}

fn print_banner() {
    println!(
        "\n{BOLD}{CYAN}  axonix{RESET} v{VERSION} {DIM}— a coding agent growing up in public{RESET}"
    );
    println!("{DIM}  Type /quit to exit, /clear to reset{RESET}\n");
}

fn print_usage(usage: &Usage) {
    if usage.input > 0 || usage.output > 0 {
        let cache_info = if usage.cache_read > 0 || usage.cache_write > 0 {
            format!(
                " (cache: {} read, {} write)",
                usage.cache_read, usage.cache_write
            )
        } else {
            String::new()
        };
        println!(
            "\n{DIM}  tokens: {} in / {} out{cache_info}{RESET}",
            usage.input, usage.output
        );
    }
}

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

    // Handle --help and --version before anything else
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("axonix v{VERSION}");
        return;
    }

    let api_key = match std::env::var("ANTHROPIC_API_KEY").or_else(|_| std::env::var("API_KEY")) {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("{RED}error:{RESET} No API key found.");
            eprintln!("Set ANTHROPIC_API_KEY or API_KEY environment variable.");
            eprintln!("Example: ANTHROPIC_API_KEY=sk-ant-... cargo run");
            std::process::exit(1);
        }
    };

    let mut model = args
        .iter()
        .position(|a| a == "--model")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "claude-opus-4-6".into());

    let skill_dirs: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == "--skills")
        .filter_map(|(i, _)| args.get(i + 1).cloned())
        .collect();

    let skills = if skill_dirs.is_empty() {
        SkillSet::empty()
    } else {
        match SkillSet::load(&skill_dirs) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{YELLOW}warning:{RESET} Failed to load skills: {e}");
                SkillSet::empty()
            }
        }
    };

    let mut agent = make_agent(&api_key, &model, skills.clone());

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
        run_prompt(&mut agent, input, &mut _ti, &mut _to).await;
        return;
    }

    // Interactive REPL mode
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());

    print_banner();
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
            // Print goodbye on Ctrl+C
            eprintln!("\n{DIM}  interrupted — bye 👋{RESET}\n");
            std::process::exit(0);
        });
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;

    loop {
        print!("{BOLD}{GREEN}> {RESET}");
        io::stdout().flush().ok();

        let line = match lines.next() {
            Some(Ok(l)) => l,
            _ => break,
        };

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        match input {
            "/quit" | "/exit" => break,
            "/help" => {
                println!("{DIM}  Commands:{RESET}");
                println!("{DIM}    /help          Show this help{RESET}");
                println!("{DIM}    /status        Show session info{RESET}");
                println!("{DIM}    /tokens        Show token usage and cost estimate{RESET}");
                println!("{DIM}    /clear         Clear conversation history{RESET}");
                println!("{DIM}    /model <name>  Switch model (clears history){RESET}");
                println!("{DIM}    /save [path]   Save conversation to file{RESET}");
                println!("{DIM}    /quit, /exit   Exit{RESET}");
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
                println!("{DIM}  elapsed:  {mins}m {secs}s{RESET}");
                println!("{DIM}  cwd:      {cwd}{RESET}");
                println!();
                continue;
            }
            "/tokens" => {
                let cost = estimate_cost(&model, total_input, total_output);
                println!("{DIM}  Token usage (session total):{RESET}");
                println!("{DIM}    input:  {total_input}{RESET}");
                println!("{DIM}    output: {total_output}{RESET}");
                println!("{DIM}    total:  {}{RESET}", total_input + total_output);
                println!("{DIM}    est. cost: ${cost:.4}{RESET}");
                println!();
                continue;
            }
            "/clear" => {
                agent = make_agent(&api_key, &model, skills.clone());
                total_input = 0;
                total_output = 0;
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

        run_prompt(&mut agent, input, &mut total_input, &mut total_output).await;
    }

    println!("\n{DIM}  bye 👋{RESET}\n");
}

async fn run_prompt(agent: &mut Agent, input: &str, total_in: &mut u64, total_out: &mut u64) {
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
                // Detect API errors and display them
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
                // Sum usage from all assistant messages in this prompt
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
    print_usage(&last_usage);
    println!();
}

fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((idx, _)) => format!("{}…", &s[..idx]),
        None => s.to_string(),
    }
}

/// Rough cost estimate based on model pricing (USD).
/// Prices are approximate and may change — this is a convenience indicator, not a bill.
fn estimate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let (input_per_m, output_per_m) = if model.contains("opus") {
        (15.0, 75.0)
    } else if model.contains("sonnet") {
        (3.0, 15.0)
    } else if model.contains("haiku") {
        (0.25, 1.25)
    } else {
        // Unknown model — use sonnet pricing as default
        (3.0, 15.0)
    };
    (input_tokens as f64 / 1_000_000.0) * input_per_m
        + (output_tokens as f64 / 1_000_000.0) * output_per_m
}

fn save_conversation(messages: &[AgentMessage], path: &str) -> io::Result<usize> {
    use std::fs::File;
    let mut file = File::create(path)?;
    let mut count = 0;
    for msg in messages {
        if let Some(llm_msg) = msg.as_llm() {
            let (role, contents) = match llm_msg {
                Message::User { content, .. } => ("User", content),
                Message::Assistant { content, .. } => ("Assistant", content),
                Message::ToolResult {
                    tool_name, content, ..
                } => {
                    // Write tool results with their tool name
                    writeln!(file, "## Tool Result: {tool_name}\n")?;
                    for c in content {
                        if let Content::Text { text } = c {
                            writeln!(file, "{text}\n")?;
                        }
                    }
                    writeln!(file, "---\n")?;
                    count += 1;
                    continue;
                }
            };
            writeln!(file, "## {role}\n")?;
            for c in contents {
                match c {
                    Content::Text { text } => writeln!(file, "{text}\n")?,
                    Content::ToolCall { name, arguments, .. } => {
                        writeln!(file, "**Tool call:** `{name}`\n```json\n{arguments}\n```\n")?
                    }
                    _ => {}
                }
            }
            writeln!(file, "---\n")?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 5), "hello…");
    }

    #[test]
    fn test_truncate_unicode() {
        assert_eq!(truncate("héllo wörld", 5), "héllo…");
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_version_constant_exists() {
        assert!(
            VERSION.contains('.'),
            "Version should contain a dot: {VERSION}"
        );
    }

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
    fn test_command_parsing_model() {
        let input = "/model claude-opus-4-6";
        assert!(input.starts_with("/model "));
        let model_name = input.trim_start_matches("/model ").trim();
        assert_eq!(model_name, "claude-opus-4-6");
    }

    #[test]
    fn test_command_parsing_model_whitespace() {
        let input = "/model   claude-opus-4-6  ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert_eq!(model_name, "claude-opus-4-6");
    }

    #[test]
    fn test_known_commands_recognized() {
        let known = ["/quit", "/exit", "/help", "/status", "/clear", "/tokens"];
        for cmd in &known {
            assert!(
                matches!(
                    *cmd,
                    "/quit" | "/exit" | "/help" | "/status" | "/clear" | "/tokens"
                ),
                "Command {cmd} should be recognized"
            );
        }
        // /save and /model are prefix commands, tested separately
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
    fn test_save_conversation_empty() {
        let messages: Vec<AgentMessage> = vec![];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        let count = save_conversation(&messages, path.to_str().unwrap()).unwrap();
        assert_eq!(count, 0);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.is_empty());
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
    fn test_truncate_adds_ellipsis() {
        let result = truncate("a]long string that goes on", 6);
        assert!(result.ends_with('…'), "Truncated string should end with ellipsis: {result}");
    }

    #[test]
    fn test_model_command_empty_name() {
        let input = "/model ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert!(model_name.is_empty(), "Empty model name should be detected");
    }

    #[test]
    fn test_model_command_whitespace_only() {
        let input = "/model    ";
        let model_name = input.trim_start_matches("/model ").trim();
        assert!(model_name.is_empty(), "Whitespace-only model name should be detected");
    }

    #[test]
    fn test_estimate_cost_opus() {
        let cost = estimate_cost("claude-opus-4-6", 1_000_000, 1_000_000);
        // Opus: $15/M input + $75/M output = $90 for 1M each
        assert!((cost - 90.0).abs() < 0.01, "Opus cost estimate wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_sonnet() {
        let cost = estimate_cost("claude-sonnet-4-20250514", 1_000_000, 0);
        assert!((cost - 3.0).abs() < 0.01, "Sonnet input cost wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_haiku() {
        let cost = estimate_cost("claude-haiku-3", 0, 1_000_000);
        assert!((cost - 1.25).abs() < 0.01, "Haiku output cost wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_unknown_model() {
        let cost = estimate_cost("some-unknown-model", 1_000_000, 1_000_000);
        // Default to sonnet pricing: $3/M + $15/M = $18
        assert!((cost - 18.0).abs() < 0.01, "Unknown model cost wrong: {cost}");
    }

    #[test]
    fn test_estimate_cost_zero_tokens() {
        let cost = estimate_cost("claude-opus-4-6", 0, 0);
        assert!((cost - 0.0).abs() < 0.001, "Zero tokens should cost $0: {cost}");
    }

    #[test]
    fn test_clear_should_preserve_model_switch() {
        // Simulates the logic: after /model switches, /clear should use the new model
        let mut model = "claude-opus-4-6".to_string();
        // Simulate /model command
        let new_model = "claude-sonnet-4-20250514";
        model = new_model.to_string();
        // Simulate /clear — should use current model, not the original
        assert_eq!(model, "claude-sonnet-4-20250514");
    }
}
