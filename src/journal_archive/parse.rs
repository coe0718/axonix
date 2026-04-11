//! Parsing helpers for journal content.

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
