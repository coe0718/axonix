//! Caddy reverse proxy health via the admin API.

use std::process::Command;

/// Caddy reverse proxy health information from the admin API.
#[derive(Debug, Clone)]
pub struct CaddyHealth {
    /// Whether the Caddy admin API responded.
    pub reachable: bool,
    /// Number of upstreams monitored by Caddy's reverse proxy.
    pub upstream_count: usize,
    /// Number of unhealthy upstreams (empty = all healthy or no upstreams).
    pub unhealthy: Vec<String>,
    /// Error message if the admin API was unreachable.
    pub error: Option<String>,
}

impl CaddyHealth {
    pub fn format(&self) -> String {
        if !self.reachable {
            return format!(
                "🔴 Caddy: unreachable ({})",
                self.error.as_deref().unwrap_or("unknown")
            );
        }
        if self.unhealthy.is_empty() {
            format!("✅ Caddy: {} upstream(s) healthy", self.upstream_count)
        } else {
            format!(
                "⚠️ Caddy: {}/{} upstreams unhealthy: {}",
                self.unhealthy.len(),
                self.upstream_count,
                self.unhealthy.join(", ")
            )
        }
    }
}

/// Query the Caddy admin API for reverse proxy upstream health.
///
/// Uses the `CADDY_ADMIN_URL` environment variable (default: `http://localhost:2019`).
/// Falls back gracefully when Caddy is not running.
pub fn caddy_health() -> CaddyHealth {
    let admin_url = std::env::var("CADDY_ADMIN_URL")
        .unwrap_or_else(|_| "http://localhost:2019".to_string());
    let url = format!("{}/reverse_proxy/upstreams", admin_url.trim_end_matches('/'));

    let output = Command::new("curl")
        .args(["-sf", "--max-time", "3", &url])
        .output();

    match output {
        Err(e) => CaddyHealth {
            reachable: false,
            upstream_count: 0,
            unhealthy: vec![],
            error: Some(e.to_string()),
        },
        Ok(out) if !out.status.success() => {
            let err = String::from_utf8_lossy(&out.stderr).to_string();
            CaddyHealth {
                reachable: false,
                upstream_count: 0,
                unhealthy: vec![],
                error: Some(if err.is_empty() {
                    "curl failed".to_string()
                } else {
                    err
                }),
            }
        }
        Ok(out) => {
            let body = String::from_utf8_lossy(&out.stdout);
            parse_caddy_upstreams(&body)
        }
    }
}

/// Parse a JSON array of Caddy upstream objects.
///
/// Format: `[{"address":"host:port","num_requests":0,"fails":0}, ...]`
pub(super) fn parse_caddy_upstreams(json: &str) -> CaddyHealth {
    let trimmed = json.trim();
    if trimmed == "null" || trimmed.is_empty() {
        return CaddyHealth {
            reachable: true,
            upstream_count: 0,
            unhealthy: vec![],
            error: None,
        };
    }
    let mut upstream_count = 0;
    let mut unhealthy = vec![];
    // Split by "address" to find each upstream entry
    for entry in trimmed.split("\"address\"") {
        if !entry.contains(':') {
            continue;
        }
        // Extract address value (first quoted string after the colon)
        let addr = entry.split('"').nth(1).unwrap_or("unknown").to_string();
        // Check if fails > 0: look for "fails":N where N > 0
        let has_fails = entry.contains("\"fails\":") && {
            let after_fails = entry.split("\"fails\":").nth(1).unwrap_or("0");
            let fails_str: String = after_fails
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            fails_str.parse::<u64>().unwrap_or(0) > 0
        };
        if !addr.is_empty() && addr != "unknown" {
            upstream_count += 1;
            if has_fails {
                unhealthy.push(addr);
            }
        }
    }
    CaddyHealth {
        reachable: true,
        upstream_count,
        unhealthy,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_caddy_health_format_unreachable() {
        let h = CaddyHealth {
            reachable: false,
            upstream_count: 0,
            unhealthy: vec![],
            error: Some("connection refused".to_string()),
        };
        let fmt = h.format();
        assert!(fmt.contains("unreachable"), "should say unreachable: {fmt}");
        assert!(fmt.contains("connection refused"), "should include error: {fmt}");
    }

    #[test]
    fn test_caddy_health_format_all_healthy() {
        let h = CaddyHealth {
            reachable: true,
            upstream_count: 3,
            unhealthy: vec![],
            error: None,
        };
        let fmt = h.format();
        assert!(fmt.contains('3'), "should contain upstream count: {fmt}");
        assert!(fmt.contains("healthy"), "should say healthy: {fmt}");
    }

    #[test]
    fn test_caddy_health_format_some_unhealthy() {
        let h = CaddyHealth {
            reachable: true,
            upstream_count: 3,
            unhealthy: vec!["host:8080".to_string()],
            error: None,
        };
        let fmt = h.format();
        assert!(fmt.contains("unhealthy"), "should mention unhealthy: {fmt}");
        assert!(fmt.contains("host:8080"), "should list unhealthy host: {fmt}");
    }

    #[test]
    fn test_parse_caddy_upstreams_empty_null() {
        let result = parse_caddy_upstreams("null");
        assert_eq!(result.upstream_count, 0);
        assert!(result.reachable);
        assert!(result.unhealthy.is_empty());
    }

    #[test]
    fn test_parse_caddy_upstreams_all_healthy() {
        let json = r#"[{"address":"host:8080","num_requests":0,"fails":0}]"#;
        let result = parse_caddy_upstreams(json);
        assert_eq!(result.upstream_count, 1, "should count 1 upstream");
        assert!(result.unhealthy.is_empty(), "no unhealthy upstreams");
        assert!(result.reachable);
    }

    #[test]
    fn test_parse_caddy_upstreams_with_failures() {
        let json = r#"[{"address":"host:8080","num_requests":5,"fails":2},{"address":"host:8081","num_requests":3,"fails":0}]"#;
        let result = parse_caddy_upstreams(json);
        assert_eq!(result.upstream_count, 2, "should count 2 upstreams");
        assert_eq!(result.unhealthy.len(), 1, "should have 1 unhealthy upstream");
        assert_eq!(result.unhealthy[0], "host:8080", "host:8080 should be unhealthy");
    }
}
