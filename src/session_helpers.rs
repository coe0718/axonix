//! Session helper functions: journal title, recent commits, Telegram summary, test count.
//!
//! Extracted from main.rs (G-123) to keep the binary entry point focused.

/// Read the latest journal entry title from JOURNAL.md.
pub fn read_journal_title() -> Option<String> {
    let content = std::fs::read_to_string("JOURNAL.md").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Get recent commit subjects (first line of commit message, no author info).
pub fn get_recent_commits(n: usize) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["log", "--format=%s", &format!("-{n}")])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        }
        _ => vec![],
    }
}

/// Format a CycleSummary as a compact Telegram message for --session-summary-telegram (Closes #46).
pub fn format_session_summary_telegram(summary: &axonix::cycle_summary::CycleSummary) -> String {
    match &summary.data {
        None => "📊 Axonix Session Summary\n(no summary data found — run --write-summary first)".to_string(),
        Some(data) => {
            let mut msg = format!("📊 Axonix Session Summary: {}\n", data.session);
            msg.push_str(&format!("✅ Completed ({}):\n", data.completed.len()));
            if data.completed.is_empty() {
                msg.push_str("  (none)\n");
            } else {
                for item in data.completed.iter().take(10) {
                    msg.push_str(&format!("  • {item}\n"));
                }
            }
            msg.push_str(&format!("⏳ Pending ({}):\n", data.pending.len()));
            if data.pending.is_empty() {
                msg.push_str("  (none)\n");
            } else {
                for item in data.pending.iter().take(10) {
                    msg.push_str(&format!("  • {item}\n"));
                }
            }
            let files = data.changed_files.len();
            let test_str = data.test_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".to_string());
            msg.push_str(&format!("🧪 Tests: {test_str} | Files: {files}"));
            msg
        }
    }
}

/// Get the current test count by running cargo test --quiet.
/// Returns None if the test run fails or output is unparseable.
pub fn get_test_count() -> Option<u32> {
    let output = std::process::Command::new("cargo")
        .args(["test", "--quiet"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Look in both stdout and stderr for "N passed"
    for line in text.lines().chain(stderr.lines()) {
        if line.contains("passed") {
            // "test result: ok. N passed" — extract N
            if let Some(n_str) = line.split_whitespace()
                .skip_while(|w| *w != "ok.")
                .nth(1)
            {
                if let Ok(n) = n_str.parse::<u32>() {
                    return Some(n);
                }
            }
        }
    }
    None
}
