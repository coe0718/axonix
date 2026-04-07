//! `/predict` command handler extracted from `handle_command`.

use crate::render::truncate;
use super::types::{CommandResult, ReplState};

/// Return the usage/help lines for the `/predict` command.
pub fn predict_usage() -> Vec<String> {
    vec![
        "  Usage:".to_string(),
        "    /predict add <text>                  Log a new prediction".to_string(),
        "    /predict resolve <id> <outcome>      Mark a prediction resolved".to_string(),
        "    /predict resolve <id> <out> | <delta> With a delta note".to_string(),
        "    /predict open                        List open predictions".to_string(),
        "    /predict list                        List all predictions".to_string(),
        String::new(),
        "  Prediction tracking builds self-calibration data over time.".to_string(),
        "  Record what you expect, then resolve with what actually happened.".to_string(),
        String::new(),
    ]
}

/// Handle the `/predict` command and all its subcommands.
///
/// `arg` is the text after `/predict ` (or `""` when bare `/predict` is typed).
pub fn handle_predict(arg: &str, state: &mut ReplState) -> CommandResult {
    if arg.is_empty() || arg == "help" {
        CommandResult::Handled(predict_usage())
    } else if let Some(text) = arg.strip_prefix("add ") {
        let text = text.trim();
        if text.is_empty() {
            return CommandResult::Handled(vec![
                "  Usage: /predict add <text>".to_string(),
                "  Example: /predict add the build will succeed with no warnings".to_string(),
                String::new(),
            ]);
        }
        let id = state.predictions.predict(text);
        let save_msg = match state.predictions.save() {
            Ok(_) => format!("  ✓ prediction #{id} logged: {}", truncate(text, 60)),
            Err(e) => format!("  ⚠ prediction #{id} queued but failed to save: {e}"),
        };
        CommandResult::Handled(vec![save_msg, String::new()])
    } else if let Some(rest) = arg.strip_prefix("resolve ") {
        // Format: "resolve <id> <outcome>" or "resolve <id> <outcome> | <delta>"
        let rest = rest.trim();
        let mut parts = rest.splitn(2, ' ');
        let id_str = parts.next().unwrap_or("").trim();
        let outcome_and_delta = parts.next().unwrap_or("").trim();

        if id_str.is_empty() || outcome_and_delta.is_empty() {
            return CommandResult::Handled(vec![
                "  Usage: /predict resolve <id> <outcome>".to_string(),
                "  Example: /predict resolve 1 build passed with 5 warnings".to_string(),
                "  With delta: /predict resolve 1 passed | underestimated warning count".to_string(),
                String::new(),
            ]);
        }

        let id: u32 = match id_str.parse() {
            Ok(n) if n > 0 => n,
            _ => {
                return CommandResult::Handled(vec![
                    format!("  Error: prediction ID must be a positive integer, got '{id_str}'"),
                    "  Use /predict list to see prediction IDs".to_string(),
                    String::new(),
                ]);
            }
        };

        // Split on " | " to get optional delta
        let (outcome, delta) = if let Some(pipe_pos) = outcome_and_delta.find(" | ") {
            let outcome = outcome_and_delta[..pipe_pos].trim();
            let delta = outcome_and_delta[pipe_pos + 3..].trim();
            (outcome, if delta.is_empty() { None } else { Some(delta) })
        } else {
            (outcome_and_delta, None)
        };

        match state.predictions.resolve(id, outcome, delta) {
            Err(e) => CommandResult::Handled(vec![
                format!("  Error: {e}"),
                String::new(),
            ]),
            Ok(prediction_text) => {
                let save_msg = match state.predictions.save() {
                    Ok(_) => {
                        let mut lines = vec![
                            format!("  ✓ prediction #{id} resolved"),
                            format!("    was:     {}", truncate(&prediction_text, 60)),
                            format!("    actual:  {}", truncate(outcome, 60)),
                        ];
                        if let Some(d) = delta {
                            lines.push(format!("    delta:   {}", truncate(d, 60)));
                        }
                        lines.push(String::new());
                        lines
                    }
                    Err(e) => vec![
                        format!("  ✓ prediction #{id} resolved (save failed: {e})"),
                        String::new(),
                    ],
                };
                CommandResult::Handled(save_msg)
            }
        }
    } else if arg == "list" {
        let total = state.predictions.count();
        if total == 0 {
            CommandResult::Handled(vec![
                "  Predictions: (none yet)".to_string(),
                "  Use /predict add <text> to log your first prediction.".to_string(),
                String::new(),
            ])
        } else {
            let mut lines = vec![format!("  Predictions ({total} total, {} open, {} resolved):",
                state.predictions.open_count(), state.predictions.resolved_count())];
            // Show open first, then resolved
            let open = state.predictions.open();
            if !open.is_empty() {
                lines.push("  Open:".to_string());
                for (id, pred) in &open {
                    lines.push(format!("    #{id:<4} [{}] {}",
                        pred.created, truncate(&pred.prediction, 55)));
                }
            }
            let resolved = state.predictions.resolved();
            if !resolved.is_empty() {
                lines.push("  Resolved:".to_string());
                for (id, pred) in &resolved {
                    let date = pred.resolved.as_deref().unwrap_or("?");
                    lines.push(format!("    #{id:<4} [{}] {}",
                        date, truncate(&pred.prediction, 55)));
                    if let Some(outcome) = &pred.outcome {
                        lines.push(format!("         → actual: {}", truncate(outcome, 55)));
                    }
                    if let Some(delta) = &pred.delta {
                        lines.push(format!("         Δ delta:  {}", truncate(delta, 55)));
                    }
                }
            }
            lines.push(String::new());
            CommandResult::Handled(lines)
        }
    } else if arg == "open" {
        let open = state.predictions.open();
        if open.is_empty() {
            CommandResult::Handled(vec![
                "  No open predictions.".to_string(),
                "  Use /predict add <text> to log one, or /predict list to see resolved.".to_string(),
                String::new(),
            ])
        } else {
            let mut lines = vec![format!("  Open predictions ({}):", open.len())];
            for (id, pred) in &open {
                lines.push(format!("    #{id:<4} [{}] {}",
                    pred.created, truncate(&pred.prediction, 55)));
            }
            lines.push(String::new());
            lines.push(format!("  Use /predict resolve <id> <outcome> to close one."));
            lines.push(String::new());
            CommandResult::Handled(lines)
        }
    } else {
        // Shorthand: `/predict <text>` without a subcommand keyword
        let text = arg.trim();
        if text.is_empty() {
            CommandResult::Handled(predict_usage())
        } else {
            CommandResult::Handled(vec![
                format!("__predict:{text}"),
            ])
        }
    }
}
