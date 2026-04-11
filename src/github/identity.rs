//! Identity resolution for GitHub operations.
//!
//! Handles bot vs. owner identity: constructing clients from environment
//! variables and configuring git committer identity.

use super::types::{
    AGENT_GIT_EMAIL, AGENT_GIT_NAME, BOT_GIT_EMAIL, BOT_GIT_NAME, GitHubIdentity,
};
use super::client::GitHubClient;

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
                return Some(Self::new(token, GitHubIdentity::Bot));
            }
        }

        if let Ok(token) = std::env::var("GH_TOKEN")
            .or_else(|_| std::env::var("GITHUB_TOKEN"))
        {
            if !token.is_empty() {
                return Some(Self::new(token, GitHubIdentity::Owner));
            }
        }

        None
    }

    /// Create a bot-only client — returns None if AXONIX_BOT_TOKEN is not set.
    /// Use this for posting comments to avoid accidentally posting as the owner.
    pub fn bot_only() -> Option<Self> {
        if let Ok(token) = std::env::var("AXONIX_BOT_TOKEN")
            .or_else(|_| std::env::var("AXONIX_TOKEN"))
        {
            if !token.is_empty() {
                return Some(Self::new(token, GitHubIdentity::Bot));
            }
        }
        None
    }

    /// Returns true if operating as the axonix-bot account.
    pub fn is_bot(&self) -> bool {
        self.identity == GitHubIdentity::Bot
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
}
