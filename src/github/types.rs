//! Types for the GitHub module: identity enum, issue entries, and git constants.

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
