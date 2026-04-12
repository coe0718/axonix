//! Proactive background tasks for the Telegram listener.
//!
//! - [`poll_github_issues`]: poll GitHub for new `agent-input` issues and acknowledge them.
//! - [`send_daily_brief`]: push a morning brief to Telegram once per day.

use crate::telegram::TelegramClient;
use super::config::AckedIssues;

/// Poll GitHub for new `agent-input` labelled issues and post acknowledgement comments.
///
/// Issues that have already been acknowledged (stored in `acked_issues`) are skipped.
/// Successfully acknowledged issues are persisted back to disk.
pub(super) async fn poll_github_issues(
    tg: &TelegramClient,
    gh_token: &str,
    acked_issues: &mut AckedIssues,
) {
    let _ = tg; // tg unused here — acknowledgements go to GitHub, not Telegram
    let gh = crate::github::GitHubClient::new(
        gh_token,
        crate::github::GitHubIdentity::Bot,
    );
    match gh.list_issues("coe0718/axonix", 20).await {
        Ok(issues) => {
            let new_issues: Vec<_> = issues
                .iter()
                .filter(|i| i.labels.iter().any(|l| l == "agent-input"))
                .filter(|i| !acked_issues.contains(u64::from(i.number)))
                .collect();
            for issue in &new_issues {
                let day = std::env::var("DAY_COUNT")
                    .ok()
                    .and_then(|s| s.split_whitespace().next().map(|n| n.to_string()))
                    .unwrap_or_else(|| "?".to_string());
                let session = std::env::var("SESSION_COUNT")
                    .ok()
                    .unwrap_or_else(|| "?".to_string());
                let ack_msg = format!(
                    "Picked up in Day {day} Session {session} — I'll look at this in the next available cron window.",
                );
                match gh
                    .post_comment(
                        "coe0718/axonix",
                        u64::from(issue.number),
                        &ack_msg,
                    )
                    .await
                {
                    Ok(_) => {
                        acked_issues.insert(u64::from(issue.number));
                        let _ = acked_issues.save();
                        eprintln!(
                            "  ✓ acknowledged issue #{}",
                            issue.number
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "  ⚠ listener: failed to ack issue #{}: {e}",
                            issue.number
                        );
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("  ⚠ listener: github poll error: {e}");
        }
    }
}

/// Send the daily morning brief to Telegram.
///
/// Called once per day when the local hour matches `config.daily_brief_hour`.
pub(super) async fn send_daily_brief(tg: &TelegramClient) {
    let brief = crate::brief::Brief::collect();
    let msg = brief.format_telegram();
    match tg.send_message(&msg).await {
        Ok(_) => eprintln!("  ✓ listener: daily brief sent"),
        Err(e) => eprintln!("  ⚠ listener: failed to send daily brief: {e}"),
    }
}
