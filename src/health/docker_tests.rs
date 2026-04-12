//! Tests for the Docker health module.

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
