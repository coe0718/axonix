//! SSH command execution and TOML host config parsing.

use std::collections::HashMap;
use std::process::{Command, Output};
use std::time::Duration;

use super::{HostEntry, SshResult};

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
pub fn parse_hosts_toml(content: &str) -> Result<Vec<HostEntry>, String> {
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
