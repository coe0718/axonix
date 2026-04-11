//! Types for conversation memory: `ConversationTurn`.

use serde::{Deserialize, Serialize};

/// A single conversation turn (one message from one party).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversationTurn {
    /// ISO 8601 timestamp (e.g. "2026-03-22T14:30:00Z").
    pub timestamp: String,
    /// Who spoke: `"user"` or `"assistant"`.
    pub role: String,
    /// The message text.
    pub text: String,
    /// Channel the message came from: `"telegram"`, `"repl"`, `"prompt"`.
    pub channel: String,
}
