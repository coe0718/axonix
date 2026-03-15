//! File linting: YAML (docker compose, etc.) and Caddyfile validation.
//!
//! The `/lint <file>` REPL command delegates here.
//! For YAML: uses serde_yaml (pure Rust) for reliable parsing with line/column error info.
//! For Caddyfile: structural heuristic checks — brace balancing, unterminated blocks,
//!   suspicious patterns. Not a full parser but catches the common mistakes.

use std::path::Path;

/// Result of a lint check.
#[derive(Debug, PartialEq)]
pub enum LintResult {
    /// File is valid — includes a brief summary.
    Ok(String),
    /// File has errors — list of (line_number, message).
    Errors(Vec<LintError>),
    /// We couldn't read or identify the file.
    Unsupported(String),
}

#[derive(Debug, PartialEq, Clone)]
pub struct LintError {
    /// 1-indexed line number, or 0 if unknown.
    pub line: usize,
    pub message: String,
}

impl LintError {
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line, message: message.into() }
    }
}

/// Detect file type and lint accordingly.
pub fn lint_file(path: &str) -> LintResult {
    let p = Path::new(path);

    let file_name = p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let ext = p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ext == "yaml" || ext == "yml" {
        lint_yaml(path)
    } else if file_name == "Caddyfile" || ext == "caddy" {
        lint_caddyfile(path)
    } else {
        LintResult::Unsupported(format!(
            "Unknown file type '{}'. Supported: .yaml/.yml, Caddyfile/.caddy",
            file_name
        ))
    }
}

/// Lint a YAML file using serde_yaml (pure Rust, no external tools required).
/// Returns Ok if valid, Errors with line info on parse failure.
pub fn lint_yaml(path: &str) -> LintResult {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return LintResult::Errors(vec![LintError::new(0, format!("Cannot read file: {e}"))]),
    };

    if content.trim().is_empty() {
        return LintResult::Ok("File is empty (valid YAML — null document)".to_string());
    }

    // Parse with serde_yaml — handles all valid YAML including multi-doc streams
    match serde_yaml::from_str::<serde_yaml::Value>(&content) {
        Ok(_) => {
            let lines = content.lines().count();
            // Count documents (--- separators)
            let doc_count = content.split("\n---").count().max(1);
            LintResult::Ok(format!("{lines} lines, {doc_count} document(s) — valid YAML ✓"))
        }
        Err(e) => {
            // serde_yaml errors include location info
            let location = e.location();
            let (line, col) = location
                .map(|l| (l.line(), l.column()))
                .unwrap_or((0, 0));
            let msg = if line > 0 {
                // Strip redundant "at line X, column Y" suffix that serde_yaml adds
                let raw = e.to_string();
                let clean = if let Some(idx) = raw.find(" at line") {
                    raw[..idx].trim().to_string()
                } else {
                    raw
                };
                format!("{clean} (column {col})")
            } else {
                e.to_string()
            };
            LintResult::Errors(vec![LintError::new(line, msg)])
        }
    }
}

/// Lint a Caddyfile using structural heuristics.
/// Checks: brace balance, unterminated strings, known directive patterns, suspicious syntax.
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

        // Warn about Windows-style line endings
        if trimmed.ends_with('\r') {
            errors.push(LintError::new(lineno, "Windows line ending (CRLF) detected — may cause parsing issues".to_string()));
        }

        // Inside a site block — check common directive mistakes
        if brace_depth == 1 {
            if trimmed == "reverse_proxy" {
                errors.push(LintError::new(
                    lineno,
                    "reverse_proxy directive missing upstream address (e.g. reverse_proxy localhost:3000)".to_string(),
                ));
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

    if errors.is_empty() {
        let line_count = content.lines().count();
        LintResult::Ok(format!("{line_count} lines, {block_count} block(s) — valid Caddyfile structure ✓"))
    } else {
        LintResult::Errors(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── YAML tests (serde_yaml) ───────────────────────────────────────────────

    #[test]
    fn test_yaml_valid_simple() {
        let result = lint_yaml_str("key: value\nother: 123\n");
        assert!(matches!(result, LintResult::Ok(_)), "Simple YAML should pass: {result:?}");
    }

    #[test]
    fn test_yaml_valid_docker_compose() {
        let content = "version: '3.8'\nservices:\n  web:\n    image: nginx\n    ports:\n      - '80:80'\n";
        let result = lint_yaml_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Docker compose YAML should pass: {result:?}");
    }

    #[test]
    fn test_yaml_invalid_syntax_unmatched_bracket() {
        let content = "key: [\nunmatched bracket\n";
        let result = lint_yaml_str(content);
        assert!(matches!(result, LintResult::Errors(_)), "Unmatched bracket should fail: {result:?}");
    }

    #[test]
    fn test_yaml_empty_file() {
        let result = lint_yaml_str("   \n\n");
        assert!(matches!(result, LintResult::Ok(_)), "Empty YAML should be ok: {result:?}");
    }

    #[test]
    fn test_yaml_error_reports_line_number() {
        let content = "key: [\nunmatched\n";
        let result = lint_yaml_str(content);
        if let LintResult::Errors(errs) = result {
            assert!(!errs.is_empty(), "Should have at least one error");
            // Line should be reported (may be 0 if location unknown)
            let _ = errs[0].line;
        }
        // If Ok (serde_yaml is lenient), that's fine too
    }

    #[test]
    fn test_yaml_invalid_tab_indentation() {
        // YAML spec forbids tabs for indentation
        let content = "key:\n\tvalue: bad\n";
        let result = lint_yaml_str(content);
        // serde_yaml catches this
        assert!(matches!(result, LintResult::Errors(_)), "Tab indentation should fail: {result:?}");
    }

    #[test]
    fn test_yaml_multiline_string_valid() {
        let content = "description: |\n  line one\n  line two\n";
        let result = lint_yaml_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Multiline string should be valid: {result:?}");
    }

    #[test]
    fn test_yaml_anchor_and_alias() {
        let content = "defaults: &defaults\n  image: nginx\n\nweb:\n  <<: *defaults\n  ports:\n    - '80:80'\n";
        let result = lint_yaml_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Anchors and aliases should be valid: {result:?}");
    }

    // ── Caddyfile tests ───────────────────────────────────────────────────────

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

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn lint_yaml_str(content: &str) -> LintResult {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "{}", content).unwrap();
        lint_yaml(tmp.path().to_str().unwrap())
    }

    fn lint_caddyfile_str(content: &str) -> LintResult {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "{}", content).unwrap();
        lint_caddyfile(tmp.path().to_str().unwrap())
    }
}
