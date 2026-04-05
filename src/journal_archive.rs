//! Journal archiving to prevent context window bloat (Issue #69, G-068).
//!
//! `JournalArchiver` moves older entries from JOURNAL.md to JOURNAL_ARCHIVE.md,
//! keeping only the most recent `keep_recent` entries in the main journal.
//! This keeps context window cost bounded as sessions accumulate.

use std::fs;

/// Result of a journal archive operation.
pub struct ArchiveResult {
    /// Number of entries moved to the archive.
    pub moved: usize,
    /// Number of entries kept in JOURNAL.md.
    pub kept: usize,
    /// Path to the archive file.
    pub archive_path: String,
}

/// Archives old journal entries to JOURNAL_ARCHIVE.md.
pub struct JournalArchiver {
    journal_path: String,
    archive_path: String,
    keep_recent: usize,
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
        content.lines().filter(|l| l.starts_with("## ")).count()
    }

    /// Split journal content into individual entry strings.
    ///
    /// The `# Journal\n\n` header is stripped; each returned entry starts with
    /// `## ` and includes all lines up to (but not including) the next `## `.
    /// Entries are returned in the same order as they appear in the file
    /// (newest first, since JOURNAL.md prepends new entries at the top).
    pub fn split_entries(content: &str) -> Vec<String> {
        // Strip the header line(s) — everything before the first `## `
        let body = match content.find("\n## ") {
            Some(pos) => &content[pos + 1..], // skip the newline before `## `
            None => {
                // Maybe starts directly with `## ` (no header)
                if content.starts_with("## ") {
                    content
                } else {
                    return Vec::new();
                }
            }
        };

        let mut entries: Vec<String> = Vec::new();
        let mut current: Vec<&str> = Vec::new();

        for line in body.lines() {
            if line.starts_with("## ") && !current.is_empty() {
                // Flush the previous entry
                let entry = current.join("\n").trim_end().to_string();
                if !entry.is_empty() {
                    entries.push(entry);
                }
                current.clear();
            }
            current.push(line);
        }

        // Flush the last entry
        if !current.is_empty() {
            let entry = current.join("\n").trim_end().to_string();
            if !entry.is_empty() {
                entries.push(entry);
            }
        }

        entries
    }

    /// Return `true` if the journal has more than `keep_recent + 5` entries.
    pub fn needs_archiving(&self) -> bool {
        match fs::read_to_string(&self.journal_path) {
            Ok(content) => Self::count_entries(&content) > self.keep_recent + 5,
            Err(_) => false,
        }
    }

    /// Archive old journal entries.
    ///
    /// If the journal has `keep_recent + 5` or fewer entries, returns `Ok` with
    /// `moved = 0` (no-op). Otherwise, moves the oldest entries to
    /// `archive_path`, keeping only the `keep_recent` newest in `journal_path`.
    pub fn archive(&self) -> Result<ArchiveResult, String> {
        let content = fs::read_to_string(&self.journal_path)
            .map_err(|e| format!("failed to read {}: {e}", self.journal_path))?;

        let total = Self::count_entries(&content);
        let threshold = self.keep_recent + 5;

        if total <= threshold {
            return Ok(ArchiveResult {
                moved: 0,
                kept: total,
                archive_path: self.archive_path.clone(),
            });
        }

        let entries = Self::split_entries(&content);
        let actual_count = entries.len();

        // Keep the newest entries (they are at the front of the Vec)
        let keep = actual_count.min(self.keep_recent);
        let kept_entries = &entries[..keep];
        let moved_entries = &entries[keep..];
        let moved = moved_entries.len();

        // --- Write back JOURNAL.md with only the kept entries ---
        let mut new_journal = String::from("# Journal\n\n");
        for (i, entry) in kept_entries.iter().enumerate() {
            new_journal.push_str(entry);
            new_journal.push('\n');
            if i + 1 < kept_entries.len() {
                new_journal.push('\n');
            }
        }

        fs::write(&self.journal_path, &new_journal)
            .map_err(|e| format!("failed to write {}: {e}", self.journal_path))?;

        // --- Append to JOURNAL_ARCHIVE.md ---
        let existing_archive = fs::read_to_string(&self.archive_path).unwrap_or_default();
        let mut archive_content = String::new();

        if existing_archive.trim().is_empty() {
            // New archive — add a header
            archive_content.push_str("# Journal Archive\n\n");
        } else {
            archive_content.push_str(&existing_archive);
            // Ensure there is a blank line separator before new entries
            if !archive_content.ends_with("\n\n") {
                if archive_content.ends_with('\n') {
                    archive_content.push('\n');
                } else {
                    archive_content.push_str("\n\n");
                }
            }
        }

        for (i, entry) in moved_entries.iter().enumerate() {
            archive_content.push_str(entry);
            archive_content.push('\n');
            if i + 1 < moved_entries.len() {
                archive_content.push('\n');
            }
        }

        fs::write(&self.archive_path, &archive_content)
            .map_err(|e| format!("failed to write {}: {e}", self.archive_path))?;

        Ok(ArchiveResult {
            moved,
            kept: keep,
            archive_path: self.archive_path.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: build N fake journal entries as a content string (newest at top).
    fn make_journal(n: usize) -> String {
        let mut s = String::from("# Journal\n\n");
        for i in (1..=n).rev() {
            s.push_str(&format!("## Day {i}, Session 1 — Entry {i}\n\nContent of entry {i}.\n\n"));
        }
        s
    }

    /// Helper: create an archiver that writes to temp files.
    fn temp_archiver(dir: &TempDir, keep_recent: usize) -> JournalArchiver {
        let journal = dir.path().join("JOURNAL.md");
        let archive = dir.path().join("JOURNAL_ARCHIVE.md");
        JournalArchiver::new(
            journal.to_str().unwrap(),
            archive.to_str().unwrap(),
            keep_recent,
        )
    }

    // ── count_entries ─────────────────────────────────────────────────────────

    #[test]
    fn test_count_entries_empty_string() {
        assert_eq!(JournalArchiver::count_entries(""), 0);
    }

    #[test]
    fn test_count_entries_header_only() {
        assert_eq!(JournalArchiver::count_entries("# Journal\n\n"), 0);
    }

    #[test]
    fn test_count_entries_one_entry() {
        let content = "# Journal\n\n## Day 1, Session 1 — Test\n\nBody.\n";
        assert_eq!(JournalArchiver::count_entries(content), 1);
    }

    #[test]
    fn test_count_entries_three_entries() {
        let content = make_journal(3);
        assert_eq!(JournalArchiver::count_entries(&content), 3);
    }

    #[test]
    fn test_count_entries_ten_entries() {
        let content = make_journal(10);
        assert_eq!(JournalArchiver::count_entries(&content), 10);
    }

    #[test]
    fn test_count_entries_does_not_count_body_hashes() {
        // Lines that start with `# ` or `### ` should not be counted
        let content = "# Journal\n\n## Day 1 — Title\n\n### Subheading\n# Top\n";
        assert_eq!(JournalArchiver::count_entries(content), 1);
    }

    // ── split_entries ─────────────────────────────────────────────────────────

    #[test]
    fn test_split_entries_empty() {
        let entries = JournalArchiver::split_entries("");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_split_entries_header_only() {
        let entries = JournalArchiver::split_entries("# Journal\n\n");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_split_entries_one_entry() {
        let content = "# Journal\n\n## Day 1, Session 1 — Test\n\nBody text.\n";
        let entries = JournalArchiver::split_entries(content);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].starts_with("## Day 1"), "entry should start with ## : {:?}", entries[0]);
        assert!(entries[0].contains("Body text."));
    }

    #[test]
    fn test_split_entries_three_entries() {
        let content = make_journal(3);
        let entries = JournalArchiver::split_entries(&content);
        assert_eq!(entries.len(), 3, "should have 3 entries");
        // Newest first
        assert!(entries[0].starts_with("## Day 3"), "first entry should be Day 3: {:?}", entries[0]);
        assert!(entries[1].starts_with("## Day 2"), "second entry should be Day 2: {:?}", entries[1]);
        assert!(entries[2].starts_with("## Day 1"), "third entry should be Day 1: {:?}", entries[2]);
    }

    #[test]
    fn test_split_entries_each_starts_with_double_hash() {
        let content = make_journal(5);
        let entries = JournalArchiver::split_entries(&content);
        for entry in &entries {
            assert!(entry.starts_with("## "), "every entry must start with ## : {entry:?}");
        }
    }

    // ── archive (no-op) ───────────────────────────────────────────────────────

    #[test]
    fn test_archive_below_threshold_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);

        // 14 entries = keep_recent(10) + 5 - 1 = threshold exactly — no-op
        let content = make_journal(14);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();

        let result = archiver.archive().unwrap();
        assert_eq!(result.moved, 0, "should not move any entries");
        assert_eq!(result.kept, 14);

        // JOURNAL_ARCHIVE.md should NOT have been created
        assert!(!std::path::Path::new(&archiver.archive_path).exists(),
            "archive file should not be created for no-op");
    }

    #[test]
    fn test_archive_at_threshold_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);

        // Exactly at threshold (10 + 5 = 15) — still no-op
        let content = make_journal(15);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();

        let result = archiver.archive().unwrap();
        assert_eq!(result.moved, 0);
    }

    // ── archive (real move) ───────────────────────────────────────────────────

    #[test]
    fn test_archive_moves_oldest_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);

        // 20 entries: 10 recent kept, 10 oldest moved
        let content = make_journal(20);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();

        let result = archiver.archive().unwrap();
        assert_eq!(result.moved, 10, "10 entries should be moved");
        assert_eq!(result.kept, 10, "10 entries should be kept");
    }

    #[test]
    fn test_archive_journal_has_only_keep_recent_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);

        let content = make_journal(20);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();

        archiver.archive().unwrap();

        let new_journal = fs::read_to_string(&archiver.journal_path).unwrap();
        assert_eq!(JournalArchiver::count_entries(&new_journal), 10,
            "JOURNAL.md should have exactly 10 entries after archiving");
    }

    #[test]
    fn test_archive_journal_keeps_newest_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);

        let content = make_journal(20);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();

        archiver.archive().unwrap();

        let new_journal = fs::read_to_string(&archiver.journal_path).unwrap();
        // Entries Day 20..Day 11 are newest (they appear first in make_journal)
        assert!(new_journal.contains("Entry 20"), "newest entry Day 20 should be kept");
        assert!(new_journal.contains("Entry 11"), "entry Day 11 should be kept");
        assert!(!new_journal.contains("Entry 10"), "Day 10 should be moved to archive");
        assert!(!new_journal.contains("Entry 1\n"), "oldest Day 1 should be moved to archive");
    }

    #[test]
    fn test_archive_archive_file_has_moved_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);

        let content = make_journal(20);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();

        archiver.archive().unwrap();

        let archive = fs::read_to_string(&archiver.archive_path).unwrap();
        assert_eq!(JournalArchiver::count_entries(&archive), 10,
            "JOURNAL_ARCHIVE.md should have the 10 moved entries");
        assert!(archive.contains("Entry 10"), "archive should have Entry 10");
        assert!(archive.contains("Entry 1\n"), "archive should have Entry 1 (oldest)");
        assert!(!archive.contains("Entry 20"), "archive should NOT have Entry 20 (newest)");
    }

    #[test]
    fn test_archive_journal_retains_header() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);

        let content = make_journal(20);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();

        archiver.archive().unwrap();

        let new_journal = fs::read_to_string(&archiver.journal_path).unwrap();
        assert!(new_journal.starts_with("# Journal\n"),
            "JOURNAL.md should still start with '# Journal': {new_journal:.100}");
    }

    #[test]
    fn test_archive_appends_to_existing_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);

        // Pre-populate the archive
        fs::write(&archiver.archive_path, "# Journal Archive\n\n## Day 0, Session 1 — Old\n\nOld content.\n").unwrap();

        let content = make_journal(20);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();

        archiver.archive().unwrap();

        let archive = fs::read_to_string(&archiver.archive_path).unwrap();
        // Old entry should still be there
        assert!(archive.contains("Day 0"), "existing archive entry should be preserved");
        // New moved entries should be appended
        assert!(archive.contains("Day 1"), "newly moved entry Day 1 should be in archive");
    }

    #[test]
    fn test_archive_missing_journal_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);
        // Don't write any journal file
        let result = archiver.archive();
        assert!(result.is_err(), "archiving a missing journal should return Err");
    }

    #[test]
    fn test_needs_archiving_false_for_small_journal() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);
        let content = make_journal(5);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();
        assert!(!archiver.needs_archiving(), "5 entries should not need archiving");
    }

    #[test]
    fn test_needs_archiving_true_for_large_journal() {
        let dir = tempfile::tempdir().unwrap();
        let archiver = temp_archiver(&dir, 10);
        let content = make_journal(20);
        fs::write(archiver.journal_path.as_str(), &content).unwrap();
        assert!(archiver.needs_archiving(), "20 entries should need archiving");
    }
}

