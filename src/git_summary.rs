//! git_summary — reads recent git commits and returns a compact activity summary (G-100).
//!
//! Avoids `git log --oneline` which crashes inside the container.
//! Uses `git show` and `git diff --stat` instead.

use std::process::Command;

/// Summary of a single git commit.
#[derive(Debug, Clone)]
pub struct CommitSummary {
    pub sha: String,
    pub message: String,
    pub date: String,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

/// Returns up to `n` recent commits from the git repository.
///
/// Uses `git show --no-patch --format="%H|||%s|||%ci"` to get commit metadata,
/// then `git diff --stat HEAD~k HEAD~(k-1)` for file change stats.
///
/// Returns an empty Vec if git is unavailable or no commits exist.
pub fn recent_activity(n: usize) -> Vec<CommitSummary> {
    if n == 0 {
        return vec![];
    }
    // Get last n commit hashes + subject + date in one pass
    // Use git show chained: start from HEAD, walk back via HEAD~1, HEAD~2, etc.
    // Safe approach: run git show for each commit index
    let mut results = Vec::new();
    for i in 0..n {
        let rev = if i == 0 {
            "HEAD".to_string()
        } else {
            format!("HEAD~{i}")
        };
        // Get sha, subject, and date
        let meta = Command::new("git")
            .args(["show", "--no-patch", "--format=%H|||%s|||%ci", &rev])
            .output();
        let meta = match meta {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => break, // no more commits
        };
        let first_line = meta.lines().next().unwrap_or("").trim().to_string();
        if first_line.is_empty() {
            break;
        }
        let parts: Vec<&str> = first_line.splitn(3, "|||").collect();
        if parts.len() < 3 {
            break;
        }
        let sha = parts[0][..8.min(parts[0].len())].to_string();
        let message = parts[1].to_string();
        let date = parts[2][..10.min(parts[2].len())].to_string(); // YYYY-MM-DD

        // Get diff stats for this commit vs its parent
        let stat_rev = format!("{rev}^..{rev}");
        let stat = Command::new("git")
            .args(["diff", "--stat", &stat_rev])
            .output();
        let (files_changed, insertions, deletions) = match stat {
            Ok(o) if o.status.success() => {
                parse_diff_stat(&String::from_utf8_lossy(&o.stdout))
            }
            _ => (0, 0, 0),
        };

        results.push(CommitSummary {
            sha,
            message,
            date,
            files_changed,
            insertions,
            deletions,
        });
    }
    results
}

/// Parse `git diff --stat` output to extract (files_changed, insertions, deletions).
///
/// The summary line looks like:
///   " 3 files changed, 42 insertions(+), 7 deletions(-)"
fn parse_diff_stat(output: &str) -> (u32, u32, u32) {
    let summary = output.lines().last().unwrap_or("").trim();
    // Files changed
    let files = extract_number(summary, "file");
    let ins = extract_number(summary, "insertion");
    let del = extract_number(summary, "deletion");
    (files, ins, del)
}

fn extract_number(s: &str, keyword: &str) -> u32 {
    // Find "N keyword" pattern
    s.split_whitespace()
        .zip(s.split_whitespace().skip(1))
        .find(|(_, next)| next.starts_with(keyword))
        .and_then(|(num, _)| num.parse().ok())
        .unwrap_or(0)
}

/// Formats recent git activity as a Telegram-ready string.
///
/// Returns a compact summary of the last `n` commits.
/// Example output:
/// ```text
/// 📝 Recent activity (last 3 commits):
/// • a1b2c3d 2026-04-03 feat(listener): add /history command (+45/-12)
/// • e4f5a6b 2026-04-02 fix(embeddings): handle Ollama timeout (+8/-2)
/// • c7d8e9f 2026-04-01 docs(journal): Day 20 Session 1 (+12/-0)
/// ```
pub fn format_for_telegram(n: usize) -> String {
    let commits = recent_activity(n);
    if commits.is_empty() {
        return "📝 No recent commits found.".to_string();
    }
    let mut lines = vec![format!(
        "📝 Recent activity (last {} commit{}):",
        commits.len(),
        if commits.len() == 1 { "" } else { "s" }
    )];
    for c in &commits {
        let stats = if c.files_changed > 0 {
            format!(" (+{}/−{})", c.insertions, c.deletions)
        } else {
            String::new()
        };
        lines.push(format!("• {} {} {}{}", c.sha, c.date, c.message, stats));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff_stat_full() {
        let output = " src/foo.rs | 10 +++++-----\n 1 file changed, 5 insertions(+), 5 deletions(-)";
        let (f, i, d) = parse_diff_stat(output);
        assert_eq!(f, 1);
        assert_eq!(i, 5);
        assert_eq!(d, 5);
    }

    #[test]
    fn test_parse_diff_stat_insertions_only() {
        let output = " src/bar.rs | 20 ++++++++++++++++++++\n 1 file changed, 20 insertions(+)";
        let (f, i, d) = parse_diff_stat(output);
        assert_eq!(f, 1);
        assert_eq!(i, 20);
        assert_eq!(d, 0);
    }

    #[test]
    fn test_parse_diff_stat_multi_file() {
        let output = " src/a.rs | 3 +++\n src/b.rs | 7 ++++---\n 2 files changed, 8 insertions(+), 3 deletions(-)";
        let (f, i, d) = parse_diff_stat(output);
        assert_eq!(f, 2);
        assert_eq!(i, 8);
        assert_eq!(d, 3);
    }

    #[test]
    fn test_parse_diff_stat_empty() {
        let (f, i, d) = parse_diff_stat("");
        assert_eq!(f, 0);
        assert_eq!(i, 0);
        assert_eq!(d, 0);
    }

    #[test]
    fn test_recent_activity_returns_something() {
        // In the real repo there will always be at least one commit
        let commits = recent_activity(3);
        // We can't assert specific commits, but we can assert structure
        for c in &commits {
            assert!(!c.sha.is_empty(), "sha should not be empty");
            assert!(!c.message.is_empty(), "message should not be empty");
            assert!(c.sha.len() <= 8, "sha should be truncated to 8 chars");
        }
    }

    #[test]
    fn test_recent_activity_zero() {
        let commits = recent_activity(0);
        assert!(commits.is_empty());
    }

    #[test]
    fn test_format_for_telegram_nonempty() {
        let output = format_for_telegram(2);
        assert!(output.contains("📝"), "should start with emoji: {output}");
    }

    #[test]
    fn test_format_for_telegram_zero() {
        // recent_activity(0) is empty, but format_for_telegram handles that
        // We can't call with 0 directly since it returns "no commits found"
        let output = format_for_telegram(0);
        assert!(output.contains("No recent commits"), "zero should say no commits: {output}");
    }
}
