//! Interactive REPL loop for axonix.
//!
//! Extracted from main.rs (G-123) to keep the binary entry point focused.
//! Owns the full interactive session: banner, input loop, command dispatch, AI calls.

use std::io::{self, BufRead, Write};

use yoagent::skills::SkillSet;
use yoagent::*;

use axonix::bluesky::BlueskyClient;
use axonix::cli;
use axonix::conversation::save_conversation;
use axonix::cost::estimate_cost;
use axonix::github::GitHubClient;
use axonix::render::*;
use axonix::repl::{handle_command, CommandResult, ReplState};
use axonix::telegram::TelegramClient;

use crate::agent_setup::make_agent;
use crate::prompt_runner::run_prompt;
use crate::session_helpers::{read_journal_title, get_recent_commits, get_test_count};

/// Context needed to run the interactive REPL.
pub struct ReplContext<'a> {
    pub api_key: &'a str,
    pub model: String,
    pub skills: SkillSet,
    pub system_prompt: String,
    pub tg: Option<TelegramClient>,
    pub gh: Option<GitHubClient>,
    pub bsky: Option<BlueskyClient>,
}

/// Run the interactive REPL loop. Returns when the user quits or stdin closes.
pub async fn run_repl_loop(mut agent: yoagent::Agent, ctx: ReplContext<'_>) {
    let ReplContext { api_key, model, skills, system_prompt, tg, gh, bsky } = ctx;

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());

    // Initialize REPL state early so memory is available for banner display.
    // (Memory is loaded from .axonix/memory.json at this point.)
    let mut repl = ReplState::new(&model);

    cli::print_banner();
    println!("{DIM}  model: {model}{RESET}");
    let skill_names: Vec<String> = if skills.is_empty() {
        vec![]
    } else {
        println!("{DIM}  skills: {} loaded{RESET}", skills.len());
        skills.skills().iter().map(|s| s.name.clone()).collect()
    };
    println!("{DIM}  cwd:   {cwd}{RESET}");
    // Show memory count if any facts are stored
    if !repl.memory.is_empty() {
        println!("{DIM}  memory: {} facts loaded — /memory list to view{RESET}", repl.memory.len());
    }
    if tg.is_some() {
        println!("{DIM}  telegram: connected — send /ask <prompt> to chat with me{RESET}");
    }
    if let Some(ref gh_client) = gh {
        println!("{DIM}  github:   {} — use /comment <n> <text> to post issue comments{RESET}", gh_client.identity.display_name());
    }
    if bsky.is_some() {
        println!("{DIM}  bluesky:  connected — use --bluesky-post <text> to post{RESET}");
    }
    println!("{DIM}  Type /help for commands{RESET}\n");

    let session_start = std::time::Instant::now();

    // Handle Ctrl+C gracefully
    let ctrlc_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let flag = ctrlc_flag.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            eprintln!("\n{DIM}  ⚡ signal received — emergency shutdown — bye 👋{RESET}\n");
            std::process::exit(0);
        });
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    // repl was already initialized above (before banner) to allow memory display

    // Telegram inbound poll: spawn a background task that polls for bot commands
    // and sends them over a channel for the main loop to process after each turn.
    let tg_rx = if let Some(ref tg_client) = tg {
        let (tx, rx) = tokio::sync::mpsc::channel::<axonix::telegram::BotCommand>(16);
        let tg_poll = tg_client.clone();
        tokio::spawn(async move {
            let mut offset: i64 = 0;
            loop {
                match tg_poll.get_updates(offset).await {
                    Ok(updates) => {
                        if !updates.is_empty() {
                            offset = updates.iter().map(|u| u.update_id).max().unwrap_or(offset) + 1;
                            let commands = tg_poll.extract_commands(&updates);
                            for cmd in commands {
                                if tx.send(cmd).await.is_err() {
                                    return; // receiver dropped, session ended
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Network error — wait before retrying
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
                // Small sleep between polls to avoid hammering the API
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        });
        Some(rx)
    } else {
        None
    };
    let mut tg_rx = tg_rx;

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

        // Dispatch through handle_command first, then handle agent-data commands inline
        let cmd_result = handle_command(input, &mut repl, &skill_names);
        match cmd_result {
            CommandResult::Quit => break,

            CommandResult::Clear => {
                agent = make_agent(&api_key, &repl.model, skills.clone(), &system_prompt);
                repl.reset_tokens();
                println!("{DIM}  (conversation cleared){RESET}\n");
                continue;
            }

            CommandResult::SwitchModel(ref new_model) => {
                agent = make_agent(&api_key, new_model, skills.clone(), &system_prompt);
                println!("{DIM}  (switched to {new_model}, conversation cleared){RESET}\n");
                continue;
            }

            CommandResult::Retry(ref prompt) => {
                println!("{DIM}  (retrying: {}){RESET}", truncate(prompt, 60));
                let prompt = prompt.clone();
                run_prompt(&mut agent, &prompt, &mut repl, tg.as_ref()).await;
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
                            Err(e) => println!("\n{RED}  ✗ failed to fetch issues: {e}{RESET}\n"),
                            Ok(issues) => {
                                println!();
                                if issues.is_empty() {
                                    println!("{DIM}  (no open issues){RESET}\n");
                                } else {
                                    println!("{DIM}  Open issues ({} shown, sorted by 👍):{RESET}", issues.len());
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
                        println!("{DIM}  (journal has {} or fewer recent entries — no archiving needed){RESET}\n",
                            result.kept);
                    }
                    Ok(result) => {
                        println!("{GREEN}  ✓ archived {} entries to {} ({} kept in JOURNAL.md){RESET}\n",
                            result.moved, result.archive_path, result.kept);
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
                    Ok(db) => {
                        match db.search_memory(query, 20) {
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
                                println!("  ({} result{})\n", results.len(),
                                    if results.len() == 1 { "" } else { "s" });
                            }
                        }
                    }
                }
                continue;
            }

            CommandResult::ShowRecentMemories => {
                let sep = "─".repeat(55);
                match axonix::db::AxonixDb::open_default() {
                    Err(e) => {
                        println!("{RED}  ✗ DB error: {e}{RESET}\n");
                    }
                    Ok(db) => {
                        match db.hot_memories_list(5) {
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
                                println!("  ({} entr{})\n", rows.len(),
                                    if rows.len() == 1 { "y" } else { "ies" });
                            }
                        }
                    }
                }
                continue;
            }

            CommandResult::Handled(ref output_lines) => {
                // Render the output lines, interpreting special markers
                let mut gh_comment_request: Option<(u64, String)> = None;
                let mut review_request: Option<String> = None;
                let mut do_recap = false;
                for line in output_lines {
                    if let Some(rest) = line.strip_prefix("__save:") {
                        // Perform the actual save (needs agent messages)
                        match save_conversation(agent.messages(), rest) {
                            Ok(count) => println!("{DIM}  saved {count} messages to {rest}{RESET}\n"),
                            Err(e) => println!("{RED}  failed to save: {e}{RESET}\n"),
                        }
                    } else if let Some(rest) = line.strip_prefix("__review:") {
                        // Collect review task for async dispatch after sync loop
                        review_request = Some(rest.to_string());
                    } else if let Some(rest) = line.strip_prefix("__gh_comment:") {
                        // format: "__gh_comment:<issue>:<body>"
                        // Collect for async dispatch after the sync loop
                        let mut parts = rest.splitn(2, ':');
                        let issue_str = parts.next().unwrap_or("0");
                        let body = parts.next().unwrap_or("").to_string();
                        if let Ok(n) = issue_str.parse::<u64>() {
                            gh_comment_request = Some((n, body));
                        }
                    } else if let Some(text) = line.strip_prefix("__predict:") {
                        // /predict <text> shorthand — store via PredictionStore
                        let text = text.trim();
                        if text.is_empty() {
                            println!("{YELLOW}  ⚠ prediction text cannot be empty{RESET}\n");
                        } else {
                            let mut store = axonix::predictions::PredictionStore::default_path();
                            let id = store.predict(text);
                            match store.save() {
                                Ok(()) => println!("{GREEN}  ✓ prediction #{id} saved: {text}{RESET}\n"),
                                Err(e) => println!("{YELLOW}  ⚠ prediction #{id} queued but save failed: {e}{RESET}\n"),
                            }
                        }
                    } else if let Some(rest) = line.strip_prefix("__lint_ok:") {
                        // format: "__lint_ok:<path>:<summary>"
                        let (path, summary) = rest.split_once(':').unwrap_or((rest, "valid"));
                        println!("{GREEN}  ✓ {path}: {summary}{RESET}");
                    } else if let Some(rest) = line.strip_prefix("__lint_errors:") {
                        let (path, count) = rest.split_once(':').unwrap_or((rest, "?"));
                        println!("{RED}  ✗ {path} has {count} error(s):{RESET}");
                    } else if let Some(rest) = line.strip_prefix("__lint_error:") {
                        // format: "__lint_error:<line>:<message>"
                        let (lineno, msg) = rest.split_once(':').unwrap_or(("0", rest));
                        let n: usize = lineno.parse().unwrap_or(0);
                        if n > 0 {
                            println!("{RED}    line {n}: {msg}{RESET}");
                        } else {
                            println!("{RED}    {msg}{RESET}");
                        }
                    } else if let Some(rest) = line.strip_prefix("__lint_unsupported:") {
                        println!("{YELLOW}  ⚠ {rest}{RESET}");
                    } else if let Some(rest) = line.strip_prefix("__ssh_error:") {
                        // format: "__ssh_error:<host>:<message>"
                        let (host, msg) = rest.split_once(':').unwrap_or((rest, "unknown error"));
                        println!("{RED}  ✗ ssh {host}: {msg}{RESET}\n");
                    } else if let Some(rest) = line.strip_prefix("__ssh_result:") {
                        // format: "__ssh_result:<host>:<exit_code>:<output>"
                        let mut parts = rest.splitn(3, ':');
                        let host = parts.next().unwrap_or("?");
                        let exit_code: i32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
                        let output = parts.next().unwrap_or("").trim();
                        if exit_code == 0 {
                            if output.is_empty() {
                                println!("{GREEN}  ✓ {host}: (no output){RESET}\n");
                            } else {
                                println!("{GREEN}  ✓ {host}{RESET}");
                                for out_line in output.lines() {
                                    println!("    {out_line}");
                                }
                                println!();
                            }
                        } else {
                            println!("{RED}  ✗ {host} (exit {exit_code}){RESET}");
                            if !output.is_empty() {
                                for out_line in output.lines() {
                                    println!("    {out_line}");
                                }
                            }
                            println!();
                        }
                    } else if line == "__recap" {
                        do_recap = true;
                    } else if line.is_empty() {
                        println!();
                    } else {
                        println!("{DIM}{line}{RESET}");
                    }
                }
                // Add trailing newline after lint errors block if needed
                let has_lint_error = output_lines.iter().any(|l| l.starts_with("__lint_errors:"));
                if has_lint_error {
                    println!();
                }
                // Handle async GitHub comment posting
                if let Some((issue_n, body)) = gh_comment_request {
                    match &gh {
                        None => println!("{YELLOW}  ⚠ No GitHub token available (set GH_TOKEN or AXONIX_BOT_TOKEN){RESET}\n"),
                        Some(gh_client) => {
                            print!("{YELLOW}  ▶ posting comment on issue #{issue_n} as {}...{RESET}", gh_client.identity.display_name());
                            io::stdout().flush().ok();
                            match gh_client.post_comment("coe0718/axonix", issue_n, &body).await {
                                Ok(url) => println!("\n{GREEN}  ✓ comment posted: {url}{RESET}\n"),
                                Err(e) => println!("\n{RED}  ✗ failed to post comment: {e}{RESET}\n"),
                            }
                        }
                    }
                }
                // Handle async /review — invoke code_reviewer sub-agent via agent prompt (G-028)
                if let Some(review_task) = review_request {
                    println!("{DIM}  🔍 invoking code_reviewer sub-agent...{RESET}");
                    io::stdout().flush().ok();
                    let review_prompt = format!(
                        "Use the code_reviewer tool to review this change: {review_task}\n\
                         Print the review findings directly. Be concise — 3-5 bullets max."
                    );
                    run_prompt(&mut agent, &review_prompt, &mut repl, tg.as_ref()).await;
                }
                // Handle /recap: post a 3-post Bluesky thread summarising the session (Issue #49)
                if do_recap {
                    match &bsky {
                        None => println!("{YELLOW}  ⚠ /recap requires Bluesky. Set BLUESKY_IDENTIFIER and BLUESKY_APP_PASSWORD{RESET}\n"),
                        Some(bsky_client) => {
                            println!("{DIM}  📡 posting recap thread to Bluesky...{RESET}");
                            io::stdout().flush().ok();
                            // 1. Get session title from JOURNAL.md
                            let session_title = read_journal_title()
                                .unwrap_or_else(|| "Axonix session".to_string());
                            // 2. Get recent commits
                            let commit_subjects = get_recent_commits(5);
                            // 3. Build and post root post
                            let day = std::env::var("DAY_COUNT").ok()
                                .and_then(|s| s.split_whitespace().next().map(|n| n.parse::<u32>().unwrap_or(8)))
                                .unwrap_or(8);
                            let session = std::env::var("SESSION_COUNT").ok()
                                .and_then(|s| s.parse::<u32>().ok())
                                .unwrap_or(1);
                            let root_text = format!("axonix Day {day}, Session {session}: {session_title}");
                            let root_text = if root_text.chars().count() > 300 {
                                let truncated: String = root_text.chars().take(297).collect();
                                format!("{truncated}…")
                            } else {
                                root_text
                            };
                            match bsky_client.post(&root_text).await {
                                Err(e) => println!("{RED}  ✗ Bluesky recap post 1 failed: {e}{RESET}\n"),
                                Ok((root_uri, root_cid)) => {
                                    println!("{GREEN}  ✓ post 1: {root_uri}{RESET}");
                                    // Post 2: what changed (commits)
                                    let commit_refs: Vec<&str> = commit_subjects.iter().map(|s| s.as_str()).collect();
                                    let commits_text = BlueskyClient::format_recap_commits(&commit_refs);
                                    match bsky_client.post_reply(&commits_text, &root_uri, &root_cid, &root_uri, &root_cid).await {
                                        Err(e) => println!("{RED}  ✗ Bluesky recap post 2 failed: {e}{RESET}\n"),
                                        Ok((p2_uri, p2_cid)) => {
                                            println!("{GREEN}  ✓ post 2: {p2_uri}{RESET}");
                                            // Post 3: test count
                                            if let Some(test_count) = get_test_count() {
                                                let tests_text = BlueskyClient::format_recap_tests(test_count, None);
                                                match bsky_client.post_reply(&tests_text, &root_uri, &root_cid, &p2_uri, &p2_cid).await {
                                                    Err(e) => println!("{RED}  ✗ Bluesky recap post 3 failed: {e}{RESET}\n"),
                                                    Ok((p3_uri, _)) => println!("{GREEN}  ✓ post 3: {p3_uri}{RESET}"),
                                                }
                                            }
                                        }
                                    }
                                    println!();
                                }
                            }
                        }
                    }
                }
                continue;
            }

            CommandResult::NotACommand => {
                // Handle commands that need agent/session data inline
                match input {
                    "/status" => {
                        let msg_count = agent.messages().len();
                        let elapsed = session_start.elapsed();
                        let mins = elapsed.as_secs() / 60;
                        let secs = elapsed.as_secs() % 60;
                        println!("{DIM}  model:    {}{RESET}", agent.model);
                        println!("{DIM}  messages: {msg_count}{RESET}");
                        println!("{DIM}  tokens:   {} in / {} out (session total){RESET}", repl.total_input, repl.total_output);
                        if repl.total_cache_read > 0 || repl.total_cache_write > 0 {
                            println!("{DIM}  cache:    {} read / {} write{RESET}", repl.total_cache_read, repl.total_cache_write);
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
                        let cost = estimate_cost(&repl.model, repl.total_input, repl.total_output, repl.total_cache_read, repl.total_cache_write);
                        println!("{DIM}  Token usage (session total):{RESET}");
                        println!("{DIM}    input:       {}{RESET}", repl.total_input);
                        println!("{DIM}    output:      {}{RESET}", repl.total_output);
                        if repl.total_cache_read > 0 || repl.total_cache_write > 0 {
                            println!("{DIM}    cache read:  {}{RESET}", repl.total_cache_read);
                            println!("{DIM}    cache write: {}{RESET}", repl.total_cache_write);
                        }
                        println!("{DIM}    total:       {}{RESET}", repl.total_input + repl.total_output + repl.total_cache_read + repl.total_cache_write);
                        println!("{DIM}    est. cost:   ${cost:.4}{RESET}");
                        println!();
                        continue;
                    }
                    _ => {} // Fall through to agent prompt
                }

                repl.push_prompt(input);
                run_prompt(&mut agent, input, &mut repl, tg.as_ref()).await;
            }
        }

        // After each main-loop turn, drain any queued Telegram bot commands
        if let Some(ref mut rx) = tg_rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    axonix::telegram::BotCommand::Ask(ask_cmd) => {
                        let ask_prompt = ask_cmd.prompt.clone();
                        let msg_id = ask_cmd.message_id;
                        println!("\n{DIM}  📱 Telegram ask: {}{RESET}", truncate(&ask_prompt, 60));
                        repl.push_prompt(&ask_prompt);
                        run_prompt(&mut agent, &ask_prompt, &mut repl, tg.as_ref()).await;
                        // Acknowledge in Telegram that the ask was processed
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("✅ Done", msg_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Help { message_id } => {
                        println!("\n{DIM}  📱 Telegram /help{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to(axonix::telegram::TELEGRAM_HELP_TEXT, message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Status { message_id } => {
                        println!("\n{DIM}  📱 Telegram /status{RESET}");
                        if let Some(ref tg_client) = tg {
                            let elapsed = session_start.elapsed().as_secs();
                            let reply = TelegramClient::format_status_reply(
                                &repl.model,
                                "interactive",
                                elapsed,
                                repl.total_input,
                                repl.total_output,
                            );
                            tg_client.reply_to(&reply, message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Health { message_id } => {
                        println!("\n{DIM}  📱 Telegram /health{RESET}");
                        if let Some(ref tg_client) = tg {
                            let snapshot = axonix::health::HealthSnapshot::collect();
                            let docker = axonix::health::docker_health();
                            let reply = format!("{}\n\n{}", snapshot.format(), docker.format());
                            tg_client.reply_to(&reply, message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Brief { message_id } => {
                        println!("\n{DIM}  📱 Telegram /brief{RESET}");
                        if let Some(ref tg_client) = tg {
                            let brief = axonix::brief::Brief::collect();
                            tg_client.reply_to(&brief.format_telegram(), message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Run { task, message_id } => {
                        println!("\n{DIM}  📱 Telegram /run: {}{RESET}", truncate(&task, 60));
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to(&format!("⏳ Running task: _{task}_"), message_id).await.ok();
                            run_prompt(&mut agent, &task, &mut repl, tg.as_ref()).await;
                            tg_client.reply_to("✅ Done", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Goal { description, message_id } => {
                        println!("\n{DIM}  📱 Telegram /goal: {}{RESET}", truncate(&description, 60));
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /goal is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Memory { action: _, message_id } => {
                        println!("\n{DIM}  📱 Telegram /memory{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /memory is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::History { message_id } => {
                        println!("\n{DIM}  📱 Telegram /history{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /history is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Goals { message_id } => {
                        println!("\n{DIM}  📱 Telegram /goals{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /goals is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Predict { text: _, message_id } => {
                        println!("\n{DIM}  📱 Telegram /predict{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /predict is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::Resolve { id: _, verdict: _, message_id } => {
                        println!("\n{DIM}  📱 Telegram /resolve{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /resolve is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                    axonix::telegram::BotCommand::ListPredictions { message_id } => {
                        println!("\n{DIM}  📱 Telegram /predictions{RESET}");
                        if let Some(ref tg_client) = tg {
                            tg_client.reply_to("⚠️ /predictions is handled by the listener daemon.", message_id).await.ok();
                        }
                    }
                }
            }
        }
    }

    println!("\n{DIM}  ⚡ AXONIX OFFLINE — shutting down subsystems... bye 👋{RESET}\n");
}
