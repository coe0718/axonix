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

mod exec;
#[cfg(test)]
mod tests;

pub use exec::ssh_exec;

use std::collections::HashMap;

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
        let loaded = exec::parse_hosts_toml(content)?;
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
