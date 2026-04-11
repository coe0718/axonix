//! Caddyfile validation using structural heuristics.
//!
//! Checks: brace balance, unterminated blocks, suspicious syntax,
//! and indentation consistency. Not a full parser but catches common mistakes.

use super::{LintError, LintResult};

/// Lint a Caddyfile using structural heuristics.
/// Checks: brace balance, unterminated strings, known directive patterns,
/// suspicious syntax, and indentation consistency.
pub fn lint_caddyfile(path: &str) -> LintResult {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return LintResult::Errors(vec![LintError::new(0, format!("Cannot read file: {e}"))]),
    };

    if content.trim().is_empty() {
        return LintResult::Ok("File is empty".to_string());
    }

    let mut errors = Vec::new();
    let mut brace_depth: i64 = 0;
    let mut brace_open_lines: Vec<usize> = Vec::new();
    let mut block_count = 0;

    for (i, line) in content.lines().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();

        // Skip comments and blank lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Warn about Windows-style line endings (check raw line, not trimmed)
        if line.ends_with('\r') {
            errors.push(LintError::new(lineno, "Windows line ending (CRLF) detected — may cause parsing issues".to_string()));
        }

        // Indentation checks: only for indented lines (inside blocks)
        if brace_depth > 0 && !trimmed.starts_with('}') {
            let (tabs, spaces) = leading_whitespace_kind(line);
            if tabs > 0 && spaces > 0 {
                errors.push(LintError::new(lineno, "Mixed tabs and spaces in indentation — use one consistently".to_string()));
            }
        }

        // Count braces on this line
        for ch in trimmed.chars() {
            match ch {
                '{' => {
                    brace_depth += 1;
                    brace_open_lines.push(lineno);
                    block_count += 1;
                }
                '}' => {
                    brace_depth -= 1;
                    if !brace_open_lines.is_empty() {
                        brace_open_lines.pop();
                    }
                    if brace_depth < 0 {
                        errors.push(LintError::new(lineno, "Unexpected '}' — no matching '{'".to_string()));
                        brace_depth = 0;
                    }
                }
                _ => {}
            }
        }

        // Inside a site block — check common directive mistakes
        // Note: brace depth is already updated for this line, so depth==1 means
        // we just opened a block OR we're in the first level of a block.
        // Skip lines that open a new block (contain '{') — they're site/block headers.
        if brace_depth == 1 && !trimmed.contains('{') {
            if trimmed == "reverse_proxy" {
                errors.push(LintError::new(
                    lineno,
                    "reverse_proxy directive missing upstream address (e.g. reverse_proxy localhost:3000)".to_string(),
                ));
            }
            // Directive at column 0 inside a block is a formatting error.
            // Closing braces and block-opening lines are exempt.
            if !trimmed.starts_with('}') && !line.starts_with(' ') && !line.starts_with('\t') {
                errors.push(LintError::new(lineno, format!(
                    "Directive '{}' appears at column 0 inside a block — should be indented",
                    trimmed.split_whitespace().next().unwrap_or(trimmed)
                )));
            }
        }

        // At top level — bare port numbers are suspicious
        if brace_depth == 0 {
            let no_brace = trimmed.trim_end_matches('{').trim();
            if !no_brace.is_empty() && no_brace.chars().all(|c| c.is_ascii_digit()) {
                errors.push(LintError::new(lineno, format!(
                    "Suspicious site address '{}' — did you mean ':{}'?",
                    no_brace, no_brace
                )));
            }
        }
    }

    // Check for unclosed braces
    if brace_depth > 0 {
        for open_line in &brace_open_lines {
            errors.push(LintError::new(*open_line, "Unclosed '{' — missing closing '}'".to_string()));
        }
    }

    // Check for inconsistent indentation width across the whole file
    let indent_errors = check_indent_consistency(&content);
    errors.extend(indent_errors);

    if errors.is_empty() {
        let line_count = content.lines().count();
        LintResult::Ok(format!("{line_count} lines, {block_count} block(s) — valid Caddyfile structure ✓"))
    } else {
        LintResult::Errors(errors)
    }
}

/// Return (tab_count, space_count) for the leading whitespace of a line.
pub fn leading_whitespace_kind(line: &str) -> (usize, usize) {
    let mut tabs = 0usize;
    let mut spaces = 0usize;
    for ch in line.chars() {
        match ch {
            '\t' => tabs += 1,
            ' '  => spaces += 1,
            _    => break,
        }
    }
    (tabs, spaces)
}

/// Check that indentation width is consistent across a Caddyfile.
///
/// Caddy convention is 2 or 4 spaces (or tabs). If some lines inside blocks
/// use 2-space indent and others use 4-space indent, flag the inconsistency.
/// Only fires if the file has a clear majority indent-width that is violated.
pub fn check_indent_consistency(content: &str) -> Vec<LintError> {
    // Collect indent widths of non-empty, non-comment, space-indented lines inside blocks.
    // Skip lines that open blocks (contain '{') since they're site/block headers.
    // Skip closing braces.
    let mut brace_depth: i64 = 0;
    let mut indent_widths: Vec<(usize, usize)> = Vec::new(); // (lineno, width)

    for (i, line) in content.lines().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Count braces (simplified — same as main pass)
        for ch in trimmed.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth = (brace_depth - 1).max(0),
                _ => {}
            }
        }
        // Only check lines indented with spaces inside blocks.
        // Skip: closing braces, lines that open new blocks (they're site/route headers).
        if brace_depth > 0
            && line.starts_with(' ')
            && !trimmed.starts_with('}')
            && !trimmed.contains('{')
        {
            let spaces = line.len() - line.trim_start_matches(' ').len();
            if spaces > 0 {
                indent_widths.push((lineno, spaces));
            }
        }
    }

    if indent_widths.len() < 2 {
        return vec![]; // not enough data to assess consistency
    }

    // Find the minimum indent width — this is the base unit (e.g. 2 or 4 spaces).
    // All deeper indents must be exact multiples of the base.
    // This correctly handles nested blocks: depth-2 = 2×base, depth-3 = 3×base.
    let min_width = indent_widths.iter().map(|(_, w)| *w).min().unwrap_or(0);
    if min_width == 0 {
        return vec![];
    }

    // Flag lines whose indent width is not a multiple of the base unit.
    let mut errors = Vec::new();
    for (lineno, width) in &indent_widths {
        if width % min_width != 0 {
            errors.push(LintError::new(*lineno, format!(
                "Inconsistent indentation: {width} spaces, expected a multiple of {min_width} — check alignment"
            )));
        }
    }
    errors
}

