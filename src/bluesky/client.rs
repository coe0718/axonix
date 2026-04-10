//! `BlueskyClient` — AT Protocol client for authenticating and posting to Bluesky.

use crate::bluesky::helpers::current_iso8601;
use crate::bluesky::history::BlueskyHistory;

pub(super) const BLUESKY_API: &str = "https://bsky.social/xrpc";

/// Bluesky AT Protocol client for Axonix.
///
/// Authenticates with app password and posts to the feed.
#[derive(Clone)]
pub struct BlueskyClient {
    pub(super) identifier: String,
    pub(super) app_password: String,
    client: reqwest::Client,
}

impl BlueskyClient {
    /// Create a client from environment variables.
    ///
    /// Reads `BLUESKY_IDENTIFIER` and `BLUESKY_APP_PASSWORD`.
    /// Returns `None` if either is missing or empty.
    pub fn from_env() -> Option<Self> {
        let identifier = std::env::var("BLUESKY_IDENTIFIER")
            .ok()
            .filter(|s| !s.is_empty())?;
        let app_password = std::env::var("BLUESKY_APP_PASSWORD")
            .ok()
            .filter(|s| !s.is_empty())?;
        Some(Self::new(identifier, app_password))
    }

    /// Create a client with explicit credentials.
    pub fn new(
        identifier: impl Into<String>,
        app_password: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            app_password: app_password.into(),
            client: crate::http_client::get(),
        }
    }

    /// Authenticate and get an access JWT + DID.
    ///
    /// Returns `(access_jwt, did)` on success.
    async fn create_session(&self) -> Result<(String, String), String> {
        let url = format!("{BLUESKY_API}/com.atproto.server.createSession");
        let res = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "identifier": self.identifier,
                "password": self.app_password,
            }))
            .send()
            .await
            .map_err(|e| format!("Bluesky auth request failed: {e}"))?;

        let status = res.status();
        let body = res.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!("Bluesky auth error {status}: {body}"));
        }

        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("Bluesky auth parse error: {e}: {body}"))?;

        let access_jwt = json
            .get("accessJwt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Bluesky auth: missing accessJwt in response: {body}"))?
            .to_string();

        let did = json
            .get("did")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Bluesky auth: missing did in response: {body}"))?
            .to_string();

        Ok((access_jwt, did))
    }

    /// Post text to the Bluesky feed. Returns `(uri, cid)` on success.
    ///
    /// Text must be ≤ 300 grapheme clusters (Bluesky's limit).
    /// This function does NOT truncate — use `format_post` for safe formatting.
    pub async fn post(&self, text: &str) -> Result<(String, String), String> {
        let (access_jwt, did) = self.create_session().await?;

        let url = format!("{BLUESKY_API}/com.atproto.repo.createRecord");

        // Bluesky requires an ISO 8601 timestamp
        let created_at = current_iso8601();

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {access_jwt}"))
            .json(&serde_json::json!({
                "repo": did,
                "collection": "app.bsky.feed.post",
                "record": {
                    "$type": "app.bsky.feed.post",
                    "text": text,
                    "createdAt": created_at,
                },
            }))
            .send()
            .await
            .map_err(|e| format!("Bluesky post request failed: {e}"))?;

        let status = res.status();
        let body = res.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!("Bluesky post error {status}: {body}"));
        }

        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("Bluesky post parse error: {e}"))?;

        let uri = json
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string();

        let cid = json
            .get("cid")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string();

        let mut history = BlueskyHistory::default_path();
        let ts = current_iso8601();
        history.record(&ts, text, &uri, &cid, false);
        history.save().ok(); // non-fatal — don't fail the post if history write fails

        Ok((uri, cid))
    }

    /// Post a reply in a Bluesky thread. Returns `(uri, cid)` on success.
    ///
    /// `root_uri` and `root_cid` are the root post of the thread.
    /// `parent_uri` and `parent_cid` are the immediate parent (same as root for first reply).
    pub async fn post_reply(
        &self,
        text: &str,
        root_uri: &str,
        root_cid: &str,
        parent_uri: &str,
        parent_cid: &str,
    ) -> Result<(String, String), String> {
        let (access_jwt, did) = self.create_session().await?;

        let url = format!("{BLUESKY_API}/com.atproto.repo.createRecord");
        let created_at = current_iso8601();

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {access_jwt}"))
            .json(&serde_json::json!({
                "repo": did,
                "collection": "app.bsky.feed.post",
                "record": {
                    "$type": "app.bsky.feed.post",
                    "text": text,
                    "createdAt": created_at,
                    "reply": {
                        "root": { "uri": root_uri, "cid": root_cid },
                        "parent": { "uri": parent_uri, "cid": parent_cid }
                    }
                },
            }))
            .send()
            .await
            .map_err(|e| format!("Bluesky reply request failed: {e}"))?;

        let status = res.status();
        let body = res.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!("Bluesky reply error {status}: {body}"));
        }

        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("Bluesky reply parse error: {e}"))?;

        let uri = json
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string();

        let cid = json
            .get("cid")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string();

        let mut history = BlueskyHistory::default_path();
        let ts = current_iso8601();
        history.record(&ts, text, &uri, &cid, true);
        history.save().ok(); // non-fatal — don't fail the post if history write fails

        Ok((uri, cid))
    }

    /// Format the "what changed" post body from a list of commit subjects.
    ///
    /// Returns a ≤300 char post. Includes "what changed:\n" prefix.
    /// Shows up to 5 commits as bullet points.
    pub fn format_recap_commits(commits: &[&str]) -> String {
        let prefix = "what changed:\n";
        let mut lines: Vec<String> = commits
            .iter()
            .take(5)
            .map(|s| format!("• {s}"))
            .collect();

        // Truncate to fit within 300 chars total
        let mut result = format!("{prefix}{}", lines.join("\n"));
        while result.chars().count() > 300 && !lines.is_empty() {
            lines.pop();
            result = format!("{prefix}{}", lines.join("\n"));
        }
        // If still over (edge case with very long single commit), truncate last item
        if result.chars().count() > 300 {
            let allowed: String = result.chars().take(297).collect();
            format!("{allowed}…")
        } else {
            result
        }
    }

    /// Format the "tests" post body from test count and optional delta.
    ///
    /// Returns a ≤300 char post.
    /// Format: "tests: N passing (+M this session)\naxonix.live"
    /// If delta is None or 0: "tests: N passing\naxonix.live"
    pub fn format_recap_tests(test_count: u32, delta: Option<i32>) -> String {
        let count_str = match delta {
            Some(d) if d != 0 => {
                let sign = if d > 0 { "+" } else { "" };
                format!("tests: {test_count} passing ({sign}{d} this session)\naxonix.live")
            }
            _ => format!("tests: {test_count} passing\naxonix.live"),
        };
        if count_str.chars().count() > 300 {
            let truncated: String = count_str.chars().take(297).collect();
            format!("{truncated}…")
        } else {
            count_str
        }
    }

    /// Format a session announcement post for Bluesky.
    ///
    /// Produces a ≤300 char post from session metadata.
    /// Title is truncated if needed to fit.
    pub fn format_session_post(day: u32, session: u32, title: &str) -> String {
        let prefix = format!("axonix Day {day}, Session {session}: ");
        let suffix = " — axonix.live";
        let max_title_chars = 300 - prefix.chars().count() - suffix.chars().count();

        let title_chars: Vec<char> = title.chars().collect();
        let trimmed_title = if title_chars.len() <= max_title_chars {
            title.to_string()
        } else {
            let truncate_at = max_title_chars.saturating_sub(1);
            let truncated: String = title_chars[..truncate_at].iter().collect();
            format!("{truncated}…")
        };

        format!("{prefix}{trimmed_title}{suffix}")
    }
}


#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
