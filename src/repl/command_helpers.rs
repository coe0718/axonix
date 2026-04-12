//! Helper functions for REPL command parsing.

use super::super::types::CommandResult;

/// Parse the argument to `/issues [N]` and return a `CommandResult`.
///
/// Returns `FetchIssues(n)` for a valid limit, or `Handled(error_lines)` for
/// bad input. Extracted from `handle_command` to keep that function ≤ 300 lines.
pub(super) fn handle_issues_arg(arg: &str) -> CommandResult {
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
