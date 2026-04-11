#[cfg(test)]
mod tests {
    use super::super::*;
    use super::super::exec::parse_hosts_toml;

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
