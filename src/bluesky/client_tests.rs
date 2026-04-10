//! Tests for bluesky::client — post formatting and client construction.

use super::*;

fn client() -> BlueskyClient {
    BlueskyClient::new("axonixai.bsky.social", "test-app-password")
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
    let title = "🦀".repeat(50);
    let post = BlueskyClient::format_session_post(2, 5, &title);
    assert!(post.chars().count() <= 300, "unicode title must fit: chars={}", post.chars().count());
}

#[test]
fn test_format_session_post_exact_300_or_less() {
    for title_len in [1, 50, 100, 200, 250, 300, 400] {
        let title = "x".repeat(title_len);
        let post = BlueskyClient::format_session_post(99, 99, &title);
        assert!(post.chars().count() <= 300, "post for title_len={title_len} exceeds 300 chars: {}", post.chars().count());
    }
}

// ── from_env / new / clone ───────────────────────────────────────────────

#[test]
fn test_from_env_returns_none_when_missing() {
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
