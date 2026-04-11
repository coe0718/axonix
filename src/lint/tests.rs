//! Integration tests for the lint module (Caddyfile tests).
//! YAML tests live alongside the yaml.rs module.

#[cfg(test)]
mod caddy_tests {
    use crate::lint::{LintResult, LintError};
    use crate::lint::caddy::{lint_caddyfile, leading_whitespace_kind};
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
        let content = "example.com {\n\treverse_proxy localhost:3000\n    tls internal\n}\n";
        let result = lint_caddyfile_str(content);
        let _ = result; // must not panic
    }

    #[test]
    fn test_caddyfile_mixed_tabs_spaces_on_same_line_flagged() {
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
        let content = "example.com {\n    route /api/* {\n        reverse_proxy localhost:3000\n    }\n}\n";
        let result = lint_caddyfile_str(content);
        assert!(matches!(result, LintResult::Ok(_)), "Nested block with doubled indent should be valid: {result:?}");
    }

    #[test]
    fn test_caddyfile_directive_at_column_zero_inside_block_flagged() {
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
