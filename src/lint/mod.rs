//! File linting: YAML (docker compose, etc.) and Caddyfile validation.
//!
//! The `/lint <file>` REPL command delegates here.
//! For YAML: uses serde_yaml (pure Rust) for reliable parsing with line/column error info.
//! For Caddyfile: structural heuristic checks — brace balancing, unterminated blocks,
//!   suspicious patterns. Not a full parser but catches the common mistakes.

mod yaml;
mod caddy;
#[cfg(test)]
mod tests;

pub use yaml::lint_yaml;
pub use caddy::lint_caddyfile;

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
