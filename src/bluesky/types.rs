//! Plain data types for Bluesky AT Protocol integration.

/// A single recorded Bluesky post.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlueskyPostRecord {
    /// ISO 8601 UTC timestamp of when it was posted.
    pub created_at: String,
    /// The text content of the post.
    pub text: String,
    /// The AT Protocol URI (e.g. `at://did:.../app.bsky.feed.post/...`).
    pub uri: String,
    /// The CID of the post.
    pub cid: String,
    /// Whether this was a reply in a thread (true) or a root post (false).
    pub is_reply: bool,
}
