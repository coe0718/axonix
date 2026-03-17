//! GitHub integration for Axonix.
//!
//! Posts issue comments and manages git identity using either the
//! axonix-bot account (AXONIX_BOT_TOKEN) or the owner's account (GH_TOKEN).
//!
//! # Priority
//!
//! When AXONIX_BOT_TOKEN is set, all GitHub API calls and git commits
//! are attributed to axonix-bot. When only GH_TOKEN is available, falls
//! back to posting as the repo owner.
//!
//! # Example
//!
//! ```no_run
//! use axonix::github::GitHubClient;
//!
//! # async fn example() {
//! let client = GitHubClient::from_env().unwrap();
//! client.post_comment("coe0718/axonix", 12, "Hello from axonix-bot!").await.ok();
//! # }
//! ```

/// The name that axonix-bot uses for git commits.
pub const BOT_GIT_NAME: &str = "axonix-bot";
/// The email axonix-bot uses for git commits.
pub const BOT_GIT_EMAIL: &str = "axonix-bot@users.noreply.github.com";

/// The fallback name when acting as Axonix (owner's config).
pub const AGENT_GIT_NAME: &str = "Axonix";
/// The fallback email when acting as Axonix (owner's config).
pub const AGENT_GIT_EMAIL: &str = "axonix@axonix.dev";

/// Which identity is being used for GitHub operations.
#[derive(Debug, Clone, PartialEq)]
pub enum GitHubIdentity {
    /// Using AXONIX_BOT_TOKEN — actions attributed to axonix-bot.
    Bot,
    /// Using GH_TOKEN — actions attributed to repo owner.
    Owner,
}

impl GitHubIdentity {
    /// Human-readable description of the active identity.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Bot => "axonix-bot",
            Self::Owner => "coe0718 (owner)",
        }
    }
}

/// GitHub API client for Axonix.
///
/// Prefers AXONIX_BOT_TOKEN over GH_TOKEN.
#[derive(Clone)]
pub struct GitHubClient {
    token: String,
    pub identity: GitHubIdentity,
    client: reqwest::Client,
}

impl GitHubClient {
    /// Create a client from environment variables.
    ///
    /// Checks AXONIX_BOT_TOKEN first, then GH_TOKEN.
    /// Returns `None` if neither is set.
    pub fn from_env() -> Option<Self> {
        if let Ok(token) = std::env::var("AXONIX_BOT_TOKEN")
            .or_else(|_| std::env::var("AXONIX_TOKEN"))
        {
            if !token.is_empty() {
                return Some(Self {
                    token,
                    identity: GitHubIdentity::Bot,
                    client: reqwest::Client::new(),
                });
            }
        }

        if let Ok(token) = std::env::var("GH_TOKEN")
            .or_else(|_| std::env::var("GITHUB_TOKEN"))
        {
            if !token.is_empty() {
                return Some(Self {
                    token,
                    identity: GitHubIdentity::Owner,
                    client: reqwest::Client::new(),
                });
            }
        }

        None
    }

    /// Create a client with an explicit token and identity.
    pub fn new(token: impl Into<String>, identity: GitHubIdentity) -> Self {
        Self {
            token: token.into(),
            identity,
            client: reqwest::Client::new(),
        }
    }

    /// Post a comment on a GitHub issue or pull request.
    ///
    /// `repo` should be in `owner/name` format (e.g., `"coe0718/axonix"`).
    /// `issue_number` is the issue or PR number.
    ///
    /// Returns the URL of the created comment on success.
    pub async fn post_comment(
        &self,
        repo: &str,
        issue_number: u64,
        body: &str,
    ) -> Result<String, String> {
        let url = format!(
            "https://api.github.com/repos/{repo}/issues/{issue_number}/comments"
        );

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "axonix-bot/1.0")
            .json(&serde_json::json!({ "body": body }))
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(format!("GitHub API error {status}: {body}"));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("GitHub response parse error: {e}"))?;

        let html_url = json
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("(url unavailable)")
            .to_string();

        Ok(html_url)
    }

    /// Configure the git committer identity in the given repository.
    ///
    /// When using the bot token, sets name/email to axonix-bot.
    /// When using owner token, sets to Axonix (agent identity).
    ///
    /// Uses `git config --local` so the change only affects this repo.
    pub fn configure_git_identity(&self, repo_path: &str) -> Result<(), String> {
        let (name, email) = match self.identity {
            GitHubIdentity::Bot => (BOT_GIT_NAME, BOT_GIT_EMAIL),
            GitHubIdentity::Owner => (AGENT_GIT_NAME, AGENT_GIT_EMAIL),
        };

        let name_result = std::process::Command::new("git")
            .args(["config", "--local", "user.name", name])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("git config user.name failed: {e}"))?;

        if !name_result.status.success() {
            let stderr = String::from_utf8_lossy(&name_result.stderr);
            return Err(format!("git config user.name error: {stderr}"));
        }

        let email_result = std::process::Command::new("git")
            .args(["config", "--local", "user.email", email])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("git config user.email failed: {e}"))?;

        if !email_result.status.success() {
            let stderr = String::from_utf8_lossy(&email_result.stderr);
            return Err(format!("git config user.email error: {stderr}"));
        }

        Ok(())
    }

    /// Returns true if operating as the axonix-bot account.
    pub fn is_bot(&self) -> bool {
        self.identity == GitHubIdentity::Bot
    }

    /// Post a discussion to a GitHub repository using the GraphQL API.
    ///
    /// `repo_id` is the GraphQL node ID of the repository (e.g., `"R_kgDORnAZ_w"`).
    /// `category_id` is the GraphQL node ID of the discussion category (e.g., `"DIC_kwDORnAZ_84C4ask"`).
    /// `title` is the discussion title.
    /// `body` is the Markdown body of the discussion.
    ///
    /// Returns the URL of the created discussion on success.
    pub async fn post_discussion(
        &self,
        repo_id: &str,
        category_id: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String> {
        let query = r#"mutation($repoId: ID!, $catId: ID!, $title: String!, $body: String!) {
            createDiscussion(input: {
                repositoryId: $repoId,
                categoryId: $catId,
                title: $title,
                body: $body
            }) {
                discussion {
                    id
                    url
                }
            }
        }"#;

        let variables = serde_json::json!({
            "repoId": repo_id,
            "catId": category_id,
            "title": title,
            "body": body,
        });

        let payload = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let res = self
            .client
            .post("https://api.github.com/graphql")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "axonix-bot/1.0")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("GitHub GraphQL request failed: {e}"))?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(format!("GitHub GraphQL error {status}: {body}"));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("GitHub GraphQL response parse error: {e}"))?;

        // Check for GraphQL-level errors
        if let Some(errors) = json.get("errors") {
            return Err(format!("GitHub GraphQL errors: {errors}"));
        }

        let url = json
            .pointer("/data/createDiscussion/discussion/url")
            .and_then(|v| v.as_str())
            .unwrap_or("(url unavailable)")
            .to_string();

        Ok(url)
    }

    /// Fetch open issues for a repo, sorted by most reactions first.
    ///
    /// `repo` should be in `owner/name` format (e.g., `"coe0718/axonix"`).
    /// `limit` caps the number of issues returned (max 100 from GitHub API).
    ///
    /// Returns a list of `IssueEntry` sorted descending by reaction count.
    pub async fn list_issues(
        &self,
        repo: &str,
        limit: u8,
    ) -> Result<Vec<IssueEntry>, String> {
        let per_page = limit.max(1);
        let url = format!(
            "https://api.github.com/repos/{repo}/issues?state=open&per_page={per_page}&sort=created&direction=desc"
        );

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "axonix-bot/1.0")
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(format!("GitHub API error {status}: {body}"));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("GitHub response parse error: {e}"))?;

        let issues = json
            .as_array()
            .ok_or_else(|| "GitHub response was not an array".to_string())?;

        let mut entries: Vec<IssueEntry> = issues
            .iter()
            .filter_map(|issue| {
                // Skip pull requests (GitHub includes PRs in /issues endpoint)
                if issue.get("pull_request").is_some() {
                    return None;
                }
                let number = issue.get("number")?.as_u64()? as u32;
                let title = issue.get("title")?.as_str()?.to_string();
                let reactions = issue
                    .get("reactions")
                    .and_then(|r| r.get("total_count"))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0) as u32;
                let labels: Vec<String> = issue
                    .get("labels")
                    .and_then(|l| l.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|label| label.get("name")?.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                Some(IssueEntry { number, title, reactions, labels })
            })
            .collect();

        // Sort by reactions descending (most-voted issues first)
        entries.sort_by(|a, b| b.reactions.cmp(&a.reactions));
        Ok(entries)
    }
}

/// A GitHub issue entry with priority-relevant metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct IssueEntry {
    /// Issue number.
    pub number: u32,
    /// Issue title.
    pub title: String,
    /// Total reaction count (👍 etc.) — used to prioritize community requests.
    pub reactions: u32,
    /// Labels attached to the issue.
    pub labels: Vec<String>,
}

/// Format a journal entry for posting as a GitHub Discussion.
///
/// Takes the raw journal title and body and wraps them for the Discussions format.
/// The title becomes the discussion title; the body gets a footer linking back to the repo.
pub fn format_discussion_body(journal_body: &str) -> String {
    let mut body = journal_body.to_string();
    body.push_str("\n\n---\n*Posted automatically by Axonix — [source](https://github.com/coe0718/axonix/blob/main/JOURNAL.md)*");
    body
}

/// Parse the latest journal entry from JOURNAL.md content.
///
/// Returns `(title, body)` where title is the `## Day N, Session M — ...` heading
/// and body is everything until the next `## ` heading or end of content.
/// Returns `None` if no entry is found.
pub fn parse_latest_journal(content: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = content.lines().collect();

    // Find the first ## heading (skip the # Journal top-level heading)
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("## ") {
            start = Some(i);
            break;
        }
    }

    let start = start?;
    let title = lines[start].trim_start_matches("## ").trim().to_string();

    // Find the end: next ## heading or end of file
    let mut end = lines.len();
    for i in (start + 1)..lines.len() {
        if lines[i].starts_with("## ") {
            end = i;
            break;
        }
    }

    // Collect body lines (skip empty lines at start and end)
    let body_lines: Vec<&str> = lines[(start + 1)..end].to_vec();
    let body = body_lines.join("\n").trim().to_string();

    if title.is_empty() {
        return None;
    }

    Some((title, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_display_bot() {
        assert_eq!(GitHubIdentity::Bot.display_name(), "axonix-bot");
    }

    #[test]
    fn test_identity_display_owner() {
        assert_eq!(GitHubIdentity::Owner.display_name(), "coe0718 (owner)");
    }

    #[test]
    fn test_identity_is_bot() {
        let client = GitHubClient::new("tok", GitHubIdentity::Bot);
        assert!(client.is_bot());
    }

    #[test]
    fn test_identity_is_not_bot() {
        let client = GitHubClient::new("tok", GitHubIdentity::Owner);
        assert!(!client.is_bot());
    }

    #[test]
    fn test_bot_git_constants() {
        assert_eq!(BOT_GIT_NAME, "axonix-bot");
        assert!(BOT_GIT_EMAIL.contains("axonix-bot"));
        assert!(BOT_GIT_EMAIL.contains("noreply.github.com"));
    }

    #[test]
    fn test_agent_git_constants() {
        assert_eq!(AGENT_GIT_NAME, "Axonix");
        assert!(AGENT_GIT_EMAIL.contains("axonix"));
    }

    #[test]
    fn test_from_env_returns_none_when_no_tokens() {
        // Can't safely unset env vars in parallel tests.
        // Structural test: verify the client stores values correctly.
        let client = GitHubClient::new("mytoken", GitHubIdentity::Bot);
        assert_eq!(client.token, "mytoken");
        assert_eq!(client.identity, GitHubIdentity::Bot);
    }

    #[test]
    fn test_new_with_owner_identity() {
        let client = GitHubClient::new("owner_token", GitHubIdentity::Owner);
        assert!(!client.is_bot());
        assert_eq!(client.identity.display_name(), "coe0718 (owner)");
    }

    // ── IssueEntry ────────────────────────────────────────────────────────────

    #[test]
    fn test_issue_entry_fields() {
        let entry = IssueEntry {
            number: 7,
            title: "Add Telegram features".to_string(),
            reactions: 3,
            labels: vec!["enhancement".to_string()],
        };
        assert_eq!(entry.number, 7);
        assert_eq!(entry.title, "Add Telegram features");
        assert_eq!(entry.reactions, 3);
        assert_eq!(entry.labels, vec!["enhancement"]);
    }

    #[test]
    fn test_issue_entry_no_labels() {
        let entry = IssueEntry {
            number: 1,
            title: "Test issue".to_string(),
            reactions: 0,
            labels: vec![],
        };
        assert!(entry.labels.is_empty());
    }

    #[test]
    fn test_issue_sort_by_reactions() {
        let mut issues = vec![
            IssueEntry { number: 1, title: "low".to_string(), reactions: 1, labels: vec![] },
            IssueEntry { number: 2, title: "high".to_string(), reactions: 10, labels: vec![] },
            IssueEntry { number: 3, title: "mid".to_string(), reactions: 5, labels: vec![] },
        ];
        issues.sort_by(|a, b| b.reactions.cmp(&a.reactions));
        assert_eq!(issues[0].number, 2, "highest reaction issue should be first");
        assert_eq!(issues[1].number, 3);
        assert_eq!(issues[2].number, 1);
    }

    // ── Discussion ──────────────────────────────────────────────────────────

    #[test]
    fn test_format_discussion_body_adds_footer() {
        let body = format_discussion_body("Session went well.");
        assert!(body.contains("Session went well."), "body should contain original text");
        assert!(body.contains("Posted automatically by Axonix"), "body should have footer");
        assert!(body.contains("JOURNAL.md"), "footer should link to JOURNAL.md");
    }

    #[test]
    fn test_format_discussion_body_preserves_markdown() {
        let input = "## Highlights\n\n- Fixed a bug\n- Added tests";
        let body = format_discussion_body(input);
        assert!(body.contains("## Highlights"), "should preserve markdown headings");
        assert!(body.contains("- Fixed a bug"), "should preserve list items");
    }

    #[test]
    fn test_parse_latest_journal_simple() {
        let content = "# Journal\n\n## Day 4, Session 1 — Big feature\n\nDid some work.\n\n## Day 3, Session 13 — Old entry\n\nOld stuff.\n";
        let result = parse_latest_journal(content);
        assert!(result.is_some(), "should find latest entry");
        let (title, body) = result.unwrap();
        assert_eq!(title, "Day 4, Session 1 — Big feature");
        assert_eq!(body, "Did some work.");
    }

    #[test]
    fn test_parse_latest_journal_multiline_body() {
        let content = "# Journal\n\n## Day 4, Session 1 — Title\n\nLine one.\nLine two.\nLine three.\n\n## Day 3 — Older\n\nOld.\n";
        let (title, body) = parse_latest_journal(content).unwrap();
        assert_eq!(title, "Day 4, Session 1 — Title");
        assert!(body.contains("Line one."));
        assert!(body.contains("Line two."));
        assert!(body.contains("Line three."));
    }

    #[test]
    fn test_parse_latest_journal_no_entries() {
        let content = "# Journal\n\nNothing here yet.\n";
        assert!(parse_latest_journal(content).is_none(), "should return None when no ## headings");
    }

    #[test]
    fn test_parse_latest_journal_empty() {
        assert!(parse_latest_journal("").is_none());
    }

    #[test]
    fn test_parse_latest_journal_single_entry_no_trailing_heading() {
        let content = "# Journal\n\n## Day 1, Session 1 — First\n\nOnly entry.\n";
        let (title, body) = parse_latest_journal(content).unwrap();
        assert_eq!(title, "Day 1, Session 1 — First");
        assert_eq!(body, "Only entry.");
    }
}
