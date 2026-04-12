//! Async marker-based dispatcher for `CommandResult::Handled` output lines.
//!
//! Handles `__save:`, `__review:`, `__gh_comment:`, `__predict:`, `__lint_*:`,
//! `__ssh_*:`, and `__recap` markers, then performs async GitHub comment posting,
//! code review dispatch, and Bluesky recap thread posting.

use std::io::{self, Write};

use axonix::bluesky::BlueskyClient;
use axonix::conversation::save_conversation;
use axonix::github::GitHubClient;
use axonix::render::*;
use axonix::repl::ReplState;
use axonix::telegram::TelegramClient;

use crate::prompt_runner::run_prompt;
use crate::session_helpers::{get_recent_commits, get_test_count, read_journal_title};

/// Handle a `CommandResult::Handled` response.
///
/// Iterates the output lines, dispatching special `__marker:` prefixes for
/// async operations (GitHub comment, Bluesky recap, code review, etc.).
#[allow(clippy::too_many_arguments)]
pub async fn handle_handled_result(
    output_lines: &[String],
    agent: &mut yoagent::Agent,
    repl: &mut ReplState,
    tg: Option<&TelegramClient>,
    gh: Option<&GitHubClient>,
    bsky: Option<&BlueskyClient>,
) {
    let mut gh_comment_request: Option<(u64, String)> = None;
    let mut review_request: Option<String> = None;
    let mut do_recap = false;

    for line in output_lines {
        if let Some(rest) = line.strip_prefix("__save:") {
            match save_conversation(agent.messages(), rest) {
                Ok(count) => println!("{DIM}  saved {count} messages to {rest}{RESET}\n"),
                Err(e) => println!("{RED}  failed to save: {e}{RESET}\n"),
            }
        } else if let Some(rest) = line.strip_prefix("__review:") {
            review_request = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("__gh_comment:") {
            let mut parts = rest.splitn(2, ':');
            let issue_str = parts.next().unwrap_or("0");
            let body = parts.next().unwrap_or("").to_string();
            if let Ok(n) = issue_str.parse::<u64>() {
                gh_comment_request = Some((n, body));
            }
        } else if let Some(text) = line.strip_prefix("__predict:") {
            let text = text.trim();
            if text.is_empty() {
                println!("{YELLOW}  ⚠ prediction text cannot be empty{RESET}\n");
            } else {
                let mut store = axonix::predictions::PredictionStore::default_path();
                let id = store.predict(text);
                match store.save() {
                    Ok(()) => println!("{GREEN}  ✓ prediction #{id} saved: {text}{RESET}\n"),
                    Err(e) => {
                        println!("{YELLOW}  ⚠ prediction #{id} queued but save failed: {e}{RESET}\n")
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("__lint_ok:") {
            let (path, summary) = rest.split_once(':').unwrap_or((rest, "valid"));
            println!("{GREEN}  ✓ {path}: {summary}{RESET}");
        } else if let Some(rest) = line.strip_prefix("__lint_errors:") {
            let (path, count) = rest.split_once(':').unwrap_or((rest, "?"));
            println!("{RED}  ✗ {path} has {count} error(s):{RESET}");
        } else if let Some(rest) = line.strip_prefix("__lint_error:") {
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
            let (host, msg) = rest.split_once(':').unwrap_or((rest, "unknown error"));
            println!("{RED}  ✗ ssh {host}: {msg}{RESET}\n");
        } else if let Some(rest) = line.strip_prefix("__ssh_result:") {
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
    let has_lint_error = output_lines
        .iter()
        .any(|l| l.starts_with("__lint_errors:"));
    if has_lint_error {
        println!();
    }

    // Handle async GitHub comment posting
    if let Some((issue_n, body)) = gh_comment_request {
        match gh {
            None => println!(
                "{YELLOW}  ⚠ No GitHub token available (set GH_TOKEN or AXONIX_BOT_TOKEN){RESET}\n"
            ),
            Some(gh_client) => {
                print!(
                    "{YELLOW}  ▶ posting comment on issue #{issue_n} as {}...{RESET}",
                    gh_client.identity.display_name()
                );
                io::stdout().flush().ok();
                match gh_client
                    .post_comment("coe0718/axonix", issue_n, &body)
                    .await
                {
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
        run_prompt(agent, &review_prompt, repl, tg).await;
    }

    // Handle /recap: post a 3-post Bluesky thread summarising the session (Issue #49)
    if do_recap {
        match bsky {
            None => println!(
                "{YELLOW}  ⚠ /recap requires Bluesky. Set BLUESKY_IDENTIFIER and BLUESKY_APP_PASSWORD{RESET}\n"
            ),
            Some(bsky_client) => {
                println!("{DIM}  📡 posting recap thread to Bluesky...{RESET}");
                io::stdout().flush().ok();
                let session_title =
                    read_journal_title().unwrap_or_else(|| "Axonix session".to_string());
                let commit_subjects = get_recent_commits(5);
                let day = std::env::var("DAY_COUNT")
                    .ok()
                    .and_then(|s| {
                        s.split_whitespace()
                            .next()
                            .map(|n| n.parse::<u32>().unwrap_or(8))
                    })
                    .unwrap_or(8);
                let session = std::env::var("SESSION_COUNT")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);
                let root_text =
                    format!("axonix Day {day}, Session {session}: {session_title}");
                let root_text = if root_text.chars().count() > 300 {
                    let truncated: String = root_text.chars().take(297).collect();
                    format!("{truncated}…")
                } else {
                    root_text
                };
                match bsky_client.post(&root_text).await {
                    Err(e) => {
                        println!("{RED}  ✗ Bluesky recap post 1 failed: {e}{RESET}\n")
                    }
                    Ok((root_uri, root_cid)) => {
                        println!("{GREEN}  ✓ post 1: {root_uri}{RESET}");
                        let commit_refs: Vec<&str> =
                            commit_subjects.iter().map(|s| s.as_str()).collect();
                        let commits_text =
                            BlueskyClient::format_recap_commits(&commit_refs);
                        match bsky_client
                            .post_reply(
                                &commits_text,
                                &root_uri,
                                &root_cid,
                                &root_uri,
                                &root_cid,
                            )
                            .await
                        {
                            Err(e) => println!(
                                "{RED}  ✗ Bluesky recap post 2 failed: {e}{RESET}\n"
                            ),
                            Ok((p2_uri, p2_cid)) => {
                                println!("{GREEN}  ✓ post 2: {p2_uri}{RESET}");
                                if let Some(test_count) = get_test_count() {
                                    let tests_text = BlueskyClient::format_recap_tests(
                                        test_count, None,
                                    );
                                    match bsky_client
                                        .post_reply(
                                            &tests_text,
                                            &root_uri,
                                            &root_cid,
                                            &p2_uri,
                                            &p2_cid,
                                        )
                                        .await
                                    {
                                        Err(e) => println!(
                                            "{RED}  ✗ Bluesky recap post 3 failed: {e}{RESET}\n"
                                        ),
                                        Ok((p3_uri, _)) => {
                                            println!("{GREEN}  ✓ post 3: {p3_uri}{RESET}")
                                        }
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
}
