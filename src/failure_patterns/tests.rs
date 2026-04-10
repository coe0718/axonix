#[cfg(test)]
mod tests {
    use crate::failure_patterns::*;

    // ── 1. test_new_store_is_empty ──────────────────────────────────────────────

    #[test]
    fn test_new_store_is_empty() {
        let store = FailurePatternStore::new("/tmp/fp_test_new.json");
        assert_eq!(store.total_count(), 0);
        assert!(store.events.is_empty());
    }

    // ── 2. test_log_failure_adds_event ──────────────────────────────────────────

    #[test]
    fn test_log_failure_adds_event() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_log1.json");
        store.log_failure(
            FailureType::FalseCompletion,
            "marked done without verifying",
            "Day 10, Session 3",
            "2026-03-23",
        );
        assert_eq!(store.total_count(), 1);
        assert_eq!(store.events[0].failure_type, FailureType::FalseCompletion);
        assert_eq!(store.events[0].session, "Day 10, Session 3");
    }

    // ── 3. test_log_multiple_failures ──────────────────────────────────────────

    #[test]
    fn test_log_multiple_failures() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_multi.json");
        store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
        store.log_failure(FailureType::InfraBlindness, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseClaim, "c", "S3", "2026-01-03");
        assert_eq!(store.total_count(), 3);
    }

    // ── 4. test_get_recent_failures_empty ──────────────────────────────────────

    #[test]
    fn test_get_recent_failures_empty() {
        let store = FailurePatternStore::new("/tmp/fp_test_empty_recent.json");
        let recent = store.get_recent_failures(5);
        assert!(recent.is_empty());
    }

    // ── 5. test_get_recent_failures_limit ──────────────────────────────────────

    #[test]
    fn test_get_recent_failures_limit() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_limit.json");
        for i in 0..5 {
            store.log_failure(
                FailureType::FalseCompletion,
                &format!("event {i}"),
                "S1",
                "2026-01-01",
            );
        }
        let recent = store.get_recent_failures(3);
        assert_eq!(recent.len(), 3);
    }

    // ── 6. test_get_recent_failures_order ──────────────────────────────────────

    #[test]
    fn test_get_recent_failures_order() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_order.json");
        store.log_failure(FailureType::FalseCompletion, "first", "S1", "2026-01-01");
        store.log_failure(FailureType::FalseCompletion, "second", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseCompletion, "third", "S3", "2026-01-03");
        let recent = store.get_recent_failures(3);
        // Most recent first
        assert_eq!(recent[0].description, "third");
        assert_eq!(recent[1].description, "second");
        assert_eq!(recent[2].description, "first");
    }

    // ── 7. test_failure_summary_empty ──────────────────────────────────────────

    #[test]
    fn test_failure_summary_empty() {
        let store = FailurePatternStore::new("/tmp/fp_test_summary_empty.json");
        let summary = store.failure_summary();
        assert!(
            summary.contains("no failures"),
            "empty store should say no failures: {summary}"
        );
    }

    // ── 8. test_failure_summary_nonempty ───────────────────────────────────────

    #[test]
    fn test_failure_summary_nonempty() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_summary_nonempty.json");
        store.log_failure(
            FailureType::RevertRequired,
            "had to revert a bad commit",
            "Day 9, Session 2",
            "2026-03-20",
        );
        let summary = store.failure_summary();
        assert!(
            summary.contains("RevertRequired"),
            "summary should mention failure type: {summary}"
        );
        assert!(
            summary.contains("1"),
            "summary should show count: {summary}"
        );
    }

    // ── 9. test_most_common_failure_type_empty ─────────────────────────────────

    #[test]
    fn test_most_common_failure_type_empty() {
        let store = FailurePatternStore::new("/tmp/fp_test_mcft_empty.json");
        assert!(store.most_common_failure_type().is_none());
    }

    // ── 10. test_most_common_failure_type_single ───────────────────────────────

    #[test]
    fn test_most_common_failure_type_single() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_mcft_single.json");
        store.log_failure(FailureType::MissedWrapUp, "forgot to update GOALS", "S1", "2026-01-01");
        let result = store.most_common_failure_type();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "MissedWrapUp");
    }

    // ── 11. test_most_common_failure_type_multiple ─────────────────────────────

    #[test]
    fn test_most_common_failure_type_multiple() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_mcft_multi.json");
        // FalseCompletion appears 3×, InfraBlindness 1×
        store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
        store.log_failure(FailureType::InfraBlindness, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseCompletion, "c", "S3", "2026-01-03");
        store.log_failure(FailureType::FalseCompletion, "d", "S4", "2026-01-04");
        let result = store.most_common_failure_type();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "FalseCompletion");
    }

    // ── 12. test_count_by_type ─────────────────────────────────────────────────

    #[test]
    fn test_count_by_type() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_cbt.json");
        store.log_failure(FailureType::GoalReopened, "a", "S1", "2026-01-01");
        store.log_failure(FailureType::GoalReopened, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseClaim, "c", "S3", "2026-01-03");
        assert_eq!(store.count_by_type(&FailureType::GoalReopened), 2);
        assert_eq!(store.count_by_type(&FailureType::FalseClaim), 1);
        assert_eq!(store.count_by_type(&FailureType::RevertRequired), 0);
    }

    // ── 13. test_total_count ───────────────────────────────────────────────────

    #[test]
    fn test_total_count() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_tc.json");
        assert_eq!(store.total_count(), 0);
        store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
        assert_eq!(store.total_count(), 1);
        store.log_failure(FailureType::MissedWrapUp, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::InfraBlindness, "c", "S3", "2026-01-03");
        assert_eq!(store.total_count(), 3);
    }

    // ── 14. test_failure_type_display ─────────────────────────────────────────

    #[test]
    fn test_failure_type_display() {
        assert_eq!(FailureType::FalseCompletion.label(), "FalseCompletion");
        assert_eq!(FailureType::InfraBlindness.label(), "InfraBlindness");
        assert_eq!(FailureType::FalseClaim.label(), "FalseClaim");
        assert_eq!(FailureType::RevertRequired.label(), "RevertRequired");
        assert_eq!(FailureType::GoalReopened.label(), "GoalReopened");
        assert_eq!(FailureType::MissedWrapUp.label(), "MissedWrapUp");
        assert_eq!(FailureType::Other("custom".to_string()).label(), "Other(custom)");
    }

    // ── 15. test_save_and_load_roundtrip ──────────────────────────────────────

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("failure_patterns.json");

        let mut store = FailurePatternStore::new(&path);
        store.log_failure(
            FailureType::FalseCompletion,
            "marked done without verifying",
            "Day 10, Session 3",
            "2026-03-23",
        );
        store.log_failure(
            FailureType::Other("custom type".to_string()),
            "something unusual happened",
            "Day 10, Session 4",
            "2026-03-24",
        );
        store.save().expect("save should succeed");

        let loaded = FailurePatternStore::load(&path);
        assert_eq!(loaded.total_count(), 2);
        assert_eq!(loaded.events[0].failure_type, FailureType::FalseCompletion);
        assert_eq!(loaded.events[0].description, "marked done without verifying");
        assert_eq!(loaded.events[0].session, "Day 10, Session 3");
        assert_eq!(loaded.events[0].date, "2026-03-23");
        assert_eq!(
            loaded.events[1].failure_type,
            FailureType::Other("custom type".to_string())
        );
    }

    // ── 16. test_load_nonexistent_path ────────────────────────────────────────

    #[test]
    fn test_load_nonexistent_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        let store = FailurePatternStore::load(&path);
        assert_eq!(store.total_count(), 0);
        assert!(store.events.is_empty());
    }

    // ── 17. test_default_path_string ──────────────────────────────────────────

    #[test]
    fn test_default_path_string() {
        // When the env var is not set, default path ends with failure_patterns.json
        // (We unset the env var to get deterministic results)
        std::env::remove_var("AXONIX_FAILURE_PATTERNS_PATH");
        let path = default_failure_patterns_path();
        assert!(
            path.to_string_lossy().ends_with("failure_patterns.json"),
            "default path should end with failure_patterns.json: {path:?}"
        );
    }

    // ── Bonus: test_other_variant_serde_roundtrip ──────────────────────────────

    #[test]
    fn test_other_variant_serde_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fp_other.json");
        let mut store = FailurePatternStore::new(&path);
        store.log_failure(
            FailureType::Other("unusual blocker".to_string()),
            "misc failure",
            "Day 5, Session 1",
            "2026-02-01",
        );
        store.save().unwrap();

        let loaded = FailurePatternStore::load(&path);
        assert_eq!(loaded.total_count(), 1);
        match &loaded.events[0].failure_type {
            FailureType::Other(s) => assert_eq!(s, "unusual blocker"),
            other => panic!("expected Other variant, got {other:?}"),
        }
    }

    // ── Bonus: test_get_recent_fewer_than_n ───────────────────────────────────

    #[test]
    fn test_get_recent_fewer_than_n() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_fewer.json");
        store.log_failure(FailureType::FalseCompletion, "only one", "S1", "2026-01-01");
        // Ask for 10 but only 1 exists
        let recent = store.get_recent_failures(10);
        assert_eq!(recent.len(), 1);
    }

    // ── Bonus: test_count_by_type_other ───────────────────────────────────────

    #[test]
    fn test_count_by_type_other() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_cbt_other.json");
        store.log_failure(FailureType::Other("network".to_string()), "a", "S1", "2026-01-01");
        store.log_failure(FailureType::Other("network".to_string()), "b", "S2", "2026-01-02");
        store.log_failure(FailureType::Other("disk".to_string()), "c", "S3", "2026-01-03");
        // "network" and "disk" are distinct Other values
        assert_eq!(store.count_by_type(&FailureType::Other("network".to_string())), 2);
        assert_eq!(store.count_by_type(&FailureType::Other("disk".to_string())), 1);
    }

    // ── G-118: types_newly_at_threshold tests ─────────────────────────────────

    #[test]
    fn test_types_newly_at_threshold_below_threshold() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_tnath_below.json");
        store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
        store.log_failure(FailureType::FalseCompletion, "b", "S2", "2026-01-02");
        // count is 2, threshold is 3 — should not breach
        let newly = store.types_newly_at_threshold(3);
        assert!(newly.is_empty(), "2 events below threshold-3 should return empty: {newly:?}");
    }

    #[test]
    fn test_types_newly_at_threshold_at_threshold() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_tnath_at.json");
        store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
        store.log_failure(FailureType::FalseCompletion, "b", "S2", "2026-01-02");
        store.log_failure(FailureType::FalseCompletion, "c", "S3", "2026-01-03");
        // count is 3, threshold is 3 — should breach once
        let newly = store.types_newly_at_threshold(3);
        assert_eq!(newly.len(), 1, "exactly one type should breach: {newly:?}");
        assert_eq!(newly[0], "FalseCompletion");
    }

    #[test]
    fn test_types_newly_at_threshold_only_once_per_session() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_tnath_once.json");
        for _ in 0..3 {
            store.log_failure(FailureType::InfraBlindness, "x", "S1", "2026-01-01");
        }
        let first = store.types_newly_at_threshold(3);
        assert_eq!(first.len(), 1, "first call should return one breached type");
        // Add a 4th event for the same type — still should not re-alert
        store.log_failure(FailureType::InfraBlindness, "y", "S2", "2026-01-02");
        let second = store.types_newly_at_threshold(3);
        assert!(second.is_empty(), "second call should not re-alert the same type: {second:?}");
    }

    #[test]
    fn test_types_newly_at_threshold_multiple_types() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_tnath_multi.json");
        // FalseCompletion × 3, MissedWrapUp × 3, FalseClaim × 2
        for _ in 0..3 {
            store.log_failure(FailureType::FalseCompletion, "a", "S1", "2026-01-01");
            store.log_failure(FailureType::MissedWrapUp, "b", "S2", "2026-01-02");
        }
        store.log_failure(FailureType::FalseClaim, "c", "S3", "2026-01-03");
        store.log_failure(FailureType::FalseClaim, "d", "S4", "2026-01-04");
        let newly = store.types_newly_at_threshold(3);
        // sorted deterministically
        assert_eq!(newly.len(), 2, "two types should breach threshold-3: {newly:?}");
        assert!(newly.contains(&"FalseCompletion".to_string()));
        assert!(newly.contains(&"MissedWrapUp".to_string()));
        assert!(!newly.contains(&"FalseClaim".to_string()), "FalseClaim count=2 should not breach");
    }

    #[test]
    fn test_types_newly_at_threshold_empty_store() {
        let mut store = FailurePatternStore::new("/tmp/fp_test_tnath_empty.json");
        let newly = store.types_newly_at_threshold(3);
        assert!(newly.is_empty(), "empty store returns no breached types");
    }
}
