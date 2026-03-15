//! SSH multi-device management for home lab machines.
//!
//! Maps friendly host aliases (e.g. "caddy-nuc") to real SSH addresses
//! and provides `ssh_exec` to run commands on remote hosts.
//!
//! Host configuration is loaded from `~/.axonix/hosts.toml` or
//! `./hosts.toml` in the working directory, with a built-in fallback
//! for the known home lab machines.
//!
//! # Example hosts.toml
//!
//! ```toml
//! [hosts.caddy-nuc]
//! address = "192.168.1.10"
//! user = "admin"
//! port = 22
//!
//! [hosts.media-server]
//! address = "192.168.1.20"
//! user = "ubuntu"
//! ```

use std::collections::HashMap;
use std::process::{Command, Output};
use std::time::Duration;

/// A registered remote host.
#[derive(Debug, Clone, PartialEq)]
pub struct HostEntry {
    /// Friendly alias (e.g. "caddy-nuc").
    pub alias: String,
    /// SSH address — hostname or IP.
    pub address: String,
    /// SSH user (default: current user).
    pub user: Option<String>,
    /// SSH port (default: 22).
    pub port: u16,
    /// Optional description for `/ssh --list` output.
    pub description: Option<String>,
}

impl HostEntry {
    pub fn new(alias: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            alias: alias.into(),
            address: address.into(),
            user: None,
            port: 22,
            description: None,
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Build the SSH destination string (user@host or just host).
    pub fn destination(&self) -> String {
        match &self.user {
            Some(u) => format!("{}@{}", u, self.address),
            None => self.address.clone(),
        }
    }
}

/// Registry of known remote hosts.
#[derive(Debug, Default)]
pub struct HostRegistry {
    hosts: HashMap<String, HostEntry>,
}

impl HostRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a host entry.
    pub fn add(&mut self, entry: HostEntry) {
        self.hosts.insert(entry.alias.clone(), entry);
    }

    /// Look up a host by alias.
    pub fn get(&self, alias: &str) -> Option<&HostEntry> {
        self.hosts.get(alias)
    }

    /// All registered aliases, sorted.
    pub fn aliases(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.hosts.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    /// Load host config from a TOML file. Returns error string on failure.
    /// TOML format:
    /// ```toml
    /// [hosts.caddy-nuc]
    /// address = "192.168.1.10"
    /// user = "admin"
    /// port = 22
    /// description = "Caddy reverse proxy NUC"
    /// ```
    pub fn load_toml(&mut self, content: &str) -> Result<usize, String> {
        let loaded = parse_hosts_toml(content)?;
        let count = loaded.len();
        for entry in loaded {
            self.add(entry);
        }
        Ok(count)
    }

    /// Load from a file path. Silently returns 0 if file doesn't exist.
    pub fn load_file(&mut self, path: &str) -> Result<usize, String> {
        match std::fs::read_to_string(path) {
            Ok(content) => self.load_toml(&content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(format!("Cannot read {path}: {e}")),
        }
    }

    /// Load from default locations: ./hosts.toml, ~/.axonix/hosts.toml
    pub fn load_defaults(&mut self) -> usize {
        let mut count = 0;
        // Current dir first
        count += self.load_file("hosts.toml").unwrap_or(0);
        // User config dir
        if let Some(home) = std::env::var_os("HOME") {
            let path = format!("{}/.axonix/hosts.toml", home.to_string_lossy());
            count += self.load_file(&path).unwrap_or(0);
        }
        count
    }
}

/// Result of an SSH command execution.
#[derive(Debug)]
pub struct SshResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub host_alias: String,
    pub command: String,
}

impl SshResult {
    pub fn succeeded(&self) -> bool {
        self.exit_code == 0
    }

    /// Combine stdout+stderr into a single string for display.
    pub fn combined_output(&self) -> String {
        let mut out = self.stdout.clone();
        if !self.stderr.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&self.stderr);
        }
        out
    }
}

/// Execute a command on a remote host via SSH.
///
/// Uses the system `ssh` binary with:
/// - `BatchMode=yes` (no password prompts — key auth only)
/// - `StrictHostKeyChecking=accept-new` (auto-accept new hosts)
/// - `ConnectTimeout=10` (fail fast if host unreachable)
pub fn ssh_exec(
    host: &HostEntry,
    command: &str,
    timeout: Option<Duration>,
) -> Result<SshResult, String> {
    let timeout_secs = timeout
        .map(|d| d.as_secs().max(1))
        .unwrap_or(10);

    let mut cmd = Command::new("ssh");
    cmd.args([
        "-o", "BatchMode=yes",
        "-o", "StrictHostKeyChecking=accept-new",
        "-o", &format!("ConnectTimeout={timeout_secs}"),
        "-p", &host.port.to_string(),
        &host.destination(),
        command,
    ]);

    let output: Output = cmd
        .output()
        .map_err(|e| format!("Failed to spawn ssh: {e}"))?;

    Ok(SshResult {
        stdout: String::from_utf8_lossy(&output.stdout).trim_end().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim_end().to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        host_alias: host.alias.clone(),
        command: command.to_string(),
    })
}

/// Parse a minimal subset of TOML for host configuration.
/// This is a hand-rolled parser to avoid adding a toml dependency.
/// Supports only the [hosts.<alias>] / key = "value" format we need.
fn parse_hosts_toml(content: &str) -> Result<Vec<HostEntry>, String> {
    let mut entries: Vec<HostEntry> = Vec::new();
    let mut current_alias: Option<String> = None;
    let mut current_fields: HashMap<String, String> = HashMap::new();

    for (lineno, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Section header: [hosts.alias-name]
        if trimmed.starts_with("[hosts.") && trimmed.ends_with(']') {
            // Flush previous entry
            if let Some(alias) = current_alias.take() {
                entries.push(build_entry(alias, &current_fields)?);
                current_fields.clear();
            }
            let alias = &trimmed[7..trimmed.len() - 1];
            if alias.is_empty() {
                return Err(format!("line {}: empty host alias", lineno + 1));
            }
            current_alias = Some(alias.to_string());
        } else if trimmed.starts_with('[') {
            // Some other section — flush and stop tracking
            if let Some(alias) = current_alias.take() {
                entries.push(build_entry(alias, &current_fields)?);
                current_fields.clear();
            }
        } else if let Some(eq_pos) = trimmed.find('=') {
            // key = "value"
            let key = trimmed[..eq_pos].trim().to_string();
            let val = trimmed[eq_pos + 1..].trim().trim_matches('"').to_string();
            if current_alias.is_some() {
                current_fields.insert(key, val);
            }
        }
    }

    // Flush last entry
    if let Some(alias) = current_alias {
        entries.push(build_entry(alias, &current_fields)?);
    }

    Ok(entries)
}

fn build_entry(alias: String, fields: &HashMap<String, String>) -> Result<HostEntry, String> {
    let address = fields
        .get("address")
        .ok_or_else(|| format!("host '{}' missing required field 'address'", alias))?
        .clone();

    let mut entry = HostEntry::new(alias, address);
    if let Some(user) = fields.get("user") {
        entry = entry.with_user(user);
    }
    if let Some(port_str) = fields.get("port") {
        let port: u16 = port_str
            .parse()
            .map_err(|_| format!("invalid port '{}' — must be 1-65535", port_str))?;
        entry = entry.with_port(port);
    }
    if let Some(desc) = fields.get("description") {
        entry = entry.with_description(desc);
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── HostEntry ──────────────────────────────────────────────────────────────

    #[test]
    fn test_host_entry_destination_with_user() {
        let h = HostEntry::new("caddy-nuc", "192.168.1.10").with_user("admin");
        assert_eq!(h.destination(), "admin@192.168.1.10");
    }

    #[test]
    fn test_host_entry_destination_no_user() {
        let h = HostEntry::new("caddy-nuc", "192.168.1.10");
        assert_eq!(h.destination(), "192.168.1.10");
    }

    #[test]
    fn test_host_entry_default_port() {
        let h = HostEntry::new("host", "1.2.3.4");
        assert_eq!(h.port, 22);
    }

    #[test]
    fn test_host_entry_custom_port() {
        let h = HostEntry::new("host", "1.2.3.4").with_port(2222);
        assert_eq!(h.port, 2222);
    }

    // ── HostRegistry ──────────────────────────────────────────────────────────

    #[test]
    fn test_registry_add_and_get() {
        let mut reg = HostRegistry::new();
        reg.add(HostEntry::new("my-host", "10.0.0.1"));
        let h = reg.get("my-host").unwrap();
        assert_eq!(h.address, "10.0.0.1");
    }

    #[test]
    fn test_registry_get_missing_returns_none() {
        let reg = HostRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_aliases_sorted() {
        let mut reg = HostRegistry::new();
        reg.add(HostEntry::new("zebra", "10.0.0.3"));
        reg.add(HostEntry::new("alpha", "10.0.0.1"));
        reg.add(HostEntry::new("beta", "10.0.0.2"));
        let aliases = reg.aliases();
        assert_eq!(aliases, vec!["alpha", "beta", "zebra"]);
    }

    #[test]
    fn test_registry_empty() {
        let reg = HostRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    // ── TOML parser ───────────────────────────────────────────────────────────

    #[test]
    fn test_parse_toml_single_host() {
        let toml = r#"
[hosts.caddy-nuc]
address = "192.168.1.10"
user = "admin"
port = 22
description = "Caddy reverse proxy NUC"
"#;
        let entries = parse_hosts_toml(toml).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.alias, "caddy-nuc");
        assert_eq!(e.address, "192.168.1.10");
        assert_eq!(e.user.as_deref(), Some("admin"));
        assert_eq!(e.port, 22);
        assert_eq!(e.description.as_deref(), Some("Caddy reverse proxy NUC"));
    }

    #[test]
    fn test_parse_toml_multiple_hosts() {
        let toml = r#"
[hosts.nuc1]
address = "192.168.1.10"

[hosts.nuc2]
address = "192.168.1.20"
user = "ubuntu"
"#;
        let entries = parse_hosts_toml(toml).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.alias == "nuc1"));
        assert!(entries.iter().any(|e| e.alias == "nuc2"));
    }

    #[test]
    fn test_parse_toml_missing_address_errors() {
        let toml = "[hosts.badhost]\nuser = \"admin\"\n";
        let result = parse_hosts_toml(toml);
        assert!(result.is_err(), "Missing address should error");
        assert!(result.unwrap_err().contains("address"));
    }

    #[test]
    fn test_parse_toml_invalid_port_errors() {
        let toml = "[hosts.h]\naddress = \"1.2.3.4\"\nport = \"notaport\"\n";
        let result = parse_hosts_toml(toml);
        assert!(result.is_err(), "Invalid port should error");
    }

    #[test]
    fn test_parse_toml_comments_ignored() {
        let toml = "# this is a comment\n[hosts.h]\n# another comment\naddress = \"1.2.3.4\"\n";
        let entries = parse_hosts_toml(toml).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].address, "1.2.3.4");
    }

    #[test]
    fn test_parse_toml_empty_content() {
        let entries = parse_hosts_toml("").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_registry_load_toml() {
        let toml = "[hosts.test]\naddress = \"10.0.0.1\"\n";
        let mut reg = HostRegistry::new();
        let count = reg.load_toml(toml).unwrap();
        assert_eq!(count, 1);
        assert!(reg.get("test").is_some());
    }

    #[test]
    fn test_registry_load_nonexistent_file_ok() {
        let mut reg = HostRegistry::new();
        let count = reg.load_file("/tmp/definitely_does_not_exist_axonix.toml").unwrap();
        assert_eq!(count, 0, "Missing file should silently return 0");
    }

    // ── SshResult ─────────────────────────────────────────────────────────────

    #[test]
    fn test_ssh_result_succeeded() {
        let r = SshResult {
            stdout: "hello".into(),
            stderr: "".into(),
            exit_code: 0,
            host_alias: "h".into(),
            command: "echo hello".into(),
        };
        assert!(r.succeeded());
    }

    #[test]
    fn test_ssh_result_failed() {
        let r = SshResult {
            stdout: "".into(),
            stderr: "connection refused".into(),
            exit_code: 1,
            host_alias: "h".into(),
            command: "ls".into(),
        };
        assert!(!r.succeeded());
    }

    #[test]
    fn test_ssh_result_combined_output_stdout_only() {
        let r = SshResult {
            stdout: "output".into(),
            stderr: "".into(),
            exit_code: 0,
            host_alias: "h".into(),
            command: "cmd".into(),
        };
        assert_eq!(r.combined_output(), "output");
    }

    #[test]
    fn test_ssh_result_combined_output_with_stderr() {
        let r = SshResult {
            stdout: "out".into(),
            stderr: "err".into(),
            exit_code: 0,
            host_alias: "h".into(),
            command: "cmd".into(),
        };
        assert_eq!(r.combined_output(), "out\nerr");
    }
}
