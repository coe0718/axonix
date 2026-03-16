//! Twitter API integration for Axonix.
//!
//! Posts session announcements and updates to Twitter using the v2 API.
//! Uses OAuth 1.0a (HMAC-SHA1) for authenticated requests.
//!
//! # Configuration
//!
//! Set these environment variables:
//!   - `TWITTER_API_KEY`       — consumer key
//!   - `TWITTER_API_SECRET`    — consumer secret
//!   - `TWITTER_ACCESS_TOKEN`  — user access token
//!   - `TWITTER_ACCESS_SECRET` — user access token secret
//!   - `TWITTER_BEARER_TOKEN`  — bearer token (for read-only ops, optional)
//!
//! # Example
//!
//! ```no_run
//! use axonix::twitter::TwitterClient;
//!
//! # async fn example() {
//! let tw = TwitterClient::from_env().unwrap();
//! tw.tweet("Day 2, Session 11 — Axonix adds goals to its dashboard.").await.ok();
//! # }
//! ```

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, Mac};
use rand::Rng;
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

const TWITTER_API_V2: &str = "https://api.twitter.com/2";

/// Twitter API client for Axonix.
///
/// Uses OAuth 1.0a with HMAC-SHA1 for authenticated requests (posting tweets).
#[derive(Clone)]
pub struct TwitterClient {
    api_key: String,
    api_secret: String,
    access_token: String,
    access_secret: String,
    client: reqwest::Client,
}

impl TwitterClient {
    /// Create a client from environment variables.
    ///
    /// Requires all four OAuth 1.0a credentials:
    /// TWITTER_API_KEY, TWITTER_API_SECRET, TWITTER_ACCESS_TOKEN, TWITTER_ACCESS_SECRET.
    /// Returns `None` if any is missing or empty.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("TWITTER_API_KEY").ok().filter(|s| !s.is_empty())?;
        let api_secret = std::env::var("TWITTER_API_SECRET").ok().filter(|s| !s.is_empty())?;
        let access_token = std::env::var("TWITTER_ACCESS_TOKEN").ok().filter(|s| !s.is_empty())?;
        let access_secret = std::env::var("TWITTER_ACCESS_SECRET").ok().filter(|s| !s.is_empty())?;
        Some(Self::new(api_key, api_secret, access_token, access_secret))
    }

    /// Create a client with explicit credentials.
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        access_token: impl Into<String>,
        access_secret: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            access_token: access_token.into(),
            access_secret: access_secret.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Post a tweet. Returns the tweet ID on success.
    ///
    /// Text must be ≤ 280 characters. This function does NOT truncate —
    /// callers should use `format_tweet` to ensure compliance.
    pub async fn tweet(&self, text: &str) -> Result<String, String> {
        let url = format!("{TWITTER_API_V2}/tweets");
        let body = serde_json::json!({ "text": text });
        let body_str = body.to_string();

        let auth_header = self.oauth_header("POST", &url, &[])?;

        let res = self.client
            .post(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await
            .map_err(|e| format!("Twitter API request failed: {e}"))?;

        let status = res.status();
        let response_body = res.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!("Twitter API error {status}: {response_body}"));
        }

        let json: serde_json::Value = serde_json::from_str(&response_body)
            .map_err(|e| format!("Twitter response parse error: {e}"))?;

        let tweet_id = json
            .get("data")
            .and_then(|d| d.get("id"))
            .and_then(|id| id.as_str())
            .unwrap_or("(unknown)")
            .to_string();

        Ok(tweet_id)
    }

    /// Format a session announcement tweet.
    ///
    /// Produces a ≤280 char tweet from session metadata.
    /// Title is truncated if needed to fit.
    pub fn format_session_tweet(day: u32, session: u32, title: &str) -> String {
        let prefix = format!("axonix Day {day}, Session {session}: ");
        let suffix = " — axonix.dev";
        // -1 for the ellipsis character we'll append on truncation
        let max_title_chars = 280 - prefix.chars().count() - suffix.chars().count();

        let title_chars: Vec<char> = title.chars().collect();
        let trimmed_title = if title_chars.len() <= max_title_chars {
            title.to_string()
        } else {
            // Truncate leaving room for ellipsis
            let truncate_at = max_title_chars.saturating_sub(1);
            let truncated: String = title_chars[..truncate_at].iter().collect();
            format!("{truncated}…")
        };

        format!("{prefix}{trimmed_title}{suffix}")
    }

    /// Build an OAuth 1.0a Authorization header for the given request.
    ///
    /// `params` are additional query/body parameters to include in the
    /// base string (for OAuth signature). For a JSON body POST, this is empty.
    fn oauth_header(
        &self,
        method: &str,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<String, String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("time error: {e}"))?
            .as_secs()
            .to_string();

        let nonce = generate_nonce();

        let mut oauth_params: Vec<(&str, String)> = vec![
            ("oauth_consumer_key", self.api_key.clone()),
            ("oauth_nonce", nonce.clone()),
            ("oauth_signature_method", "HMAC-SHA1".to_string()),
            ("oauth_timestamp", timestamp.clone()),
            ("oauth_token", self.access_token.clone()),
            ("oauth_version", "1.0".to_string()),
        ];

        // Collect all params for signature: oauth params + request params
        let mut all_params: Vec<(String, String)> = oauth_params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (k, v) in params {
            all_params.push((k.to_string(), v.to_string()));
        }

        let signature = self.compute_signature(method, url, &all_params)?;
        oauth_params.push(("oauth_signature", signature));

        // Build the Authorization header value
        let header_parts: Vec<String> = oauth_params
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", percent_encode(k), percent_encode(v)))
            .collect();

        Ok(format!("OAuth {}", header_parts.join(", ")))
    }

    /// Compute the HMAC-SHA1 OAuth signature.
    fn compute_signature(
        &self,
        method: &str,
        url: &str,
        params: &[(String, String)],
    ) -> Result<String, String> {
        // Sort and encode all params
        let mut sorted: Vec<(String, String)> = params
            .iter()
            .map(|(k, v)| (percent_encode(k), percent_encode(v)))
            .collect();
        sorted.sort();

        let param_string = sorted
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let base_string = format!(
            "{}&{}&{}",
            percent_encode(method),
            percent_encode(url),
            percent_encode(&param_string)
        );

        let signing_key = format!(
            "{}&{}",
            percent_encode(&self.api_secret),
            percent_encode(&self.access_secret)
        );

        let mut mac = HmacSha1::new_from_slice(signing_key.as_bytes())
            .map_err(|e| format!("HMAC init error: {e}"))?;
        mac.update(base_string.as_bytes());
        let result = mac.finalize().into_bytes();

        Ok(BASE64.encode(result))
    }
}

/// Generate a random OAuth nonce (32 hex chars).
fn generate_nonce() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Percent-encode a string per RFC 3986.
///
/// Encodes all characters except unreserved: A-Z a-z 0-9 - _ . ~
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0').to_ascii_uppercase());
                out.push(char::from_digit((byte & 0xf) as u32, 16).unwrap_or('0').to_ascii_uppercase());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> TwitterClient {
        TwitterClient::new("api_key", "api_secret", "access_token", "access_secret")
    }

    // ── percent_encode ────────────────────────────────────────────────────────

    #[test]
    fn test_percent_encode_unreserved_chars_unchanged() {
        assert_eq!(percent_encode("hello-world_test.OK~"), "hello-world_test.OK~");
        assert_eq!(percent_encode("abc123"), "abc123");
    }

    #[test]
    fn test_percent_encode_space() {
        assert_eq!(percent_encode(" "), "%20");
    }

    #[test]
    fn test_percent_encode_ampersand() {
        assert_eq!(percent_encode("&"), "%26");
    }

    #[test]
    fn test_percent_encode_equals() {
        assert_eq!(percent_encode("="), "%3D");
    }

    #[test]
    fn test_percent_encode_slash() {
        assert_eq!(percent_encode("/"), "%2F");
    }

    #[test]
    fn test_percent_encode_plus() {
        assert_eq!(percent_encode("+"), "%2B");
    }

    #[test]
    fn test_percent_encode_colon() {
        assert_eq!(percent_encode(":"), "%3A");
    }

    #[test]
    fn test_percent_encode_empty() {
        assert_eq!(percent_encode(""), "");
    }

    #[test]
    fn test_percent_encode_url_components() {
        let url = "https://api.twitter.com/2/tweets";
        let encoded = percent_encode(url);
        assert!(encoded.contains("%3A"), "colon should be encoded");
        assert!(encoded.contains("%2F"), "slash should be encoded");
        assert!(!encoded.contains("://"), "unencoded :// should not appear");
    }

    // ── format_session_tweet ─────────────────────────────────────────────────

    #[test]
    fn test_format_session_tweet_short_title() {
        let tweet = TwitterClient::format_session_tweet(2, 11, "Add goals to dashboard");
        assert!(tweet.contains("Day 2, Session 11"), "should include day/session");
        assert!(tweet.contains("Add goals to dashboard"), "should include title");
        assert!(tweet.contains("axonix.dev"), "should include URL");
        // Twitter counts characters, not bytes
        assert!(tweet.chars().count() <= 280, "tweet must be ≤280 chars: chars={}", tweet.chars().count());
    }

    #[test]
    fn test_format_session_tweet_long_title_truncated() {
        let long_title = "A".repeat(300);
        let tweet = TwitterClient::format_session_tweet(1, 1, &long_title);
        // Twitter measures length in Unicode characters, not bytes.
        // "—" (em-dash) and "…" (ellipsis) are 1 char each, 3 bytes each.
        assert!(tweet.chars().count() <= 280, "long tweet must be truncated to ≤280 chars: chars={}", tweet.chars().count());
    }

    #[test]
    fn test_format_session_tweet_format() {
        let tweet = TwitterClient::format_session_tweet(2, 11, "Fixes issue #14");
        assert!(tweet.starts_with("axonix Day 2, Session 11:"), "should start with axonix prefix");
    }

    // ── oauth signature components ────────────────────────────────────────────

    #[test]
    fn test_compute_signature_deterministic_with_same_inputs() {
        // Same inputs → same HMAC output (HMAC is deterministic)
        let c = client();
        let params = vec![
            ("oauth_consumer_key".to_string(), "key".to_string()),
            ("oauth_timestamp".to_string(), "1234567890".to_string()),
        ];
        let sig1 = c.compute_signature("POST", "https://api.twitter.com/2/tweets", &params).unwrap();
        let sig2 = c.compute_signature("POST", "https://api.twitter.com/2/tweets", &params).unwrap();
        assert_eq!(sig1, sig2, "same inputs should produce same signature");
    }

    #[test]
    fn test_compute_signature_different_methods_differ() {
        let c = client();
        let params: Vec<(String, String)> = vec![];
        let sig_post = c.compute_signature("POST", "https://api.twitter.com/2/tweets", &params).unwrap();
        let sig_get = c.compute_signature("GET", "https://api.twitter.com/2/tweets", &params).unwrap();
        assert_ne!(sig_post, sig_get, "POST and GET should produce different signatures");
    }

    #[test]
    fn test_compute_signature_is_base64() {
        let c = client();
        let params: Vec<(String, String)> = vec![];
        let sig = c.compute_signature("POST", "https://api.twitter.com/2/tweets", &params).unwrap();
        // Base64 characters: A-Z a-z 0-9 + / =
        assert!(
            sig.chars().all(|ch| ch.is_alphanumeric() || ch == '+' || ch == '/' || ch == '='),
            "signature must be valid base64: {sig}"
        );
    }

    // ── nonce ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_nonce_length() {
        let n = generate_nonce();
        assert_eq!(n.len(), 32, "nonce should be 32 hex chars");
    }

    #[test]
    fn test_nonce_is_hex() {
        let n = generate_nonce();
        assert!(n.chars().all(|c| c.is_ascii_hexdigit()), "nonce must be hex: {n}");
    }

    #[test]
    fn test_nonces_are_unique() {
        // Very unlikely to collide; if this fails, the RNG is broken
        let n1 = generate_nonce();
        let n2 = generate_nonce();
        assert_ne!(n1, n2, "two nonces should not be equal");
    }

    // ── oauth_header ──────────────────────────────────────────────────────────

    #[test]
    fn test_oauth_header_structure() {
        let c = client();
        let header = c.oauth_header("POST", "https://api.twitter.com/2/tweets", &[]).unwrap();
        assert!(header.starts_with("OAuth "), "header must start with 'OAuth '");
        assert!(header.contains("oauth_consumer_key="), "must contain consumer key");
        assert!(header.contains("oauth_signature="), "must contain signature");
        assert!(header.contains("oauth_timestamp="), "must contain timestamp");
        assert!(header.contains("oauth_nonce="), "must contain nonce");
        assert!(header.contains("oauth_signature_method=\"HMAC-SHA1\""), "must specify HMAC-SHA1");
    }

    #[test]
    fn test_oauth_header_quoted_values() {
        let c = client();
        let header = c.oauth_header("POST", "https://api.twitter.com/2/tweets", &[]).unwrap();
        // All OAuth param values must be quoted
        let parts: Vec<&str> = header.trim_start_matches("OAuth ").split(", ").collect();
        for part in &parts {
            assert!(
                part.contains("=\"") && part.ends_with('"'),
                "each part must have quoted value: {part}"
            );
        }
    }
}
