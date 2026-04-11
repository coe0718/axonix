//! Core archiving logic: reading files and writing them.

use std::fs;
use super::JournalArchiver;
use super::parse;

/// Result of a journal archive operation.
pub struct ArchiveResult {
    /// Number of entries moved to the archive.
    pub moved: usize,
    /// Number of entries kept in JOURNAL.md.
    pub kept: usize,
    /// Path to the archive file.
    pub archive_path: String,
}

/// Return `true` if the journal has more than `keep_recent + 5` entries.
pub fn needs_archiving(archiver: &JournalArchiver) -> bool {
    match fs::read_to_string(&archiver.journal_path) {
        Ok(content) => parse::count_entries(&content) > archiver.keep_recent + 5,
        Err(_) => false,
    }
}

/// Archive old journal entries.
///
/// If the journal has `keep_recent + 5` or fewer entries, returns `Ok` with
/// `moved = 0` (no-op). Otherwise, moves the oldest entries to
/// `archive_path`, keeping only the `keep_recent` newest in `journal_path`.
pub fn archive(archiver: &JournalArchiver) -> Result<ArchiveResult, String> {
    let content = fs::read_to_string(&archiver.journal_path)
        .map_err(|e| format!("failed to read {}: {e}", archiver.journal_path))?;

    let total = parse::count_entries(&content);
    let threshold = archiver.keep_recent + 5;

    if total <= threshold {
        return Ok(ArchiveResult {
            moved: 0,
            kept: total,
            archive_path: archiver.archive_path.clone(),
        });
    }

    let entries = parse::split_entries(&content);
    let actual_count = entries.len();

    // Keep the newest entries (they are at the front of the Vec)
    let keep = actual_count.min(archiver.keep_recent);
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

    fs::write(&archiver.journal_path, &new_journal)
        .map_err(|e| format!("failed to write {}: {e}", archiver.journal_path))?;

    // --- Append to JOURNAL_ARCHIVE.md ---
    let existing_archive = fs::read_to_string(&archiver.archive_path).unwrap_or_default();
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

    fs::write(&archiver.archive_path, &archive_content)
        .map_err(|e| format!("failed to write {}: {e}", archiver.archive_path))?;

    Ok(ArchiveResult {
        moved,
        kept: keep,
        archive_path: archiver.archive_path.clone(),
    })
}
