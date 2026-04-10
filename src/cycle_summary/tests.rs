#[cfg(test)]
mod tests {
    use crate::cycle_summary::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("axonix_cycle_test_{name}.json"))
    }

    fn cleanup(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }

    // ── CycleSummaryData ─────────────────────────────────────────────────────

    #[test]
    fn test_new_summary_data_is_empty() {
        let data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        assert_eq!(data.session, "Day 7, Session 5");
        assert_eq!(data.date, "2026-03-20");
        assert!(data.completed.is_empty());
        assert!(data.changed_files.is_empty());
        assert!(data.pending.is_empty());
        assert!(data.learnings.is_empty());
        assert!(data.test_count.is_none());
    }

    #[test]
    fn test_format_for_prompt_returns_none_when_empty() {
        let data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        assert!(data.format_for_prompt().is_none(),
            "Empty summary should return None");
    }

    #[test]
    fn test_format_for_prompt_contains_session_and_date() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("Fixed Issue #38".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("Day 7, Session 5"), "Should contain session name");
        assert!(formatted.contains("2026-03-20"), "Should contain date");
    }

    #[test]
    fn test_format_for_prompt_contains_completed_items() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("Implemented cycle_summary module".to_string());
        data.completed.push("Fixed Issue #38".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("Implemented cycle_summary module"));
        assert!(formatted.contains("Fixed Issue #38"));
    }

    #[test]
    fn test_format_for_prompt_contains_pending_items() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.pending.push("G-031: morning brief".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("G-031: morning brief"));
        assert!(formatted.contains("Pending"));
    }

    #[test]
    fn test_format_for_prompt_contains_changed_files() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.changed_files.push("src/cycle_summary.rs".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("src/cycle_summary.rs"));
    }

    #[test]
    fn test_format_for_prompt_contains_learnings() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.learnings.push("compact summary keeps context bounded".to_string());
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("compact summary keeps context bounded"));
    }

    #[test]
    fn test_format_for_prompt_shows_test_count() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("wrote tests".to_string());
        data.test_count = Some(506);
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("506"), "Should show test count");
    }

    #[test]
    fn test_format_for_prompt_caps_completed_at_10() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        for i in 0..15 {
            data.completed.push(format!("item {i}"));
        }
        let formatted = data.format_for_prompt().unwrap();
        // Only first 10 should appear (items 0-9)
        assert!(formatted.contains("item 9"), "Item 9 should appear");
        assert!(!formatted.contains("item 10"), "Item 10 should not appear (capped at 10)");
    }

    #[test]
    fn test_format_for_prompt_caps_changed_files_at_15() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        for i in 0..20 {
            data.changed_files.push(format!("src/file_{i}.rs"));
        }
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("src/file_14.rs"), "File 14 should appear");
        assert!(!formatted.contains("src/file_15.rs"), "File 15 should not appear (capped at 15)");
    }

    #[test]
    fn test_format_for_prompt_caps_learnings_at_5() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        for i in 0..10 {
            data.learnings.push(format!("learning {i}"));
        }
        let formatted = data.format_for_prompt().unwrap();
        assert!(formatted.contains("learning 4"), "Learning 4 should appear");
        assert!(!formatted.contains("learning 5"), "Learning 5 should not appear (capped at 5)");
    }

    #[test]
    fn test_cycle_summary_data_serializes_to_json() {
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("thing done".to_string());
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("Day 7, Session 5"));
        assert!(json.contains("thing done"));
    }

    #[test]
    fn test_cycle_summary_data_deserializes_from_json() {
        let json = r#"{
            "session": "Day 7, Session 5",
            "date": "2026-03-20",
            "completed": ["Fixed Issue #38"],
            "changed_files": ["src/main.rs"],
            "pending": ["G-031"],
            "learnings": ["keep it compact"]
        }"#;
        let data: CycleSummaryData = serde_json::from_str(json).unwrap();
        assert_eq!(data.session, "Day 7, Session 5");
        assert_eq!(data.completed, vec!["Fixed Issue #38"]);
        assert_eq!(data.changed_files, vec!["src/main.rs"]);
        assert_eq!(data.pending, vec!["G-031"]);
        assert_eq!(data.learnings, vec!["keep it compact"]);
    }

    #[test]
    fn test_cycle_summary_data_deserializes_with_defaults() {
        // Minimal JSON — missing optional fields should default to empty vecs
        let json = r#"{"session": "Day 7", "date": "2026-03-20"}"#;
        let data: CycleSummaryData = serde_json::from_str(json).unwrap();
        assert!(data.completed.is_empty());
        assert!(data.pending.is_empty());
        assert!(data.test_count.is_none());
    }

    // ── CycleSummary (file manager) ──────────────────────────────────────────

    #[test]
    fn test_cycle_summary_new_has_no_data() {
        let path = tmp_path("new_no_data");
        cleanup(&path);
        let cs = CycleSummary::new(&path);
        assert!(cs.data.is_none());
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_load_missing_file_returns_none_data() {
        let path = tmp_path("load_missing");
        cleanup(&path); // ensure it doesn't exist
        let cs = CycleSummary::load(&path);
        assert!(cs.data.is_none(), "Missing file should produce None data");
    }

    #[test]
    fn test_cycle_summary_load_invalid_json_returns_none_data() {
        let path = tmp_path("load_invalid");
        fs::write(&path, "not valid json {{{{").unwrap();
        let cs = CycleSummary::load(&path);
        assert!(cs.data.is_none(), "Invalid JSON should produce None data");
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_save_and_load_roundtrip() {
        let path = tmp_path("roundtrip");
        cleanup(&path);

        let mut cs = CycleSummary::new(&path);
        let mut data = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        data.completed.push("roundtrip test".to_string());
        data.test_count = Some(506);
        cs.set(data);
        cs.save().unwrap();

        let loaded = CycleSummary::load(&path);
        let d = loaded.data.unwrap();
        assert_eq!(d.session, "Day 7, Session 5");
        assert_eq!(d.completed, vec!["roundtrip test"]);
        assert_eq!(d.test_count, Some(506));
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_save_creates_parent_dir() {
        let path = tmp_path("parent_dir_test");
        let subpath = path.join("subdir").join("cycle_summary.json");
        let _ = fs::remove_dir_all(&path);

        let mut cs = CycleSummary::new(&subpath);
        let data = CycleSummaryData::new("Day 7", "2026-03-20");
        cs.set(data);
        // add something so format_for_prompt would return Some (not strictly needed here)
        cs.add_completed("test");
        cs.save().unwrap();
        assert!(subpath.exists(), "File should be created with parent dirs");
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_cycle_summary_save_without_data_returns_error() {
        let path = tmp_path("save_no_data");
        cleanup(&path);
        let cs = CycleSummary::new(&path);
        let result = cs.save();
        assert!(result.is_err(), "Save without data should return error");
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_set_session_initializes_data() {
        let path = tmp_path("set_session");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        assert!(cs.data.is_some());
        assert_eq!(cs.data.as_ref().unwrap().session, "Day 7, Session 5");
    }

    #[test]
    fn test_cycle_summary_set_session_updates_existing_data() {
        let path = tmp_path("update_session");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 4", "2026-03-20");
        cs.set_session("Day 7, Session 5", "2026-03-20"); // update
        assert_eq!(cs.data.as_ref().unwrap().session, "Day 7, Session 5");
    }

    #[test]
    fn test_cycle_summary_add_methods_require_data() {
        let path = tmp_path("add_methods");
        let mut cs = CycleSummary::new(&path);
        // These are no-ops when data is None — should not panic
        cs.add_completed("test");
        cs.add_changed_file("src/main.rs");
        cs.add_pending("G-031");
        cs.add_learning("learned something");
        cs.set_test_count(506);
        assert!(cs.data.is_none(), "Data should still be None");
    }

    #[test]
    fn test_cycle_summary_add_completed() {
        let path = tmp_path("add_completed");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_completed("Fixed Issue #38");
        assert_eq!(cs.data.as_ref().unwrap().completed, vec!["Fixed Issue #38"]);
    }

    #[test]
    fn test_cycle_summary_add_changed_file() {
        let path = tmp_path("add_file");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_changed_file("src/cycle_summary.rs");
        assert_eq!(cs.data.as_ref().unwrap().changed_files, vec!["src/cycle_summary.rs"]);
    }

    #[test]
    fn test_cycle_summary_add_pending() {
        let path = tmp_path("add_pending");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_pending("G-031: morning brief on schedule");
        assert_eq!(cs.data.as_ref().unwrap().pending, vec!["G-031: morning brief on schedule"]);
    }

    #[test]
    fn test_cycle_summary_add_learning() {
        let path = tmp_path("add_learning");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_learning("context bounded by summary");
        assert_eq!(cs.data.as_ref().unwrap().learnings, vec!["context bounded by summary"]);
    }

    #[test]
    fn test_cycle_summary_set_test_count() {
        let path = tmp_path("set_count");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.set_test_count(506);
        assert_eq!(cs.data.as_ref().unwrap().test_count, Some(506));
    }

    #[test]
    fn test_format_for_system_prompt_none_when_no_data() {
        let path = tmp_path("fmt_no_data");
        let cs = CycleSummary::new(&path);
        assert!(cs.format_for_system_prompt().is_none());
    }

    #[test]
    fn test_format_for_system_prompt_none_when_empty_data() {
        let path = tmp_path("fmt_empty");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7", "2026-03-20");
        // No content added — should return None
        assert!(cs.format_for_system_prompt().is_none());
    }

    #[test]
    fn test_format_for_system_prompt_some_when_has_content() {
        let path = tmp_path("fmt_with_content");
        let mut cs = CycleSummary::new(&path);
        cs.set_session("Day 7, Session 5", "2026-03-20");
        cs.add_completed("Fixed Issue #38");
        let result = cs.format_for_system_prompt();
        assert!(result.is_some());
        assert!(result.unwrap().contains("Last Session"));
    }

    #[test]
    fn test_cycle_summary_load_valid_json_parses() {
        let path = tmp_path("load_valid");
        let json = r#"{
            "session": "Day 7, Session 4",
            "date": "2026-03-20",
            "completed": ["Hit 500 tests"],
            "pending": ["G-031"]
        }"#;
        fs::write(&path, json).unwrap();
        let cs = CycleSummary::load(&path);
        let data = cs.data.unwrap();
        assert_eq!(data.session, "Day 7, Session 4");
        assert_eq!(data.completed, vec!["Hit 500 tests"]);
        cleanup(&path);
    }

    #[test]
    fn test_cycle_summary_overwrites_on_save() {
        let path = tmp_path("overwrite");
        cleanup(&path);

        // First save
        let mut cs1 = CycleSummary::new(&path);
        let mut d1 = CycleSummaryData::new("Day 7, Session 4", "2026-03-20");
        d1.completed.push("first save".to_string());
        cs1.set(d1);
        cs1.save().unwrap();

        // Second save (overwrites)
        let mut cs2 = CycleSummary::new(&path);
        let mut d2 = CycleSummaryData::new("Day 7, Session 5", "2026-03-20");
        d2.completed.push("second save".to_string());
        cs2.set(d2);
        cs2.save().unwrap();

        let loaded = CycleSummary::load(&path);
        let data = loaded.data.unwrap();
        assert_eq!(data.session, "Day 7, Session 5", "Should have second session");
        assert_eq!(data.completed, vec!["second save"]);
        cleanup(&path);
    }

    #[test]
    fn test_default_path_uses_axonix_dir() {
        let cs = CycleSummary::default_path();
        let path_str = cs.path.to_string_lossy();
        assert!(path_str.contains(".axonix"), "Should be in .axonix directory");
        assert!(path_str.contains("cycle_summary.json"), "Should be cycle_summary.json");
    }

    // ── from_real_data / write_default (G-035) ──────────────────────────────

    /// Verify from_real_data sets the provided session label (G-035).
    #[test]
    fn test_from_real_data_has_session_label() {
        let data = CycleSummary::from_real_data("test label");
        assert_eq!(data.session, "test label", "session should match provided label");
    }

    /// Verify from_real_data produces a non-empty date (G-035).
    #[test]
    fn test_from_real_data_date_not_empty() {
        let data = CycleSummary::from_real_data("test session");
        assert!(!data.date.is_empty(), "date should not be empty");
        // Should be either a real date (YYYY-MM-DD) or "unknown"
        let looks_like_date = data.date.len() >= 7;
        assert!(looks_like_date, "date should be at least 7 chars, got: '{}'", data.date);
    }

    /// Verify write_default roundtrip: write then read back session field (G-035).
    #[test]
    fn test_write_summary_roundtrip() {
        let data = CycleSummary::from_real_data("roundtrip session");
        let tmp = std::env::temp_dir().join("axonix_write_default_test.json");
        // Write manually to a temp path to avoid polluting .axonix
        let mut cs = CycleSummary::new(&tmp);
        cs.set(data.clone());
        cs.save().unwrap();
        let loaded = CycleSummary::load(&tmp);
        let loaded_data = loaded.data.unwrap();
        assert_eq!(loaded_data.session, "roundtrip session");
        let _ = std::fs::remove_file(&tmp);
    }

    /// Verify from_real_data with empty label is preserved (G-035).
    #[test]
    fn test_from_real_data_empty_label_preserved() {
        let data = CycleSummary::from_real_data("");
        assert_eq!(data.session, "", "empty label should be stored as-is");
    }

    /// Verify from_real_data completed list contains strings (may be empty if no git) (G-035).
    #[test]
    fn test_from_real_data_completed_is_vec_of_strings() {
        let data = CycleSummary::from_real_data("label");
        // All completed items must be non-empty strings
        for item in &data.completed {
            assert!(!item.is_empty(), "completed items should be non-empty strings");
        }
    }
}
