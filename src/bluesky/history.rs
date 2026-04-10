//! `BlueskyHistory` — persisted post history for Bluesky.

use crate::bluesky::types::BlueskyPostRecord;

/// Persisted history of Bluesky posts made by Axonix.
///
/// Stored at `.axonix/bluesky_history.json`.
/// Loaded at startup. Updated after every successful post.
/// Used to detect near-duplicate posts and surface stats in the morning brief.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BlueskyHistory {
    /// All recorded posts, oldest first.
    pub posts: Vec<BlueskyPostRecord>,
    /// Path to the JSON file (not serialized).
    #[serde(skip)]
    path: String,
}

impl BlueskyHistory {
    /// Default path: `.axonix/bluesky_history.json`
    pub fn default_path() -> Self {
        Self::load(".axonix/bluesky_history.json")
    }

    /// Load from a file path. Returns an empty history if file doesn't exist.
    pub fn load(path: &str) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return Self { posts: vec![], path: path.to_string() },
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(mut h) => {
                h.path = path.to_string();
                h
            }
            Err(_) => Self { posts: vec![], path: path.to_string() },
        }
    }

    /// Save to the configured path. Creates `.axonix/` directory if needed.
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = std::path::Path::new(&self.path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("could not create dir {:?}: {e}", parent))?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize failed: {e}"))?;
        std::fs::write(&self.path, json)
            .map_err(|e| format!("write failed: {e}"))?;
        Ok(())
    }

    /// Record a new post. Does NOT save — call save() after.
    pub fn record(&mut self, created_at: &str, text: &str, uri: &str, cid: &str, is_reply: bool) {
        self.posts.push(BlueskyPostRecord {
            created_at: created_at.to_string(),
            text: text.to_string(),
            uri: uri.to_string(),
            cid: cid.to_string(),
            is_reply,
        });
    }

    /// Return the number of posts.
    pub fn len(&self) -> usize {
        self.posts.len()
    }

    /// Return true if no posts have been recorded.
    pub fn is_empty(&self) -> bool {
        self.posts.is_empty()
    }

    /// Return the most recent post, if any.
    pub fn latest(&self) -> Option<&BlueskyPostRecord> {
        self.posts.last()
    }

    /// Check if a given text is a near-duplicate of any recent post (last 20).
    ///
    /// "Near-duplicate" means the text is identical or one is a prefix of the other.
    /// Used to warn before posting something that was already posted recently.
    pub fn is_near_duplicate(&self, text: &str) -> bool {
        let text = text.trim();
        let recent = self.posts.iter().rev().take(20);
        for post in recent {
            let existing = post.text.trim();
            if existing == text
                || existing.starts_with(text)
                || text.starts_with(existing)
            {
                return true;
            }
        }
        false
    }

    /// Return the date of the most recent root post (non-reply).
    pub fn last_root_post_date(&self) -> Option<String> {
        self.posts.iter().rev()
            .find(|p| !p.is_reply)
            .map(|p| {
                // Return just the date part (first 10 chars of ISO 8601)
                p.created_at.chars().take(10).collect()
            })
    }

    /// Return counts: (total_posts, root_posts, replies)
    pub fn stats(&self) -> (usize, usize, usize) {
        let root = self.posts.iter().filter(|p| !p.is_reply).count();
        let replies = self.posts.iter().filter(|p| p.is_reply).count();
        (self.posts.len(), root, replies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bluesky_history_empty_by_default() {
        let h = BlueskyHistory::load("/tmp/nonexistent_bsky_test.json");
        assert!(h.is_empty(), "fresh history should be empty");
        assert_eq!(h.len(), 0);
        assert!(h.latest().is_none());
    }

    #[test]
    fn test_bluesky_history_record_and_retrieve() {
        let mut h = BlueskyHistory::load("/tmp/nonexistent_bsky_test.json");
        h.record("2026-03-22T12:00:00.000Z", "hello world", "at://did/post/1", "cid1", false);
        assert_eq!(h.len(), 1);
        let latest = h.latest().unwrap();
        assert_eq!(latest.text, "hello world");
        assert_eq!(latest.uri, "at://did/post/1");
        assert!(!latest.is_reply);
    }

    #[test]
    fn test_bluesky_history_save_and_load() {
        let path = "/tmp/test_bsky_history_save.json";
        let mut h = BlueskyHistory::load(path);
        h.record("2026-03-22T12:00:00.000Z", "test post", "at://did/post/1", "cid1", false);
        h.save().expect("save should succeed");

        let loaded = BlueskyHistory::load(path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.latest().unwrap().text, "test post");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_bluesky_history_is_near_duplicate_exact() {
        let mut h = BlueskyHistory::load("/tmp/nonexistent_bsky_test2.json");
        h.record("2026-03-22T12:00:00.000Z", "axonix Day 9 session update", "at://did/post/1", "cid1", false);
        assert!(h.is_near_duplicate("axonix Day 9 session update"), "exact match should be near-duplicate");
    }

    #[test]
    fn test_bluesky_history_is_near_duplicate_false_for_different() {
        let mut h = BlueskyHistory::load("/tmp/nonexistent_bsky_test3.json");
        h.record("2026-03-22T12:00:00.000Z", "axonix Day 9 session update", "at://did/post/1", "cid1", false);
        assert!(!h.is_near_duplicate("completely different text"), "different text should not be near-duplicate");
    }

    #[test]
    fn test_bluesky_history_stats() {
        let mut h = BlueskyHistory::load("/tmp/nonexistent_bsky_test4.json");
        h.record("2026-03-22T12:00:00.000Z", "root post 1", "at://did/1", "cid1", false);
        h.record("2026-03-22T12:01:00.000Z", "reply 1", "at://did/2", "cid2", true);
        h.record("2026-03-22T12:02:00.000Z", "root post 2", "at://did/3", "cid3", false);
        let (total, root, replies) = h.stats();
        assert_eq!(total, 3);
        assert_eq!(root, 2);
        assert_eq!(replies, 1);
    }

    #[test]
    fn test_bluesky_history_last_root_post_date() {
        let mut h = BlueskyHistory::load("/tmp/nonexistent_bsky_test5.json");
        h.record("2026-03-22T12:00:00.000Z", "root post", "at://did/1", "cid1", false);
        h.record("2026-03-22T13:00:00.000Z", "reply", "at://did/2", "cid2", true);
        let date = h.last_root_post_date();
        assert_eq!(date, Some("2026-03-22".to_string()));
    }

    #[test]
    fn test_bluesky_history_last_root_post_date_none_when_only_replies() {
        let mut h = BlueskyHistory::load("/tmp/nonexistent_bsky_test6.json");
        h.record("2026-03-22T12:00:00.000Z", "reply", "at://did/1", "cid1", true);
        let date = h.last_root_post_date();
        assert!(date.is_none(), "should be None when only replies exist");
    }

    #[test]
    fn test_bluesky_history_near_duplicate_prefix() {
        let mut h = BlueskyHistory::load("/tmp/nonexistent_bsky_test7.json");
        h.record("2026-03-22T12:00:00.000Z", "axonix Day 9", "at://did/1", "cid1", false);
        assert!(h.is_near_duplicate("axonix Day 9"), "exact prefix should match");
    }
}
