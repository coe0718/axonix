//! Tests for `PredictionStore` (in predictions::store).

#[cfg(test)]
mod tests {
    use crate::predictions::store::PredictionStore;
    use crate::predictions::helpers::{unix_to_ymd, today_str, extract_goal_ids};
    use crate::predictions::types::parse_days_from_delta;

    fn tmp_store() -> (PredictionStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        let store = PredictionStore::new(path);
        (store, dir)
    }

    // ── basic operations ────────────────────────────────────────────────────────

    #[test]
    fn test_predict_returns_incrementing_ids() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("test will pass");
        let id2 = store.predict("build will succeed");
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_predict_stores_text() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("adding 10 tests");
        let pred = store.get(id).unwrap();
        assert_eq!(pred.prediction, "adding 10 tests");
        assert!(!pred.is_resolved());
    }

    #[test]
    fn test_predict_records_date() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("some prediction");
        let pred = store.get(id).unwrap();
        assert_eq!(pred.created.len(), 10, "date should be YYYY-MM-DD");
        assert!(pred.created.starts_with("20"), "date should start with 20xx");
    }

    #[test]
    fn test_resolve_prediction() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("will add 10 tests");
        let result = store.resolve(id, "actually added 12 tests", Some("underestimated by 2"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "will add 10 tests");
        let pred = store.get(id).unwrap();
        assert!(pred.is_resolved());
        assert_eq!(pred.outcome.as_deref(), Some("actually added 12 tests"));
        assert_eq!(pred.delta.as_deref(), Some("underestimated by 2"));
        assert!(pred.resolved.is_some());
    }

    #[test]
    fn test_resolve_nonexistent() {
        let (mut store, _dir) = tmp_store();
        let result = store.resolve(999, "outcome", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_resolve_already_resolved() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("test");
        store.resolve(id, "outcome", None).unwrap();
        let result = store.resolve(id, "second outcome", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already resolved"));
    }

    #[test]
    fn test_resolve_without_delta() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("will succeed");
        store.resolve(id, "succeeded", None).unwrap();
        let pred = store.get(id).unwrap();
        assert!(pred.delta.is_none());
    }

    // ── listing ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_open_returns_unresolved() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("open prediction");
        let id2 = store.predict("resolved prediction");
        store.resolve(id2, "done", None).unwrap();
        let open = store.open();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].0, id1);
    }

    #[test]
    fn test_resolved_returns_resolved() {
        let (mut store, _dir) = tmp_store();
        store.predict("open");
        let id2 = store.predict("resolved");
        store.resolve(id2, "done", None).unwrap();
        let resolved = store.resolved();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, id2);
    }

    #[test]
    fn test_counts() {
        let (mut store, _dir) = tmp_store();
        assert_eq!(store.count(), 0);
        assert_eq!(store.open_count(), 0);
        assert_eq!(store.resolved_count(), 0);
        store.predict("one");
        store.predict("two");
        let id3 = store.predict("three");
        store.resolve(id3, "done", None).unwrap();
        assert_eq!(store.count(), 3);
        assert_eq!(store.open_count(), 2);
        assert_eq!(store.resolved_count(), 1);
    }

    // ── persistence ─────────────────────────────────────────────────────────────

    #[test]
    fn test_save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");

        // Create and save
        let mut store = PredictionStore::new(path.clone());
        let id1 = store.predict("first prediction");
        let id2 = store.predict("second prediction");
        store.resolve(id2, "it happened", Some("exactly as expected")).unwrap();
        store.save().unwrap();

        // Reload
        let store2 = PredictionStore::new(path);
        assert_eq!(store2.count(), 2);
        let p1 = store2.get(id1).unwrap();
        assert_eq!(p1.prediction, "first prediction");
        assert!(!p1.is_resolved());
        let p2 = store2.get(id2).unwrap();
        assert!(p2.is_resolved());
        assert_eq!(p2.outcome.as_deref(), Some("it happened"));
        assert_eq!(p2.delta.as_deref(), Some("exactly as expected"));
    }

    #[test]
    fn test_load_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        let store = PredictionStore::new(path);
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_load_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        std::fs::write(&path, b"{ invalid json }").unwrap();
        let store = PredictionStore::new(path);
        assert_eq!(store.count(), 0, "corrupt file should result in empty store");
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("predictions.json");
        let mut store = PredictionStore::new(path.clone());
        store.predict("test");
        assert!(store.save().is_ok());
        assert!(path.exists());
    }

    #[test]
    fn test_next_id_continues_after_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("predictions.json");
        let mut store = PredictionStore::new(path.clone());
        store.predict("first");
        store.predict("second");
        store.save().unwrap();

        let mut store2 = PredictionStore::new(path);
        let id3 = store2.predict("third");
        assert_eq!(id3, 3, "next_id should continue from where we left off");
    }

    // ── date ────────────────────────────────────────────────────────────────────

    #[test]
    fn test_unix_to_ymd_epoch() {
        let (y, m, d) = unix_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_unix_to_ymd_known_date() {
        // 2026-03-17T00:00:00Z = 1773705600
        let (y, m, d) = unix_to_ymd(1773705600);
        assert_eq!(y, 2026);
        assert_eq!(m, 3);
        assert_eq!(d, 17);
    }

    #[test]
    fn test_today_str_format() {
        let today = today_str();
        assert_eq!(today.len(), 10);
        assert!(today.contains('-'));
        assert!(today.starts_with("20"));
    }

    // ── get nonexistent ─────────────────────────────────────────────────────────

    #[test]
    fn test_get_nonexistent() {
        let (store, _dir) = tmp_store();
        assert!(store.get(999).is_none());
    }

    // ── empty store operations ──────────────────────────────────────────────────

    #[test]
    fn test_open_empty() {
        let (store, _dir) = tmp_store();
        assert!(store.open().is_empty());
    }

    #[test]
    fn test_resolved_empty() {
        let (store, _dir) = tmp_store();
        assert!(store.resolved().is_empty());
    }

    // ── calibration ──────────────────────────────────────────────────────────────

    #[test]
    fn test_calibration_zero_resolved() {
        let (store, _dir) = tmp_store();
        let score = store.calibration_score();
        assert_eq!(score.total_resolved, 0);
        assert_eq!(score.correct, 0);
        assert_eq!(score.hit_rate, 0.0);
        assert_eq!(score.avg_days_early, 0.0);
        assert!(score.direction_bias.is_empty(), "empty store should have empty bias");
    }

    #[test]
    fn test_calibration_all_correct() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("prediction one");
        let id2 = store.predict("prediction two");
        let id3 = store.predict("prediction three");
        store.resolve(id1, "TRUE", None).unwrap();
        store.resolve(id2, "TRUE — exactly right", None).unwrap();
        store.resolve(id3, "TRUE", None).unwrap();
        let score = store.calibration_score();
        assert_eq!(score.total_resolved, 3);
        assert_eq!(score.correct, 3);
        assert!((score.hit_rate - 1.0).abs() < 1e-9, "hit_rate should be 1.0");
    }

    #[test]
    fn test_calibration_mixed() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("will succeed");
        let id2 = store.predict("will also succeed");
        let id3 = store.predict("will fail");
        store.resolve(id1, "TRUE", None).unwrap();
        store.resolve(id2, "TRUE", None).unwrap();
        store.resolve(id3, "FALSE", None).unwrap();
        let score = store.calibration_score();
        assert_eq!(score.total_resolved, 3);
        assert_eq!(score.correct, 2);
        let expected = 2.0 / 3.0;
        assert!((score.hit_rate - expected).abs() < 1e-9,
            "hit_rate should be ~0.667, got {}", score.hit_rate);
        assert_eq!(score.direction_bias, "calibrated");
    }

    #[test]
    fn test_calibration_format_some() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("will pass");
        store.resolve(id1, "TRUE", Some("1 day(s) early")).unwrap();
        let result = store.format_calibration_for_system_prompt();
        assert!(result.is_some(), "should return Some when there are resolved predictions");
        let text = result.unwrap();
        assert!(text.contains("Prediction Calibration"), "should contain section header");
        assert!(text.contains("1/1"), "should show 1/1 correct");
        assert!(text.contains("100.0%"), "should show 100% hit rate");
    }

    #[test]
    fn test_calibration_format_none() {
        let (store, _dir) = tmp_store();
        let result = store.format_calibration_for_system_prompt();
        assert!(result.is_none(), "should return None when 0 resolved predictions");
    }

    #[test]
    fn test_calibration_avg_days_early_early() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("will finish early");
        let id2 = store.predict("will also finish early");
        store.resolve(id1, "TRUE", Some("3 day(s) early")).unwrap();
        store.resolve(id2, "TRUE", Some("1 days early")).unwrap();
        let score = store.calibration_score();
        assert!((score.avg_days_early - 2.0).abs() < 1e-9,
            "avg_days_early should be 2.0, got {}", score.avg_days_early);
    }

    #[test]
    fn test_calibration_avg_days_late() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("will be late");
        store.resolve(id1, "FALSE", Some("2 day(s) late")).unwrap();
        let score = store.calibration_score();
        assert!((score.avg_days_early - (-2.0)).abs() < 1e-9,
            "avg_days_early should be -2.0 for late, got {}", score.avg_days_early);
    }

    #[test]
    fn test_calibration_no_delta_skipped() {
        let (mut store, _dir) = tmp_store();
        let id1 = store.predict("no delta");
        store.resolve(id1, "TRUE", None).unwrap();
        let score = store.calibration_score();
        assert_eq!(score.avg_days_early, 0.0, "no delta → avg_days_early should be 0.0");
    }

    #[test]
    fn test_calibration_direction_optimistic() {
        let (mut store, _dir) = tmp_store();
        for _ in 0..4 {
            let id = store.predict("will succeed");
            store.resolve(id, "TRUE", Some("2 days early")).unwrap();
        }
        let id5 = store.predict("this one too");
        store.resolve(id5, "TRUE", Some("1 days early")).unwrap();
        let score = store.calibration_score();
        // hit_rate = 1.0 > 0.7, avg_days_early > 0.5 → optimistic
        assert_eq!(score.direction_bias, "optimistic");
    }

    #[test]
    fn test_calibration_direction_pessimistic() {
        let (mut store, _dir) = tmp_store();
        for _ in 0..4 {
            let id = store.predict("will not succeed");
            store.resolve(id, "FALSE", None).unwrap();
        }
        let score = store.calibration_score();
        // hit_rate = 0.0 < 0.4 → pessimistic
        assert_eq!(score.direction_bias, "pessimistic");
    }

    #[test]
    fn test_parse_days_from_delta_early() {
        assert_eq!(parse_days_from_delta("1 day(s) early"), Some(1.0));
        assert_eq!(parse_days_from_delta("3 days early"), Some(3.0));
        assert_eq!(parse_days_from_delta("10 day early"), Some(10.0));
    }

    #[test]
    fn test_parse_days_from_delta_late() {
        assert_eq!(parse_days_from_delta("2 day(s) late"), Some(-2.0));
        assert_eq!(parse_days_from_delta("5 days late"), Some(-5.0));
    }

    #[test]
    fn test_parse_days_from_delta_unparseable() {
        assert_eq!(parse_days_from_delta("no timing info"), None);
        assert_eq!(parse_days_from_delta(""), None);
        assert_eq!(parse_days_from_delta("something happened"), None);
    }

    // ── SQLite write-through ─────────────────────────────────────────────────

    /// Verify that predict() immediately writes through to SQLite.
    #[test]
    fn test_predict_writes_to_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("predictions.json");
        let db_path = dir.path().join("axonix.db");

        let mut store = PredictionStore::new_with_db(json_path, db_path.clone());
        store.predict("the build will succeed on first attempt");

        // Open the DB directly and verify the prediction landed there.
        let db = crate::db::AxonixDb::open(&db_path).expect("should open db");
        let rows = db.predictions_list().expect("predictions_list should work");
        assert_eq!(rows.len(), 1, "one prediction should be in SQLite");
        let (id, text, _created, outcome, _delta, _resolved) = &rows[0];
        assert_eq!(id, "1");
        assert_eq!(text, "the build will succeed on first attempt");
        assert!(outcome.is_none(), "new prediction should be unresolved");
    }

    /// Verify that resolve() updates the SQLite row with outcome/delta/resolved.
    #[test]
    fn test_resolve_updates_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("predictions.json");
        let db_path = dir.path().join("axonix.db");

        let mut store = PredictionStore::new_with_db(json_path, db_path.clone());
        let id = store.predict("tests will pass without changes");
        store.resolve(id, "TRUE — all 42 tests passed", Some("1 days early")).unwrap();

        let db = crate::db::AxonixDb::open(&db_path).expect("should open db");
        let rows = db.predictions_list().expect("predictions_list should work");
        assert_eq!(rows.len(), 1);
        let (_id, _text, _created, outcome, delta, resolved) = &rows[0];
        assert_eq!(outcome.as_deref(), Some("TRUE — all 42 tests passed"));
        assert_eq!(delta.as_deref(), Some("1 days early"));
        assert!(resolved.is_some(), "resolved date should be set");
    }

    /// Verify that load prefers SQLite data when non-empty, ignoring JSON.
    #[test]
    fn test_load_prefers_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("predictions.json");
        let db_path = dir.path().join("axonix.db");

        // Write a prediction directly to SQLite (no PredictionStore involved).
        let db = crate::db::AxonixDb::open(&db_path).expect("should open db");
        db.prediction_upsert("5", "written directly to db", "2025-01-01", None, None, None)
            .expect("upsert should succeed");
        drop(db);

        // Write different data to JSON.
        let json_content = r#"{"1":{"prediction":"from json only","created":"2024-12-01","outcome":null,"delta":null,"resolved":null}}"#;
        std::fs::write(&json_path, json_content).unwrap();

        // Load: SQLite has data so it should be preferred over JSON.
        let store = PredictionStore::new_with_db(json_path, db_path);
        assert_eq!(store.count(), 1, "should load from SQLite (1 row), not JSON (1 row different)");
        // The prediction from SQLite (id=5) should be present.
        let pred = store.get(5).expect("prediction #5 should be loaded from SQLite");
        assert_eq!(pred.prediction, "written directly to db");
        // The JSON-only prediction (id=1) should NOT be loaded.
        assert!(store.get(1).is_none(), "JSON-only prediction should not appear when SQLite is non-empty");
    }

    // ── extract_goal_ids ─────────────────────────────────────────────────────

    #[test]
    fn test_extract_goal_ids_single() {
        let ids = extract_goal_ids("By Day 23, G-120 will be complete.");
        assert_eq!(ids, vec!["G-120"]);
    }

    #[test]
    fn test_extract_goal_ids_multiple() {
        let ids = extract_goal_ids("G-113 and G-117 will both be implemented.");
        assert_eq!(ids, vec!["G-113", "G-117"]);
    }

    #[test]
    fn test_extract_goal_ids_none() {
        let ids = extract_goal_ids("No goal IDs here at all.");
        assert!(ids.is_empty());
    }

    #[test]
    fn test_extract_goal_ids_g_without_digits() {
        // "G-" not followed by digits should not produce an entry
        let ids = extract_goal_ids("G- something happened.");
        assert!(ids.is_empty());
    }

    // ── auto_resolve_from_goals ──────────────────────────────────────────────

    #[test]
    fn test_auto_resolve_from_goals_resolves_matching_goal() {
        let (mut store, _dir) = tmp_store();
        store.predict("By Day 23, G-120 will be complete.");

        let archive = "- [x] [G-120] Split telegram.rs into modules\n";
        let resolved = store.auto_resolve_from_goals(archive);

        assert_eq!(resolved.len(), 1, "should resolve the G-120 prediction");
        assert!(resolved[0].1.contains("G-120"), "resolved text should mention G-120");

        // Verify it's actually resolved in the store
        assert_eq!(store.open_count(), 0, "no open predictions should remain");
        let pred = store.get(1).unwrap();
        assert!(pred.is_resolved(), "prediction should be resolved");
        assert!(pred.outcome.as_deref().unwrap().contains("TRUE"), "outcome should be TRUE");
        assert!(pred.outcome.as_deref().unwrap().contains("G-120"), "outcome should mention goal");
    }

    #[test]
    fn test_auto_resolve_from_goals_no_match() {
        let (mut store, _dir) = tmp_store();
        store.predict("By Day 23, G-999 will be complete.");

        // Archive does not contain G-999
        let archive = "- [x] [G-120] Something else\n";
        let resolved = store.auto_resolve_from_goals(archive);

        assert!(resolved.is_empty(), "should not resolve when goal not in archive");
        assert_eq!(store.open_count(), 1, "prediction should remain open");
    }

    #[test]
    fn test_auto_resolve_from_goals_already_resolved_not_touched() {
        let (mut store, _dir) = tmp_store();
        let id = store.predict("By Day 23, G-120 will be complete.");
        // Resolve it manually first
        store.resolve(id, "manual resolution", None).unwrap();

        let archive = "- [x] [G-120] Split telegram.rs\n";
        let resolved = store.auto_resolve_from_goals(archive);

        // auto_resolve should find no open predictions to act on
        assert!(resolved.is_empty(), "already-resolved prediction should not be touched");
    }

    #[test]
    fn test_auto_resolve_from_goals_multiple_goals_first_match_wins() {
        let (mut store, _dir) = tmp_store();
        store.predict("G-113 and G-117 will both be implemented.");

        // Archive only has G-113
        let archive = "- [x] [G-113] Predict command implemented\n";
        let resolved = store.auto_resolve_from_goals(archive);

        assert_eq!(resolved.len(), 1, "should resolve when first mentioned goal is complete");
        assert_eq!(store.open_count(), 0, "prediction should be resolved");
    }
}
