//! Journal archiving to prevent context window bloat (Issue #69, G-068).
//!
//! `JournalArchiver` moves older entries from JOURNAL.md to JOURNAL_ARCHIVE.md,
//! keeping only the most recent `keep_recent` entries in the main journal.
//! This keeps context window cost bounded as sessions accumulate.

mod parse;
mod archive;
#[cfg(test)]
mod tests;

pub use archive::ArchiveResult;

/// Archives old journal entries to JOURNAL_ARCHIVE.md.
pub struct JournalArchiver {
    pub(crate) journal_path: String,
    pub(crate) archive_path: String,
    pub(crate) keep_recent: usize,
}

impl JournalArchiver {
    /// Create a new archiver with explicit paths and keep count.
    pub fn new(journal_path: impl Into<String>, archive_path: impl Into<String>, keep_recent: usize) -> Self {
        Self {
            journal_path: journal_path.into(),
            archive_path: archive_path.into(),
            keep_recent,
        }
    }

    /// Create an archiver with default paths and `keep_recent = 10`.
    pub fn default() -> Self {
        Self::new("JOURNAL.md", "docs/archive/JOURNAL.md", 10)
    }

    /// Count the number of entries in a journal content string.
    /// An entry is a line that starts with `## `.
    pub fn count_entries(content: &str) -> usize {
        parse::count_entries(content)
    }

    /// Split journal content into individual entry strings.
    ///
    /// The `# Journal\n\n` header is stripped; each returned entry starts with
    /// `## ` and includes all lines up to (but not including) the next `## `.
    /// Entries are returned in the same order as they appear in the file
    /// (newest first, since JOURNAL.md prepends new entries at the top).
    pub fn split_entries(content: &str) -> Vec<String> {
        parse::split_entries(content)
    }

    /// Return `true` if the journal has more than `keep_recent + 5` entries.
    pub fn needs_archiving(&self) -> bool {
        archive::needs_archiving(self)
    }

    /// Archive old journal entries.
    ///
    /// If the journal has `keep_recent + 5` or fewer entries, returns `Ok` with
    /// `moved = 0` (no-op). Otherwise, moves the oldest entries to
    /// `archive_path`, keeping only the `keep_recent` newest in `journal_path`.
    pub fn archive(&self) -> Result<ArchiveResult, String> {
        archive::archive(self)
    }
}
