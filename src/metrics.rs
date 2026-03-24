//! metrics — ordered, deduplicated writes to METRICS.md (Issue #67, G-071)
//!
//! `insert_metrics_row` inserts a new row immediately after the `|-----|` separator
//! line and removes any existing row for the same Day+Session before inserting.
//! This fixes two bugs: rows appended to end-of-file (wrong order) and duplicate
//! stub rows left behind after a session's final write.

use std::path::Path;

/// Extract (day, session) from a pipe-delimited METRICS.md row.
///
/// Columns (0-indexed after splitting on `|` and trimming):
///   index 0 — empty string (before the leading `|`)
///   index 1 — Day
///   index 2 — Session
///
/// Returns `None` if the row does not have at least 3 pipe-separated fields.
fn parse_day_session(row: &str) -> Option<(String, String)> {
    let cols: Vec<&str> = row.split('|').collect();
    if cols.len() < 3 {
        return None;
    }
    let day = cols[1].trim().to_string();
    let session = cols[2].trim().to_string();
    if day.is_empty() || session.is_empty() {
        return None;
    }
    Some((day, session))
}

/// Returns `true` when `line` is the `|-----|` header-separator row.
fn is_separator(line: &str) -> bool {
    line.trim_start().starts_with("|---")
}

/// Insert `row` into the METRICS.md at `path`:
///
/// 1. Reads the existing file.
/// 2. Removes any existing row whose Day+Session matches that of `row`.
/// 3. Inserts `row` immediately after the `|-----|` separator line.
/// 4. Writes the result back to `path`.
///
/// If no separator is found the new row is appended to the end of the file.
pub fn insert_metrics_row(path: &Path, row: &str) -> std::io::Result<()> {
    let target = parse_day_session(row);

    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // Step 1 — remove existing rows that share the same Day+Session.
    if let Some((ref day, ref session)) = target {
        lines.retain(|line| {
            // Keep non-data lines (header, separator, blank) unconditionally.
            if !line.trim_start().starts_with('|') || is_separator(line) {
                return true;
            }
            match parse_day_session(line) {
                Some((d, s)) => !(d == *day && s == *session),
                None => true,
            }
        });
    }

    // Step 2 — find the separator line and insert the new row right after it.
    let sep_pos = lines.iter().position(|l| is_separator(l));

    match sep_pos {
        Some(pos) => lines.insert(pos + 1, row.to_string()),
        None => lines.push(row.to_string()),
    }

    // Preserve trailing newline.
    let mut output = lines.join("\n");
    output.push('\n');

    std::fs::write(path, output)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    /// Build a minimal METRICS.md string with optional extra rows.
    fn base_metrics(extra_rows: &[&str]) -> String {
        let mut s = String::from(
            "# Metrics\n\
             | Day | Session | Date | Tokens | Tests | Failed | Files | +Lines | -Lines | Committed | Notes |\n\
             |-----|---------|------|--------|-------|--------|-------|--------|--------|-----------|-------|\n",
        );
        for r in extra_rows {
            s.push_str(r);
            s.push('\n');
        }
        s
    }

    fn write_tmp(content: &str) -> NamedTempFile {
        let f = NamedTempFile::new().unwrap();
        fs::write(f.path(), content).unwrap();
        f
    }

    // ── Test 1: row is inserted immediately after the separator ──────────────
    #[test]
    fn test_inserts_after_separator() {
        let content = base_metrics(&[]);
        let f = write_tmp(&content);

        let row = "| 11 | S3 | 2026-03-24 | ~?k | 721 | 0 | ? | ? | ? | yes | Day 11 S3 — done |";
        insert_metrics_row(f.path(), row).unwrap();

        let result = fs::read_to_string(f.path()).unwrap();
        let lines: Vec<&str> = result.lines().collect();

        // separator is line index 2 (0-based); new row must be at index 3
        let sep_idx = lines.iter().position(|l| is_separator(l)).unwrap();
        assert_eq!(lines[sep_idx + 1], row);
    }

    // ── Test 2: deduplicates an existing stub row for the same Day+Session ───
    #[test]
    fn test_deduplicates_same_day_session() {
        let stub = "| 11 | S3 | 2026-03-24 | ~?k | 721 | 0 | ? | ? | ? | yes | Day 11 S3 — in progress |";
        let content = base_metrics(&[stub]);
        let f = write_tmp(&content);

        let final_row = "| 11 | S3 | 2026-03-24 | ~58k | 721 | 0 | 4 | 487 | 1 | yes | Day 11 S3 — G-071 done |";
        insert_metrics_row(f.path(), final_row).unwrap();

        let result = fs::read_to_string(f.path()).unwrap();
        // stub must be gone
        assert!(!result.contains("in progress"), "stub row should be removed");
        // final row must be present
        assert!(result.contains("G-071 done"));
        // exactly one data row for Day 11 S3
        let count = result
            .lines()
            .filter(|l| {
                l.contains("| 11 | S3 |") || l.contains("| 11 | S3 |")
            })
            .count();
        assert_eq!(count, 1, "should be exactly one row for Day 11 S3");
    }

    // ── Test 3: no separator — appends to end ────────────────────────────────
    #[test]
    fn test_no_separator_appends_to_end() {
        let content = "# Metrics\nSome content without a separator\n";
        let f = write_tmp(content);

        let row = "| 11 | S3 | 2026-03-24 | ~?k | 721 | 0 | ? | ? | ? | yes | Day 11 S3 — done |";
        insert_metrics_row(f.path(), row).unwrap();

        let result = fs::read_to_string(f.path()).unwrap();
        assert!(result.ends_with(&format!("{}\n", row)));
    }

    // ── Test 4: only header and separator — inserts correctly ────────────────
    #[test]
    fn test_only_header_no_data_rows() {
        let content = base_metrics(&[]);
        let f = write_tmp(&content);

        let row = "| 5 | S1 | 2026-03-18 | ~20k | 100 | 0 | 3 | 50 | 5 | yes | Day 5 S1 |";
        insert_metrics_row(f.path(), row).unwrap();

        let result = fs::read_to_string(f.path()).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        let sep_idx = lines.iter().position(|l| is_separator(l)).unwrap();
        assert_eq!(lines[sep_idx + 1], row);
    }

    // ── Test 5: does NOT remove rows for a different Day/Session ─────────────
    #[test]
    fn test_does_not_remove_different_day_session() {
        let other = "| 10 | S6 | 2026-03-23 | ~58k | 672 | 0 | 9 | 525 | 224 | yes | Day 10 S6 — done |";
        let content = base_metrics(&[other]);
        let f = write_tmp(&content);

        let row = "| 11 | S3 | 2026-03-24 | ~?k | 721 | 0 | ? | ? | ? | yes | Day 11 S3 — done |";
        insert_metrics_row(f.path(), row).unwrap();

        let result = fs::read_to_string(f.path()).unwrap();
        assert!(result.contains("Day 10 S6"), "different-session row must be preserved");
        assert!(result.contains("Day 11 S3"), "new row must be present");
    }

    // ── Test 6: preserves all other rows in order ────────────────────────────
    #[test]
    fn test_preserves_other_rows_in_order() {
        let rows = [
            "| 10 | S5 | 2026-03-23 | ~75k | 650 | 0 | 10 | 1292 | 2 | yes | Day 10 S5 |",
            "| 10 | S4 | 2026-03-23 | ~82k | 612 | 0 | 6 | 479 | 25 | yes | Day 10 S4 |",
            "| 10 | S3 | 2026-03-23 | ~56k | 597 | 0 | 9 | 537 | 2 | yes | Day 10 S3 |",
        ];
        let content = base_metrics(&rows);
        let f = write_tmp(&content);

        let row = "| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | Day 11 S1 — new |";
        insert_metrics_row(f.path(), row).unwrap();

        let result = fs::read_to_string(f.path()).unwrap();
        let data_lines: Vec<&str> = result
            .lines()
            .filter(|l| l.starts_with("| ") && !is_separator(l) && !l.starts_with("| Day"))
            .collect();

        // New row at index 0, originals follow in original order.
        assert!(data_lines[0].contains("Day 11 S1"), "new row first");
        assert!(data_lines[1].contains("Day 10 S5"), "S5 second");
        assert!(data_lines[2].contains("Day 10 S4"), "S4 third");
        assert!(data_lines[3].contains("Day 10 S3"), "S3 fourth");
    }

    // ── Test 7: new row is at the top (right after separator), not at bottom ─
    #[test]
    fn test_new_row_at_top_not_bottom() {
        let old_row = "| 9 | S1 | 2026-03-22 | ~22k | 539 | 0 | 5 | 177 | 0 | yes | Day 9 S1 |";
        let content = base_metrics(&[old_row]);
        let f = write_tmp(&content);

        let new_row = "| 11 | S3 | 2026-03-24 | ~?k | 721 | 0 | ? | ? | ? | yes | Day 11 S3 — done |";
        insert_metrics_row(f.path(), new_row).unwrap();

        let result = fs::read_to_string(f.path()).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        let sep_idx = lines.iter().position(|l| is_separator(l)).unwrap();

        // The row right after the separator must be the newly inserted one.
        assert_eq!(lines[sep_idx + 1], new_row, "new row must be right after separator");

        // The old row must come after the new row (not before).
        let new_pos = lines.iter().position(|l| *l == new_row).unwrap();
        let old_pos = lines.iter().position(|l| *l == old_row).unwrap();
        assert!(new_pos < old_pos, "new row must appear before old row");
    }

    // ── Test 8: pipe chars in the notes field don't confuse the parser ───────
    #[test]
    fn test_pipe_chars_in_notes_field() {
        // A row where the Notes column itself contains a pipe-like description.
        // The parser uses only columns 1 and 2 so extra pipes at the end are OK
        // as long as Day and Session are unambiguous.
        let stub = "| 12 | S1 | 2026-03-25 | ~?k | 800 | 0 | ? | ? | ? | yes | Day 12 S1 — stub |";
        let content = base_metrics(&[stub]);
        let f = write_tmp(&content);

        // New row with a notes field containing a parenthetical that looks pipe-ish.
        let row = "| 12 | S1 | 2026-03-25 | ~60k | 800 | 0 | 5 | 200 | 10 | yes | Day 12 S1 — fix(a|b) edge case |";
        insert_metrics_row(f.path(), row).unwrap();

        let result = fs::read_to_string(f.path()).unwrap();
        // Stub must be gone.
        assert!(!result.contains("stub"), "stub row should be removed");
        // New row present.
        assert!(result.contains("fix(a|b)"), "new row with pipe-in-notes must be present");
        // Only one row for Day 12 S1.
        let count = result
            .lines()
            .filter(|l| l.contains("| 12 | S1 |"))
            .count();
        assert_eq!(count, 1);
    }

    // ── Test 9: empty file is handled gracefully ─────────────────────────────
    #[test]
    fn test_empty_file() {
        let f = write_tmp("");
        let row = "| 1 | S1 | 2026-03-14 | ~30k | 40 | 0 | 4 | 206 | 26 | yes | First boot |";
        insert_metrics_row(f.path(), row).unwrap();
        let result = fs::read_to_string(f.path()).unwrap();
        assert!(result.contains("First boot"));
    }

    // ── Test 10: parse_day_session returns None for truly malformed rows ─────
    #[test]
    fn test_parse_day_session_none_for_malformed() {
        // No pipe chars at all → fewer than 3 segments → None.
        assert!(parse_day_session("no pipes here").is_none());
        // Only one pipe → only 2 segments → None.
        assert!(parse_day_session("only|one").is_none());
        // Valid row → Some with trimmed day and session.
        assert_eq!(
            parse_day_session("| 11 | S3 | 2026-03-24 | rest |"),
            Some(("11".to_string(), "S3".to_string()))
        );
        // Separator rows have non-empty "columns" but the is_separator guard
        // in retain() prevents them from ever being removed as data rows.
        // Verify the separator is correctly identified.
        assert!(is_separator("|-----|---------|------|"));
        assert!(!is_separator("| 11 | S3 | 2026-03-24 |"));
    }
}
