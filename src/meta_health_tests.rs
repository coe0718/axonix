//! Tests for the meta_health module.

use super::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn make_temp_file(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{}", content).unwrap();
    f
}

#[test]
fn test_health_status_is_ok() {
    assert!(HealthStatus::Ok("good".to_string()).is_ok());
    assert!(!HealthStatus::Warn("warn".to_string()).is_ok());
    assert!(!HealthStatus::Missing("gone".to_string()).is_ok());
}

#[test]
fn test_health_status_emoji() {
    assert_eq!(HealthStatus::Ok("".to_string()).emoji(), "✓");
    assert_eq!(HealthStatus::Warn("".to_string()).emoji(), "⚠");
    assert_eq!(HealthStatus::Missing("".to_string()).emoji(), "✗");
}

#[test]
fn test_health_status_message() {
    assert_eq!(HealthStatus::Ok("all good".to_string()).message(), "all good");
    assert_eq!(HealthStatus::Warn("stale".to_string()).message(), "stale");
    assert_eq!(HealthStatus::Missing("gone".to_string()).message(), "gone");
}

#[test]
fn test_check_file_freshness_missing() {
    let status = check_file_freshness(Path::new("/nonexistent/path.json"), Duration::from_secs(3600), "test.json");
    assert!(matches!(status, HealthStatus::Missing(_)));
    assert!(status.message().contains("not found"));
}

#[test]
fn test_check_file_freshness_fresh_file() {
    // A file just created should be fresh
    let f = make_temp_file("{}");
    let status = check_file_freshness(f.path(), Duration::from_secs(3600), "test.json");
    assert!(matches!(status, HealthStatus::Ok(_)), "fresh file should be Ok: {:?}", status.message());
}

#[test]
fn test_check_metrics_stale_no_stale() {
    let f = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | notes |\n");
    let status = check_metrics_stale(f.path());
    assert!(matches!(status, HealthStatus::Ok(_)), "no stale rows: {:?}", status.message());
}

#[test]
fn test_check_metrics_stale_with_stale() {
    let f = make_temp_file("| 11 | S2 | 2026-03-24 | ~?k | 700 | 0 | ? | ? | ? | yes | in progress |\n");
    let status = check_metrics_stale(f.path());
    assert!(matches!(status, HealthStatus::Warn(_)), "stale row should Warn: {:?}", status.message());
    assert!(status.message().contains("1 row(s)"));
}

#[test]
fn test_check_metrics_stale_multiple_stale() {
    let content = "| 11 | S2 | 2026-03-24 | ~?k | 700 | 0 | ? | ? | ? | yes | x |\n\
                   | 11 | S1 | 2026-03-24 | ~?k | 672 | 0 | ? | ? | ? | yes | y |\n";
    let f = make_temp_file(content);
    let status = check_metrics_stale(f.path());
    assert!(matches!(status, HealthStatus::Warn(_)));
    assert!(status.message().contains("2 row(s)"));
}

#[test]
fn test_check_metrics_missing() {
    let status = check_metrics_stale(Path::new("/nonexistent/METRICS.md"));
    assert!(matches!(status, HealthStatus::Missing(_)));
}

#[test]
fn test_meta_health_check_all_ok() {
    let preds = make_temp_file(r#"{"1": {"prediction": "test", "created": "2026-03-24", "outcome": null, "delta": null, "resolved": null}}"#);
    let cycle = make_temp_file(r#"{"session": "Day 11 S1", "date": "2026-03-24", "completed": [], "pending": [], "changed_files": [], "test_count": 700}"#);
    let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
    let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
    assert!(check.all_ok(), "all fresh files should be ok: {:?}", check.issues());
}

#[test]
fn test_meta_health_check_missing_predictions() {
    let cycle = make_temp_file("{}");
    let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
    let check = MetaHealthCheck::run_with_paths(
        Path::new("/nonexistent/predictions.json"),
        cycle.path(),
        metrics.path(),
    );
    assert!(!check.all_ok());
    assert!(!check.predictions.is_ok());
    let issues = check.issues();
    assert!(!issues.is_empty());
    assert!(issues[0].contains("✗"));
}

#[test]
fn test_meta_health_check_stale_metrics() {
    let preds = make_temp_file("{}");
    let cycle = make_temp_file("{}");
    let metrics = make_temp_file("| 11 | S2 | 2026-03-24 | ~?k | 700 | 0 | ? | ? | ? | yes | in progress |\n");
    let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
    assert!(!check.metrics_clean.is_ok());
    let issues = check.issues();
    assert!(issues.iter().any(|i| i.contains("METRICS.md")));
}

#[test]
fn test_format_terminal_all_ok() {
    let preds = make_temp_file("{}");
    let cycle = make_temp_file("{}");
    let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
    let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
    let out = check.format_terminal();
    assert!(out.contains("✓"), "should show checkmark when ok: {out}");
}

#[test]
fn test_format_telegram_all_ok() {
    let preds = make_temp_file("{}");
    let cycle = make_temp_file("{}");
    let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
    let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
    let msg = check.format_telegram();
    assert!(msg.contains("ok"), "all ok telegram: {msg}");
}

#[test]
fn test_format_telegram_with_issues() {
    let check = MetaHealthCheck::run_with_paths(
        Path::new("/nonexistent/predictions.json"),
        Path::new("/nonexistent/cycle_summary.json"),
        Path::new("/nonexistent/METRICS.md"),
    );
    let msg = check.format_telegram();
    assert!(msg.contains("issues"), "should mention issues: {msg}");
}

#[test]
fn test_issues_empty_when_all_ok() {
    let preds = make_temp_file("{}");
    let cycle = make_temp_file("{}");
    let metrics = make_temp_file("| 11 | S1 | 2026-03-24 | ~22k | 700 | 0 | 1 | 3 | 12 | yes | ok |\n");
    let check = MetaHealthCheck::run_with_paths(preds.path(), cycle.path(), metrics.path());
    assert!(check.issues().is_empty(), "no issues when all ok");
}

#[test]
fn test_issues_lists_all_problems() {
    let check = MetaHealthCheck::run_with_paths(
        Path::new("/nonexistent/predictions.json"),
        Path::new("/nonexistent/cycle_summary.json"),
        Path::new("/nonexistent/METRICS.md"),
    );
    assert_eq!(check.issues().len(), 3, "should report 3 issues when all missing");
}
