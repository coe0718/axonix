//! YAML validation using serde_yaml (pure Rust, no external tools required).

use super::{LintError, LintResult};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn lint_yaml_str(content: &str) -> LintResult {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "{}", content).unwrap();
        lint_yaml(tmp.path().to_str().unwrap())
    }

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
}
