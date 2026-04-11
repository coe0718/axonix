//! HTTP client methods for GitHub API: post_comment, close_issue, list_issues.

use super::types::{GitHubIdentity, IssueEntry};

/// GitHub API client for Axonix.
///
/// Prefers AXONIX_BOT_TOKEN over GH_TOKEN.
#[derive(Clone)]
pub struct GitHubClient {
    pub(super) token: String,
    pub identity: GitHubIdentity,
    pub(super) client: reqwest::Client,
}

impl GitHubClient {
    /// Create a client with an explicit token and identity.
    pub fn new(token: impl Into<String>, identity: GitHubIdentity) -> Self {
        Self {
            token: token.into(),
            identity,
            client: crate::http_client::get(),
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

    /// Close a GitHub issue.
    ///
    /// `repo` should be in `owner/name` format (e.g., `"coe0718/axonix"`).
    /// `issue_number` is the issue number to close.
    ///
    /// Returns `Ok(())` on success.
    pub async fn close_issue(
        &self,
        repo: &str,
        issue_number: u64,
    ) -> Result<(), String> {
        let url = format!(
            "https://api.github.com/repos/{repo}/issues/{issue_number}"
        );

        let res = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "axonix-bot/1.0")
            .json(&serde_json::json!({ "state": "closed", "state_reason": "completed" }))
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(format!("GitHub API error {status}: {body}"));
        }

        Ok(())
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
