//! TelegramClient status/response formatting helpers.

use super::TelegramClient;

impl TelegramClient {
    /// Format a long agent response for Telegram.
    ///
    /// Telegram has a 4096-character message limit. Splits long responses
    /// into chunks at paragraph boundaries, respecting UTF-8 character boundaries.
    pub fn format_response(text: &str) -> Vec<String> {
        const MAX_LEN: usize = 3800; // leave room for formatting overhead
        if text.len() <= MAX_LEN {
            return vec![text.to_string()];
        }
        let mut chunks = Vec::new();
        let mut remaining = text;
        while remaining.len() > MAX_LEN {
            // Find a safe byte boundary at or before MAX_LEN.
            // Walk backwards from MAX_LEN to find the start of a valid char.
            let safe_end = (0..=MAX_LEN)
                .rev()
                .find(|&i| remaining.is_char_boundary(i))
                .unwrap_or(0);

            // Within that safe window, try to split at the last newline.
            let split_at = remaining[..safe_end]
                .rfind('\n')
                .unwrap_or(safe_end);

            // Ensure split_at is also on a char boundary (rfind on '\n' always gives
            // a char-boundary position, but guard defensively).
            let split_at = if remaining.is_char_boundary(split_at) {
                split_at
            } else {
                safe_end
            };

            chunks.push(remaining[..split_at].to_string());
            remaining = remaining[split_at..].trim_start_matches('\n');
        }
        if !remaining.is_empty() {
            chunks.push(remaining.to_string());
        }
        chunks
    }

    /// Build a status reply string for the `/status` command.
    ///
    /// `model` — current model name
    /// `mode` — "interactive" or "cron"
    /// `elapsed_secs` — seconds since session start (0 if unknown)
    /// `tokens_in` / `tokens_out` — session token totals
    pub fn format_status_reply(
        model: &str,
        mode: &str,
        elapsed_secs: u64,
        tokens_in: u64,
        tokens_out: u64,
    ) -> String {
        let mins = elapsed_secs / 60;
        let secs = elapsed_secs % 60;
        let elapsed_str = if mins > 0 {
            format!("{mins}m {secs}s")
        } else {
            format!("{secs}s")
        };
        format!(
            "*Axonix Status*\n\
            🤖 model: `{model}`\n\
            ⚙️ mode: {mode}\n\
            ⏱ uptime: {elapsed_str}\n\
            📊 tokens: {tokens_in} in / {tokens_out} out",
        )
    }

    /// Build an enhanced status reply including active goal and last commit.
    pub fn format_enhanced_status_reply(
        model: &str,
        elapsed_secs: u64,
        active_goal: Option<&str>,
        last_commit: Option<&str>,
        prediction_accuracy: Option<&str>,
    ) -> String {
        let mins = elapsed_secs / 60;
        let secs = elapsed_secs % 60;
        let elapsed_str = if mins > 0 {
            format!("{mins}m {secs}s")
        } else {
            format!("{secs}s")
        };
        let mut parts = vec![
            format!("*Axonix Status*"),
            format!("🤖 model: `{model}`"),
            format!("⚙️ mode: listener"),
            format!("⏱ uptime: {elapsed_str}"),
        ];
        if let Some(goal) = active_goal {
            parts.push(format!("🎯 active goal: {goal}"));
        }
        if let Some(commit) = last_commit {
            parts.push(format!("📝 last commit: {commit}"));
        }
        if let Some(acc) = prediction_accuracy {
            parts.push(format!("🎯 predictions: {acc}"));
        }
        // Append git activity summary (last 3 commits)
        let git_summary = crate::git_summary::format_for_telegram(3);
        parts.push(String::new());
        parts.push(git_summary);
        parts.join("\n")
    }

    /// Split a message into Telegram-safe chunks (alias for format_response).
    pub fn chunk_message(text: &str) -> Vec<String> {
        Self::format_response(text)
    }
}
