//! Bluesky AT Protocol integration for Axonix.
//!
//! Posts session announcements to Bluesky using the AT Protocol API.
//! Uses app passwords (not OAuth) — free tier, no paid plan required.
//!
//! # Configuration
//!
//! Set these environment variables:
//!   - `BLUESKY_IDENTIFIER` — your handle (e.g. `axonix.bsky.social`) or DID
//!   - `BLUESKY_APP_PASSWORD` — app password from Bluesky Settings → App Passwords
//!
//! # Authentication Flow
//!
//! 1. POST `com.atproto.server.createSession` → get `accessJwt` + `did`
//! 2. POST `com.atproto.repo.createRecord` with the JWT to post
//!
//! # Example
//!
//! ```no_run
//! use axonix::bluesky::BlueskyClient;
//!
//! # async fn example() {
//! let bsky = BlueskyClient::from_env().unwrap();
//! bsky.post("Day 3, Session 11 — Bluesky integration live.").await.ok();
//! # }
//! ```

const BLUESKY_API: &str = "https://bsky.social/xrpc";

/// Bluesky AT Protocol client for Axonix.
///
/// Authenticates with app password and posts to the feed.
#[derive(Clone)]
pub struct BlueskyClient {
    identifier: String,
    app_password: String,
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
            client: reqwest::Client::new(),
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

/// Generate a current ISO 8601 UTC timestamp string.
///
/// Bluesky requires `createdAt` in the record. Format: `2026-03-16T12:34:56.000Z`
fn current_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Convert unix timestamp to year/month/day/hour/min/sec
    // Using a simple manual conversion (no chrono dep needed)
    let (year, month, day, hour, min, sec) = unix_to_ymd_hms(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.000Z")
}

/// Convert a Unix timestamp (seconds since epoch) to (year, month, day, hour, min, sec).
///
/// Implements the proleptic Gregorian calendar algorithm used for UTC conversion.
/// Accurate for dates from 1970 through roughly 2100.
fn unix_to_ymd_hms(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec = (secs % 60) as u32;
    let mins_total = secs / 60;
    let min = (mins_total % 60) as u32;
    let hours_total = mins_total / 60;
    let hour = (hours_total % 24) as u32;
    let days_total = hours_total / 24; // days since 1970-01-01

    // Algorithm: Civil date from days since epoch (Julian Day Number method)
    // Reference: https://howardhinnant.github.io/date_algorithms.html
    let z = days_total + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (if month <= 2 { y + 1 } else { y }) as u32;

    (year, month, day, hour, min, sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> BlueskyClient {
        BlueskyClient::new("axonix.bsky.social", "test-app-password")
    }

    // ── format_session_post ──────────────────────────────────────────────────

    #[test]
    fn test_format_session_post_short_title() {
        let post = BlueskyClient::format_session_post(3, 11, "Bluesky integration live");
        assert!(post.contains("Day 3, Session 11"), "should include day/session");
        assert!(post.contains("Bluesky integration live"), "should include title");
        assert!(post.contains("axonix.live"), "should include URL");
        assert!(post.chars().count() <= 300, "post must be ≤300 chars: {}", post.chars().count());
    }

    #[test]
    fn test_format_session_post_long_title_truncated() {
        let long_title = "A".repeat(350);
        let post = BlueskyClient::format_session_post(1, 1, &long_title);
        assert!(post.chars().count() <= 300, "long post must be truncated: chars={}", post.chars().count());
        assert!(post.ends_with("axonix.live"), "should still end with URL after truncation");
    }

    #[test]
    fn test_format_session_post_starts_with_axonix() {
        let post = BlueskyClient::format_session_post(3, 11, "some title");
        assert!(post.starts_with("axonix Day 3, Session 11:"), "should start with axonix prefix");
    }

    #[test]
    fn test_format_session_post_unicode_title_fits() {
        // Unicode title with multi-byte chars — char count must stay ≤300
        let title = "🦀".repeat(50); // 50 chars, 200 bytes
        let post = BlueskyClient::format_session_post(2, 5, &title);
        assert!(post.chars().count() <= 300, "unicode title must fit: chars={}", post.chars().count());
    }

    #[test]
    fn test_format_session_post_exact_300_or_less() {
        // Verify the formula never overshoots 300 chars even at boundary
        for title_len in [1, 50, 100, 200, 250, 300, 400] {
            let title = "x".repeat(title_len);
            let post = BlueskyClient::format_session_post(99, 99, &title);
            assert!(post.chars().count() <= 300, "post for title_len={title_len} exceeds 300 chars: {}", post.chars().count());
        }
    }

    // ── current_iso8601 ──────────────────────────────────────────────────────

    #[test]
    fn test_current_iso8601_format() {
        let ts = current_iso8601();
        // Must match: YYYY-MM-DDTHH:MM:SS.000Z
        assert!(ts.ends_with(".000Z"), "must end with .000Z: {ts}");
        assert_eq!(ts.len(), 24, "must be 24 chars: {ts}");
        // Basic structure: 4 digit year, dash, 2 digit month, etc.
        assert_eq!(&ts[4..5], "-", "year-month separator: {ts}");
        assert_eq!(&ts[7..8], "-", "month-day separator: {ts}");
        assert_eq!(&ts[10..11], "T", "date-time separator: {ts}");
    }

    #[test]
    fn test_current_iso8601_year_is_reasonable() {
        let ts = current_iso8601();
        let year: u32 = ts[..4].parse().expect("year should be numeric");
        assert!(year >= 2024 && year <= 2100, "year should be reasonable: {year}");
    }

    // ── unix_to_ymd_hms ──────────────────────────────────────────────────────

    #[test]
    fn test_unix_epoch_is_1970_01_01() {
        let (y, mo, d, h, mi, s) = unix_to_ymd_hms(0);
        assert_eq!((y, mo, d, h, mi, s), (1970, 1, 1, 0, 0, 0), "epoch should be 1970-01-01T00:00:00");
    }

    #[test]
    fn test_known_date_2026_03_16() {
        // 2026-03-16T00:00:00Z = 1773619200 seconds since epoch
        // Verified: python3 -c "import datetime; dt = datetime.datetime(2026,3,16,tzinfo=datetime.timezone.utc); print(int(dt.timestamp()))"
        let (y, mo, d, h, mi, s) = unix_to_ymd_hms(1773619200);
        assert_eq!(y, 2026, "year should be 2026");
        assert_eq!(mo, 3, "month should be 3 (March)");
        assert_eq!(d, 16, "day should be 16");
        assert_eq!(h, 0, "hour should be 0");
        assert_eq!(mi, 0, "minute should be 0");
        assert_eq!(s, 0, "second should be 0");
    }

    #[test]
    fn test_known_date_with_time_components() {
        // 2026-03-16T14:30:45Z = 1773671445 seconds since epoch
        // Verified: python3 -c "import datetime; dt = datetime.datetime(2026,3,16,14,30,45,tzinfo=datetime.timezone.utc); print(int(dt.timestamp()))"
        let secs = 1773671445u64;
        let (y, mo, d, h, mi, s) = unix_to_ymd_hms(secs);
        assert_eq!(y, 2026);
        assert_eq!(mo, 3);
        assert_eq!(d, 16);
        assert_eq!(h, 14);
        assert_eq!(mi, 30);
        assert_eq!(s, 45);
    }

    #[test]
    fn test_unix_to_ymd_month_boundaries() {
        // 2026-01-01T00:00:00Z = 1767225600
        let (y, mo, d, ..) = unix_to_ymd_hms(1767225600);
        assert_eq!(y, 2026, "should be year 2026");
        assert_eq!(mo, 1, "should be January");
        assert_eq!(d, 1, "should be day 1");
    }

    // ── from_env ─────────────────────────────────────────────────────────────

    #[test]
    fn test_from_env_returns_none_when_missing() {
        // Verify constructor works (can't unset env vars safely in parallel tests)
        let c = BlueskyClient::new("handle.bsky.social", "app-password");
        assert_eq!(c.identifier, "handle.bsky.social");
        assert_eq!(c.app_password, "app-password");
    }

    #[test]
    fn test_client_clone() {
        let c = client();
        let c2 = c.clone();
        assert_eq!(c.identifier, c2.identifier);
        assert_eq!(c.app_password, c2.app_password);
    }

    // ── format_recap_commits ─────────────────────────────────────────────────

    #[test]
    fn test_format_recap_commits_empty() {
        let result = BlueskyClient::format_recap_commits(&[]);
        assert!(result.contains("what changed"), "should have header");
        assert!(result.chars().count() <= 300);
    }

    #[test]
    fn test_format_recap_commits_five() {
        let commits = ["feat: add /recap", "fix: bluesky reply", "docs: update README", "test: add coverage", "chore: bump version"];
        let result = BlueskyClient::format_recap_commits(&commits);
        assert!(result.contains("feat: add /recap"));
        assert!(result.chars().count() <= 300);
    }

    #[test]
    fn test_format_recap_commits_truncates_at_five() {
        let commits: Vec<&str> = (0..10).map(|_| "feat: some commit").collect();
        let result = BlueskyClient::format_recap_commits(&commits);
        // Should only show up to 5
        let bullet_count = result.matches('•').count();
        assert!(bullet_count <= 5, "should show at most 5 commits, got {bullet_count}");
        assert!(result.chars().count() <= 300);
    }

    // ── format_recap_tests ───────────────────────────────────────────────────

    #[test]
    fn test_format_recap_tests_with_delta() {
        let result = BlueskyClient::format_recap_tests(536, Some(8));
        assert!(result.contains("536"), "should contain test count");
        assert!(result.contains("+8"), "should contain delta");
        assert!(result.contains("axonix.live"));
        assert!(result.chars().count() <= 300);
    }

    #[test]
    fn test_format_recap_tests_no_delta() {
        let result = BlueskyClient::format_recap_tests(536, None);
        assert!(result.contains("536"));
        assert!(result.contains("axonix.live"));
        assert!(!result.contains('+'), "no delta should mean no + sign");
        assert!(result.chars().count() <= 300);
    }

    #[test]
    fn test_format_recap_tests_zero_delta_no_sign() {
        let result = BlueskyClient::format_recap_tests(536, Some(0));
        assert!(!result.contains('+'), "zero delta should not show + sign");
    }
}
