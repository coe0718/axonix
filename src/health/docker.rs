//! Docker container health via the Docker HTTP API.

use std::process::Command;

/// Status of a single Docker container.
#[derive(Debug, Clone)]
pub struct ContainerStatus {
    /// Container name (e.g. "axonix", "axonix-listener")
    pub name: String,
    /// Container state: "running", "exited", "paused", etc.
    pub state: String,
    /// Container status string (e.g. "Up 2 days", "Exited (0) 3 minutes ago")
    pub status: String,
    /// Whether the container is healthy (state == "running")
    pub healthy: bool,
    /// True if the container has a health check anomaly:
    /// state is "restarting", or the status string contains "(unhealthy)".
    /// A container can be `healthy=true` (running) yet still have `anomaly=true`
    /// when Docker reports an unhealthy health-check result.
    pub anomaly: bool,
}

/// Health summary of all Docker containers.
#[derive(Debug, Clone)]
pub struct DockerHealth {
    pub containers: Vec<ContainerStatus>,
    /// Error message if Docker was unreachable
    pub error: Option<String>,
}

impl DockerHealth {
    pub fn healthy_count(&self) -> usize {
        self.containers.iter().filter(|c| c.healthy).count()
    }

    pub fn total_count(&self) -> usize {
        self.containers.len()
    }

    pub fn all_healthy(&self) -> bool {
        self.containers.iter().all(|c| c.healthy)
    }

    pub fn format(&self) -> String {
        if let Some(ref err) = self.error {
            return format!("🐳 Containers: (unavailable — {})", err);
        }
        let healthy = self.healthy_count();
        let total = self.total_count();
        let icon = if healthy == total { "🟢" } else { "🔴" };
        let mut out = format!("🐳 Containers: {} {}/{} running\n", icon, healthy, total);
        for c in &self.containers {
            let marker = if !c.healthy { "  ✗" } else if c.anomaly { "  ⚠" } else { "  ✓" };
            out.push_str(&format!("{} {} — {}\n", marker, c.name, c.status));
        }
        out.trim_end().to_string()
    }

    pub fn format_compact(&self) -> String {
        if let Some(ref err) = self.error {
            return format!("containers: unavailable ({})", err);
        }
        format!("{}/{} containers running", self.healthy_count(), self.total_count())
    }
}

/// Query the Docker API for container status.
///
/// Reads `DOCKER_HOST` env var (default: `http://localhost:2375`).
/// If DOCKER_HOST starts with "tcp://", converts it to "http://".
/// Falls back gracefully when Docker is not reachable.
pub fn docker_health() -> DockerHealth {
    let docker_host = std::env::var("DOCKER_HOST")
        .unwrap_or_else(|_| "http://localhost:2375".to_string());

    let base_url = if docker_host.starts_with("tcp://") {
        docker_host.replacen("tcp://", "http://", 1)
    } else {
        docker_host
    };

    let url = format!("{}/containers/json?all=1", base_url.trim_end_matches('/'));

    let output = Command::new("curl")
        .args(["-sf", "--max-time", "3", &url])
        .output();

    match output {
        Err(e) => DockerHealth {
            containers: vec![],
            error: Some(e.to_string()),
        },
        Ok(out) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            DockerHealth {
                containers: vec![],
                error: Some(if stderr.is_empty() {
                    "curl failed".to_string()
                } else {
                    stderr.trim().to_string()
                }),
            }
        }
        Ok(out) => {
            let body = String::from_utf8_lossy(&out.stdout);
            parse_docker_containers(&body)
        }
    }
}

/// Parse Docker's `GET /containers/json?all=1` JSON response.
///
/// Format: `[{"Names":["/axonix"],"State":"running","Status":"Up 2 days"}, ...]`
pub(super) fn parse_docker_containers(json: &str) -> DockerHealth {
    let trimmed = json.trim();
    if trimmed == "null" || trimmed.is_empty() {
        return DockerHealth { containers: vec![], error: None };
    }

    let mut containers = Vec::new();

    // Split by object boundaries — each container is a JSON object {...}
    // We look for "Names", "State", and "Status" fields in each object.
    // Strategy: split on "Names" to get one chunk per container.
    for entry in trimmed.split("\"Names\"") {
        if !entry.contains("\"State\"") {
            continue;
        }

        // Extract the first name from the Names array: ["/axonix", ...]
        let name = extract_first_name_value(entry);
        if name.is_empty() {
            continue;
        }

        let state = extract_json_string_value(entry, "\"State\"");
        let status = extract_json_string_value(entry, "\"Status\"");

        let healthy = state == "running";
        let anomaly = state == "restarting" || status.contains("(unhealthy)");
        containers.push(ContainerStatus { name, state, status, healthy, anomaly });
    }

    DockerHealth { containers, error: None }
}

/// Extract the first container name from a Docker Names array fragment.
/// Returns the name with the leading "/" stripped.
fn extract_first_name_value(fragment: &str) -> String {
    if let Some(bracket_pos) = fragment.find('[') {
        let after_bracket = &fragment[bracket_pos + 1..];
        if let Some(q1) = after_bracket.find('"') {
            let after_q1 = &after_bracket[q1 + 1..];
            if let Some(q2) = after_q1.find('"') {
                let raw = &after_q1[..q2];
                return raw.trim_start_matches('/').to_string();
            }
        }
    }
    String::new()
}

/// Extract the string value of a JSON field from a fragment.
fn extract_json_string_value(fragment: &str, key: &str) -> String {
    if let Some(key_pos) = fragment.find(key) {
        let after_key = &fragment[key_pos + key.len()..];
        if let Some(colon_pos) = after_key.find(':') {
            let after_colon = after_key[colon_pos + 1..].trim_start();
            if after_colon.starts_with('"') {
                let inner = &after_colon[1..];
                if let Some(end_q) = inner.find('"') {
                    return inner[..end_q].to_string();
                }
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_container(name: &str, state: &str, status: &str) -> ContainerStatus {
        ContainerStatus {
            name: name.to_string(),
            state: state.to_string(),
            status: status.to_string(),
            healthy: state == "running",
            anomaly: state == "restarting" || status.contains("(unhealthy)"),
        }
    }

    #[test]
    fn test_docker_health_format_all_healthy() {
        let dh = DockerHealth {
            containers: vec![
                make_container("axonix", "running", "Up 2 days"),
                make_container("axonix-listener", "running", "Up 2 days"),
                make_container("axonix-stream", "running", "Up 2 days"),
                make_container("axonix-dockerproxy", "running", "Up 2 days"),
            ],
            error: None,
        };
        let fmt = dh.format();
        assert!(fmt.contains("4/4"), "should show 4/4 running: {fmt}");
        assert!(fmt.contains("🟢"), "should show green icon when all healthy: {fmt}");
        assert!(fmt.contains("axonix"), "should list container names: {fmt}");
    }

    #[test]
    fn test_docker_health_format_some_unhealthy() {
        let dh = DockerHealth {
            containers: vec![
                make_container("axonix", "running", "Up 2 days"),
                make_container("axonix-listener", "exited", "Exited (1) 3 minutes ago"),
            ],
            error: None,
        };
        let fmt = dh.format();
        assert!(fmt.contains("🔴"), "should show red icon when some unhealthy: {fmt}");
        assert!(fmt.contains("✗"), "should show ✗ for stopped container: {fmt}");
        assert!(fmt.contains("1/2"), "should show 1/2 running: {fmt}");
    }

    #[test]
    fn test_docker_health_format_empty() {
        let dh = DockerHealth { containers: vec![], error: None };
        let fmt = dh.format();
        assert!(!fmt.is_empty(), "format should return something even with 0 containers: {fmt}");
        assert!(fmt.contains("0/0"), "should show 0/0: {fmt}");
    }

    #[test]
    fn test_docker_health_format_compact_all_healthy() {
        let dh = DockerHealth {
            containers: vec![
                make_container("a", "running", "Up 1 day"),
                make_container("b", "running", "Up 1 day"),
                make_container("c", "running", "Up 1 day"),
                make_container("d", "running", "Up 1 day"),
            ],
            error: None,
        };
        let compact = dh.format_compact();
        assert!(compact.contains("4/4"), "compact should show 4/4: {compact}");
        assert!(compact.contains("containers running"), "compact should say 'containers running': {compact}");
    }

    #[test]
    fn test_docker_health_format_compact_error() {
        let dh = DockerHealth {
            containers: vec![],
            error: Some("connection refused".to_string()),
        };
        let compact = dh.format_compact();
        assert!(compact.contains("unavailable"), "compact error should say unavailable: {compact}");
        assert!(compact.contains("connection refused"), "compact error should include the error: {compact}");
    }

    #[test]
    fn test_docker_health_all_healthy_true() {
        let dh = DockerHealth {
            containers: vec![
                make_container("a", "running", "Up 1 day"),
                make_container("b", "running", "Up 1 day"),
            ],
            error: None,
        };
        assert!(dh.all_healthy(), "all_healthy() should be true when all containers are running");
    }

    #[test]
    fn test_docker_health_all_healthy_false() {
        let dh = DockerHealth {
            containers: vec![
                make_container("a", "running", "Up 1 day"),
                make_container("b", "exited", "Exited (0) 1 hour ago"),
            ],
            error: None,
        };
        assert!(!dh.all_healthy(), "all_healthy() should be false when any container is not running");
    }

    #[test]
    fn test_docker_health_counts() {
        let dh = DockerHealth {
            containers: vec![
                make_container("a", "running", "Up 1 day"),
                make_container("b", "exited", "Exited (0) 1 hour ago"),
                make_container("c", "running", "Up 3 days"),
            ],
            error: None,
        };
        assert_eq!(dh.healthy_count(), 2, "healthy_count() should be 2");
        assert_eq!(dh.total_count(), 3, "total_count() should be 3");
    }

    #[test]
    fn test_docker_health_no_panic() {
        let result = docker_health();
        if result.error.is_some() {
            assert!(result.containers.is_empty(), "on error, containers should be empty");
        }
        let _ = result.format();
        let _ = result.format_compact();
    }

    #[test]
    fn test_container_status_anomaly_restarting() {
        let c = make_container("axonix", "restarting", "Restarting (1) 3 seconds ago");
        assert!(c.anomaly, "restarting state should set anomaly=true");
        assert!(!c.healthy, "restarting container should not be healthy");
    }

    #[test]
    fn test_container_status_anomaly_unhealthy_status() {
        let c = make_container("axonix", "running", "Up 5 min (unhealthy)");
        assert!(c.anomaly, "status containing '(unhealthy)' should set anomaly=true");
        assert!(c.healthy, "state is 'running' so healthy should still be true");
    }

    #[test]
    fn test_container_status_no_anomaly() {
        let c = make_container("axonix", "running", "Up 2 days");
        assert!(!c.anomaly, "normal running container should have anomaly=false");
        assert!(c.healthy, "running container should be healthy");
    }

    #[test]
    fn test_docker_health_format_shows_warning_for_anomaly() {
        let dh = DockerHealth {
            containers: vec![
                make_container("axonix", "running", "Up 5 min (unhealthy)"),
                make_container("axonix-listener", "running", "Up 2 days"),
            ],
            error: None,
        };
        let fmt = dh.format();
        assert!(fmt.contains('⚠'), "format should show ⚠ for anomaly container: {fmt}");
        assert!(fmt.contains('✓'), "format should show ✓ for healthy container: {fmt}");
    }

    #[test]
    fn test_parse_docker_containers_empty() {
        let result = parse_docker_containers("null");
        assert!(result.containers.is_empty());
        assert!(result.error.is_none());
    }
}
