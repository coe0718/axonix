//! `/ssh` command handler extracted from `handle_command`.
//!
//! Named `ssh_cmd` to avoid shadowing the `crate::ssh` module.

use std::time::Duration;
use crate::ssh::ssh_exec;
use super::types::{CommandResult, ReplState};

/// Handle the `/ssh` command and all its subcommands.
///
/// `arg` is the text after `/ssh ` (or `""` when bare `/ssh` is typed).
pub fn handle_ssh(arg: &str, state: &mut ReplState) -> CommandResult {
    if arg.is_empty() || arg == "--help" {
        let hosts: Vec<String> = state.ssh_hosts.aliases().iter().map(|a| a.to_string()).collect();
        let mut lines = vec![
            "  Usage:".to_string(),
            "    /ssh list              List registered hosts".to_string(),
            "    /ssh <host> <command>  Run a command on a remote host".to_string(),
            String::new(),
        ];
        if hosts.is_empty() {
            lines.push("  No hosts registered. Create hosts.toml in the working directory:".to_string());
            lines.push("    [hosts.caddy-nuc]".to_string());
            lines.push("    address = \"192.168.1.10\"".to_string());
            lines.push("    user = \"admin\"".to_string());
        } else {
            lines.push(format!("  Registered hosts: {}", hosts.join(", ")));
        }
        lines.push(String::new());
        CommandResult::Handled(lines)
    } else if arg == "list" {
        if state.ssh_hosts.is_empty() {
            CommandResult::Handled(vec![
                "  No hosts registered.".to_string(),
                "  Create hosts.toml in the working directory or ~/.axonix/hosts.toml".to_string(),
                "  Example:".to_string(),
                "    [hosts.caddy-nuc]".to_string(),
                "    address = \"192.168.1.10\"".to_string(),
                "    user = \"admin\"".to_string(),
                String::new(),
            ])
        } else {
            let mut lines = vec![format!("  SSH Hosts ({} registered):", state.ssh_hosts.len())];
            for alias in state.ssh_hosts.aliases() {
                if let Some(host) = state.ssh_hosts.get(alias) {
                    let mut desc = format!("    {:<20} {}", alias, host.destination());
                    if host.port != 22 {
                        desc.push_str(&format!(":{}", host.port));
                    }
                    if let Some(d) = &host.description {
                        desc.push_str(&format!("  — {d}"));
                    }
                    lines.push(desc);
                }
            }
            lines.push(String::new());
            CommandResult::Handled(lines)
        }
    } else {
        // "/ssh <host> <command>"
        let mut parts = arg.splitn(2, ' ');
        let host_alias = parts.next().unwrap_or("").trim();
        let remote_cmd = parts.next().unwrap_or("").trim();

        if host_alias.is_empty() {
            return CommandResult::Handled(vec![
                "  Usage: /ssh <host> <command>".to_string(),
                "  Use /ssh list to see registered hosts".to_string(),
                String::new(),
            ]);
        }

        if remote_cmd.is_empty() {
            return CommandResult::Handled(vec![
                format!("  Usage: /ssh {host_alias} <command>"),
                "  Example: /ssh caddy-nuc systemctl reload caddy".to_string(),
                String::new(),
            ]);
        }

        match state.ssh_hosts.get(host_alias) {
            None => CommandResult::Handled(vec![
                format!("  Unknown host: '{host_alias}'"),
                "  Use /ssh list to see registered hosts".to_string(),
                String::new(),
            ]),
            Some(host) => {
                let host = host.clone();
                match ssh_exec(&host, remote_cmd, Some(Duration::from_secs(15))) {
                    Err(e) => CommandResult::Handled(vec![
                        format!("__ssh_error:{host_alias}:{e}"),
                    ]),
                    Ok(result) => {
                        CommandResult::Handled(vec![
                            format!("__ssh_result:{}:{}:{}", host_alias, result.exit_code, result.combined_output()),
                        ])
                    }
                }
            }
        }
    }
}
