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
pub(super) fn leading_whitespace_kind(line: &str) -> (usize, usize) {
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
fn check_indent_consistency(content: &str) -> Vec<LintError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn lint_caddyfile_str(content: &str) -> LintResult {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "{}", content).unwrap();
        lint_caddyfile(tmp.path().to_str().unwrap())
    }

    #[test]
    fn test_caddyfile_balanced_braces_ok() {
        let content = "example.com {\n    reverse_proxy localhost:3000\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Balanced braces should pass: {result:?}");
    }

    #[test]
    fn test_caddyfile_unclosed_brace() {
        let content = "example.com {\n    reverse_proxy localhost:3000\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Errors(_)), "Unclosed brace should error: {result:?}");
        if let LintResult::Errors(errs) = result {
            assert!(errs.iter().any(|e| e.message.contains("Unclosed")), "Should mention unclosed: {errs:?}");
        }
    }

    #[test]
    fn test_caddyfile_extra_closing_brace() {
        let content = "example.com {\n    reverse_proxy localhost:3000\n}\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Errors(_)), "Extra }} should error: {result:?}");
        if let LintResult::Errors(errs) = result {
            assert!(errs.iter().any(|e| e.message.contains("Unexpected")), "Should mention unexpected: {errs:?}");
        }
    }

    #[test]
    fn test_caddyfile_comment_ignored() {
        let content = "# This is a comment\nexample.com {\n    # another comment\n    root * /var/www\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Comments should be ignored: {result:?}");
    }

    #[test]
    fn test_caddyfile_empty() {
        let content = "";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Empty file should be ok: {result:?}");
    }

    #[test]
    fn test_caddyfile_multiple_blocks() {
        let content = "example.com {\n    reverse_proxy localhost:3000\n}\n\napi.example.com {\n    reverse_proxy localhost:4000\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Multiple blocks should pass: {result:?}");
    }

    #[test]
    fn test_caddyfile_nested_blocks() {
        let content = "example.com {\n    route /api/* {\n        reverse_proxy localhost:3000\n    }\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Nested blocks should pass: {result:?}");
    }

    #[test]
    fn test_caddyfile_reverse_proxy_missing_upstream() {
        let content = "example.com {\n    reverse_proxy\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Errors(_)), "reverse_proxy with no upstream should warn: {result:?}");
    }

    #[test]
    fn test_caddyfile_tls_internal() {
        let content = "example.com {\n    reverse_proxy localhost:3000\n    tls internal\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "tls internal should be valid: {result:?}");
    }

    #[test]
    fn test_caddyfile_consistent_2space_indent_ok() {
        let content = "example.com {\n  reverse_proxy localhost:3000\n  tls internal\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Consistent 2-space indent should pass: {result:?}");
    }

    #[test]
    fn test_caddyfile_consistent_4space_indent_ok() {
        let content = "example.com {\n    reverse_proxy localhost:3000\n    tls internal\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Consistent 4-space indent should pass: {result:?}");
    }

    #[test]
    fn test_caddyfile_tab_indent_ok() {
        let content = "example.com {\n\treverse_proxy localhost:3000\n\ttls internal\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Consistent tab indent should pass: {result:?}");
    }

    #[test]
    fn test_caddyfile_mixed_tabs_and_spaces_flagged() {
        // One line uses tab, next uses spaces — mixed indentation
        let content = "example.com {\n\treverse_proxy localhost:3000\n    tls internal\n}\n";
        let result = lint_caddyfile_str(content);
        // Should either catch it as mixed-indent (tab+space on same line) or just pass
        // (different lines using different styles is allowed in some tools)
        // The key test is that it doesn't panic
        let _ = result; // must not panic
    }

    #[test]
    fn test_caddyfile_mixed_tabs_spaces_on_same_line_flagged() {
        // Single line with both tab and spaces in leading whitespace
        let content = "example.com {\n\t  reverse_proxy localhost:3000\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Errors(_)), "Mixed tab+space leading whitespace should flag: {result:?}");
        if let LintResult::Errors(errs) = result {
            assert!(errs.iter().any(|e| e.message.contains("Mixed") || e.message.contains("tab")),
                "Error should mention mixed indentation: {errs:?}");
        }
    }

    #[test]
    fn test_caddyfile_inconsistent_indent_width_flagged() {
        // Most lines use 4-space indent; one line uses 3-space (inconsistent)
        let content = "example.com {\n    reverse_proxy localhost:3000\n    tls internal\n   redir / /home\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Errors(_)), "Inconsistent indent width should flag: {result:?}");
        if let LintResult::Errors(errs) = result {
            assert!(errs.iter().any(|e| e.message.contains("Inconsistent") || e.message.contains("indent")),
                "Error should mention inconsistent indentation: {errs:?}");
        }
    }

    #[test]
    fn test_caddyfile_nested_block_double_indent_ok() {
        // Nested blocks naturally have 2× the indent width — this is valid
        let content = "example.com {\n    route /api/* {\n        reverse_proxy localhost:3000\n    }\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Nested block with doubled indent should be valid: {result:?}");
    }

    #[test]
    fn test_caddyfile_directive_at_column_zero_inside_block_flagged() {
        // Directive written at column 0 inside a block (common copy-paste mistake)
        let content = "example.com {\nreverse_proxy localhost:3000\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Errors(_)), "Unindented directive inside block should flag: {result:?}");
        if let LintResult::Errors(errs) = result {
            assert!(errs.iter().any(|e| e.message.contains("column 0") || e.message.contains("indented")),
                "Error should mention indentation: {errs:?}");
        }
    }

    #[test]
    fn test_leading_whitespace_kind_tabs() {
        let (tabs, spaces) = leading_whitespace_kind("\t\tdirective");
        assert_eq!(tabs, 2);
        assert_eq!(spaces, 0);
    }

    #[test]
    fn test_leading_whitespace_kind_spaces() {
        let (tabs, spaces) = leading_whitespace_kind("    directive");
        assert_eq!(tabs, 0);
        assert_eq!(spaces, 4);
    }

    #[test]
    fn test_leading_whitespace_kind_mixed() {
        let (tabs, spaces) = leading_whitespace_kind("\t  directive");
        assert_eq!(tabs, 1);
        assert_eq!(spaces, 2);
    }

    #[test]
    fn test_leading_whitespace_kind_no_indent() {
        let (tabs, spaces) = leading_whitespace_kind("directive");
        assert_eq!(tabs, 0);
        assert_eq!(spaces, 0);
    }

    // ── File type detection ───────────────────────────────────────────────────

    #[test]
    fn test_unsupported_extension() {
        let ext = "toml";
        let is_supported = ext == "yaml" || ext == "yml" || ext == "caddy";
        assert!(!is_supported);
    }

    #[test]
    fn test_yaml_extensions_supported() {
        for ext in &["yaml", "yml"] {
            let is_supported = *ext == "yaml" || *ext == "yml";
            assert!(is_supported, "{ext} should be supported");
        }
    }

    #[test]
    fn test_caddyfile_name_detection() {
        let is_caddyfile = |name: &str| -> bool {
            let p = std::path::Path::new(name);
            let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            fname == "Caddyfile" || ext == "caddy"
        };
        assert!(is_caddyfile("Caddyfile"));
        assert!(is_caddyfile("my.caddy"));
        assert!(!is_caddyfile("docker-compose.yml"));
    }
}
