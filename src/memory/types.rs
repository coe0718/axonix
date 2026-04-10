//! `MemoryEntry` — a single key-value record in the memory store.

/// A single memory entry.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    /// The stored value.
    pub value: String,
    /// Optional note explaining why this was recorded or how to use it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// ISO 8601 timestamp of when this entry was last updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}

impl MemoryEntry {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            note: None,
            updated: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn with_updated(mut self, updated: impl Into<String>) -> Self {
        self.updated = Some(updated.into());
        self
    }
}
