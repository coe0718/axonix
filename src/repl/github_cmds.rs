//! GitHub comment and respond command handlers extracted from `handle_command`.

use super::types::CommandResult;

/// Handle the `/comment` command.
///
/// `arg` is the text after `/comment ` (or `""` when bare `/comment` is typed).
pub fn handle_comment(arg: &str) -> CommandResult {
    if arg.is_empty() {
        return CommandResult::Handled(vec![
            "  Usage: /comment <issue_number> <text>".to_string(),
            "  Example: /comment 13 Thanks for the report — fixed in this session.".to_string(),
            String::new(),
        ]);
    }

    let mut parts = arg.splitn(2, ' ');
    let issue_str = parts.next().unwrap_or("").trim();
    let body = parts.next().unwrap_or("").trim();

    match issue_str.parse::<u64>() {
        Err(_) | Ok(0) => CommandResult::Handled(vec![
            format!("  Error: issue number must be a positive integer, got '{issue_str}'"),
            "  Usage: /comment <issue_number> <text>".to_string(),
            String::new(),
        ]),
        Ok(_) if body.is_empty() => CommandResult::Handled(vec![
            format!("  Error: comment body cannot be empty"),
            format!("  Usage: /comment {issue_str} <text>"),
            String::new(),
        ]),
        Ok(n) => {
            // Return marker for caller to handle async POST
            CommandResult::Handled(vec![
                format!("__gh_comment:{n}:{body}"),
            ])
        }
    }
}

/// Handle the `/respond` command.
///
/// `arg` is the text after `/respond ` (or `""` when bare `/respond` is typed).
pub fn handle_respond(arg: &str) -> CommandResult {
    if arg.is_empty() {
        return CommandResult::Handled(vec![
            "  Usage: /respond <issue_number> <text>".to_string(),
            "         /respond <issue_number> close <text>".to_string(),
            "  Example: /respond 13 Fixed in this session. Tests added.".to_string(),
            "  Example: /respond 13 close Fixed and deployed.".to_string(),
            "  Difference from /comment: use 'close' to close the issue after responding.".to_string(),
            String::new(),
        ]);
    }

    let mut parts = arg.splitn(2, ' ');
    let issue_str = parts.next().unwrap_or("").trim();
    let rest = parts.next().unwrap_or("").trim();

    match issue_str.parse::<u64>() {
        Err(_) | Ok(0) => CommandResult::Handled(vec![
            format!("  Error: issue number must be a positive integer, got '{issue_str}'"),
            "  Usage: /respond <issue_number> <text>".to_string(),
            String::new(),
        ]),
        Ok(n) => {
            let (close_flag, body) = if rest.starts_with("close ") {
                (1u8, rest.trim_start_matches("close ").trim())
            } else if rest == "close" {
                // just "close" with no body
                return CommandResult::Handled(vec![
                    "  Error: 'close' must be followed by response text".to_string(),
                    format!("  Usage: /respond {n} close <text>"),
                    String::new(),
                ]);
            } else {
                (0u8, rest)
            };

            if body.is_empty() {
                return CommandResult::Handled(vec![
                    "  Error: response body cannot be empty".to_string(),
                    format!("  Usage: /respond {n} <text>"),
                    String::new(),
                ]);
            }

            CommandResult::Handled(vec![
                format!("__gh_respond:{n}:{close_flag}:{body}"),
            ])
        }
    }
}
