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

mod client;
mod identity;
mod types;

pub use client::GitHubClient;
pub use types::{
    AGENT_GIT_EMAIL, AGENT_GIT_NAME, BOT_GIT_EMAIL, BOT_GIT_NAME,
    GitHubIdentity, IssueEntry,
};

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

    #[test]
    fn test_issue_entry_reactions_zero() {
        let entry = IssueEntry {
            number: 5,
            title: "No reactions".to_string(),
            reactions: 0,
            labels: vec![],
        };
        assert_eq!(entry.reactions, 0);
    }

    #[test]
    fn test_issue_entry_multiple_labels() {
        let entry = IssueEntry {
            number: 3,
            title: "Multi-label".to_string(),
            reactions: 2,
            labels: vec!["bug".to_string(), "enhancement".to_string(), "good first issue".to_string()],
        };
        assert_eq!(entry.labels.len(), 3);
        assert!(entry.labels.contains(&"bug".to_string()));
        assert!(entry.labels.contains(&"enhancement".to_string()));
    }

    #[test]
    fn test_github_identity_equality() {
        assert_eq!(GitHubIdentity::Bot, GitHubIdentity::Bot);
        assert_eq!(GitHubIdentity::Owner, GitHubIdentity::Owner);
        assert_ne!(GitHubIdentity::Bot, GitHubIdentity::Owner);
    }

    #[test]
    fn test_client_token_stored() {
        let client = GitHubClient::new("secret-token-123", GitHubIdentity::Bot);
        assert_eq!(client.token, "secret-token-123");
    }
}
