use super::*;
use tempfile::tempdir;

fn open_tmp() -> AxonixDb {
    let dir = tempdir().unwrap();
    let path = dir.into_path().join("test.db");
    AxonixDb::open(&path).unwrap()
}

// ── KV ───────────────────────────────────────────────────────────────────

#[test]
fn test_kv_set_and_get() {
    let db = open_tmp();
    db.kv_set("hello", "world").unwrap();
    assert_eq!(db.kv_get("hello").unwrap(), Some("world".to_string()));
}

#[test]
fn test_kv_overwrite() {
    let db = open_tmp();
    db.kv_set("key", "first").unwrap();
    db.kv_set("key", "second").unwrap();
    assert_eq!(db.kv_get("key").unwrap(), Some("second".to_string()));
}

#[test]
fn test_kv_get_missing() {
    let db = open_tmp();
    assert_eq!(db.kv_get("nonexistent").unwrap(), None);
}

#[test]
fn test_kv_delete() {
    let db = open_tmp();
    db.kv_set("delete_me", "value").unwrap();
    assert!(db.kv_delete("delete_me").unwrap());
    assert_eq!(db.kv_get("delete_me").unwrap(), None);
}

#[test]
fn test_kv_delete_missing() {
    let db = open_tmp();
    assert!(!db.kv_delete("ghost_key").unwrap());
}

#[test]
fn test_kv_list() {
    let db = open_tmp();
    db.kv_set("alpha", "1").unwrap();
    db.kv_set("beta", "2").unwrap();
    db.kv_set("gamma", "3").unwrap();
    let list = db.kv_list().unwrap();
    assert_eq!(list.len(), 3);
    // Ordered by key
    assert_eq!(list[0].0, "alpha");
    assert_eq!(list[1].0, "beta");
    assert_eq!(list[2].0, "gamma");
}

// ── Sessions ─────────────────────────────────────────────────────────────

#[test]
fn test_session_insert_and_query() {
    let db = open_tmp();
    let id = db
        .session_insert(75, "S1", "2025-01-01", Some("100k"), Some(42), Some(0), Some("all good"))
        .unwrap();
    assert!(id > 0);
    let rows = db.sessions_recent(10).unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.day, 75);
    assert_eq!(row.session, "S1");
    assert_eq!(row.tokens, Some("100k".to_string()));
    assert_eq!(row.tests, Some(42));
    assert_eq!(row.failed, Some(0));
    assert_eq!(row.notes, Some("all good".to_string()));
}

#[test]
fn test_sessions_recent_limit() {
    let db = open_tmp();
    for i in 1..=5u64 {
        db.session_insert(i as i64, "S1", "2025-01-01", None, None, None, None)
            .unwrap();
    }
    let rows = db.sessions_recent(3).unwrap();
    assert_eq!(rows.len(), 3);
}

// ── Goals ────────────────────────────────────────────────────────────────

#[test]
fn test_goal_upsert_and_get() {
    let db = open_tmp();
    db.goal_upsert("G-075", "SQLite memory", "active").unwrap();
    let goal = db.goal_get("G-075").unwrap().expect("goal should exist");
    assert_eq!(goal.id, "G-075");
    assert_eq!(goal.title, "SQLite memory");
    assert_eq!(goal.status, "active");
    assert!(goal.completed_at.is_none());
}

#[test]
fn test_goal_complete() {
    let db = open_tmp();
    db.goal_upsert("G-010", "Some goal", "active").unwrap();
    let updated = db.goal_complete("G-010").unwrap();
    assert!(updated);
    let goal = db.goal_get("G-010").unwrap().expect("goal should exist");
    assert_eq!(goal.status, "done");
    assert!(goal.completed_at.is_some());
}

#[test]
fn test_goal_complete_nonexistent() {
    let db = open_tmp();
    let updated = db.goal_complete("G-999").unwrap();
    assert!(!updated);
}

#[test]
fn test_goals_by_status() {
    let db = open_tmp();
    db.goal_upsert("G-001", "Active goal 1", "active").unwrap();
    db.goal_upsert("G-002", "Active goal 2", "active").unwrap();
    db.goal_upsert("G-003", "Backlog goal",  "backlog").unwrap();
    db.goal_upsert("G-004", "Done goal",     "done").unwrap();

    let active = db.goals_by_status("active").unwrap();
    assert_eq!(active.len(), 2);

    let backlog = db.goals_by_status("backlog").unwrap();
    assert_eq!(backlog.len(), 1);
    assert_eq!(backlog[0].id, "G-003");

    let done = db.goals_by_status("done").unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].id, "G-004");
}

// ── Observations ─────────────────────────────────────────────────────────

#[test]
fn test_observation_store_and_list() {
    let db = open_tmp();
    db.observation_store("obs:1", "rust borrow checker error", "rust,error").unwrap();
    db.observation_store("obs:2", "repl command dispatch", "repl,slash-command").unwrap();
    let rows = db.observations_list(10).unwrap();
    assert_eq!(rows.len(), 2, "should have 2 observations");
}

#[test]
fn test_observation_store_upsert() {
    let db = open_tmp();
    db.observation_store("obs:dup", "original text", "tag1").unwrap();
    db.observation_store("obs:dup", "updated text", "tag1,tag2").unwrap();
    let rows = db.observations_list(10).unwrap();
    assert_eq!(rows.len(), 1, "upsert should result in only 1 row");
    assert_eq!(rows[0].text, "updated text", "text should be updated");
    assert_eq!(rows[0].tags, "tag1,tag2", "tags should be updated");
}

#[test]
fn test_search_memory_finds_by_keyword() {
    let db = open_tmp();
    db.observation_store("obs:rust", "rust borrow checker error", "rust,error").unwrap();
    let results = db.search_memory("rust", 10).unwrap();
    assert!(!results.is_empty(), "search for 'rust' should find the observation");
    assert_eq!(results[0].key, "obs:rust");
    assert!(results[0].score > 0.0);
}

#[test]
fn test_search_memory_tags_score_higher() {
    let db = open_tmp();
    // "repl" in tags → score 2.0
    db.observation_store("obs:tag", "something about commands", "repl,commands").unwrap();
    // "repl" only in text → score 1.0
    db.observation_store("obs:text", "the repl handles user input", "").unwrap();
    let results = db.search_memory("repl", 10).unwrap();
    assert_eq!(results.len(), 2, "both observations should match");
    assert_eq!(results[0].key, "obs:tag",
        "tag match should rank higher than text match");
    assert!(results[0].score > results[1].score,
        "tag score ({}) should beat text score ({})", results[0].score, results[1].score);
}

#[test]
fn test_search_memory_no_results() {
    let db = open_tmp();
    db.observation_store("obs:a", "something about rust", "rust").unwrap();
    let results = db.search_memory("xyzzy nothing", 10).unwrap();
    assert!(results.is_empty(), "search for nonsense words should return empty");
}

#[test]
fn test_search_memory_limit() {
    let db = open_tmp();
    for i in 0..5 {
        db.observation_store(
            &format!("obs:{i}"),
            &format!("rust error number {i}"),
            "rust,error",
        ).unwrap();
    }
    let results = db.search_memory("rust", 3).unwrap();
    assert_eq!(results.len(), 3, "limit should cap results at 3");
}

#[test]
fn test_search_memory_filters_stop_words() {
    let db = open_tmp();
    db.observation_store("obs:sw", "database error occurred", "error,database").unwrap();
    // "the" is a stop word; "error" is meaningful
    let results = db.search_memory("the error", 10).unwrap();
    assert!(!results.is_empty(), "should match on 'error' even though 'the' is filtered");
    assert_eq!(results[0].key, "obs:sw");
}

// ── seed_from_journal ────────────────────────────────────────────────────

#[test]
fn test_seed_from_journal_parses_entries() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let db = AxonixDb::open(&db_path).unwrap();

    // Write a mock journal
    let journal = dir.path().join("JOURNAL.md");
    std::fs::write(
        &journal,
        "# Journal\n\n## Day 16, S7 — G-088: memory search\n\nBuilt TF-IDF search over SQLite observations table.\n\n## Day 16, S6 — G-087: archive fix\n\nFixed /archive-journal slash-command dispatch.\n",
    ).unwrap();

    let n = db.seed_from_journal(&journal).unwrap();
    assert_eq!(n, 2);

    let results = db.search_memory("memory search", 10).unwrap();
    assert!(!results.is_empty(), "should find memory search entry");
}

#[test]
fn test_seed_from_journal_upserts_idempotent() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let db = AxonixDb::open(&db_path).unwrap();

    let journal = dir.path().join("JOURNAL.md");
    std::fs::write(
        &journal,
        "# Journal\n\n## Day 1, S1 — test entry\n\nSome content.\n",
    ).unwrap();

    let n1 = db.seed_from_journal(&journal).unwrap();
    let n2 = db.seed_from_journal(&journal).unwrap();
    assert_eq!(n1, 1);
    assert_eq!(n2, 1); // idempotent, still upserts same entry

    // Should still only have 1 observation
    let list = db.observations_list(10).unwrap();
    assert_eq!(list.len(), 1);
}

#[test]
fn test_seed_from_journal_missing_file() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let db = AxonixDb::open(&db_path).unwrap();

    let missing = dir.path().join("NONEXISTENT.md");
    let result = db.seed_from_journal(&missing);
    assert!(result.is_err() || result.unwrap() == 0);
}

// ── Hot memories ──────────────────────────────────────────────────────────

#[test]
fn test_hot_memory_insert_and_list() {
    let db = open_tmp();
    let id = db.hot_memory_insert(
        "Operator prefers concise summaries",
        "concise summaries",
        "[]",
        "[\"style\",\"preferences\"]",
        0.9,
    ).unwrap();
    assert!(id > 0);

    let rows = db.hot_memories_list(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].content, "Operator prefers concise summaries");
    assert_eq!(rows[0].importance, 0.9);
    assert_eq!(rows[0].access_count, 0);
}

#[test]
fn test_hot_memory_search_finds_by_keyword() {
    let db = open_tmp();
    db.hot_memory_insert("rust borrow checker patterns", "rust borrow", "[]", "[\"rust\"]", 0.7).unwrap();
    db.hot_memory_insert("SQLite migration scripts", "sqlite migration", "[]", "[\"database\"]", 0.6).unwrap();

    let results = db.hot_memory_search("rust", 10).unwrap();
    assert!(!results.is_empty(), "should find rust memory");
    assert_eq!(results[0].summary, "rust borrow");
}

#[test]
fn test_hot_memory_touch_increments_access_count() {
    let db = open_tmp();
    let id = db.hot_memory_insert("some fact", "fact", "[]", "[]", 0.6).unwrap();
    db.hot_memory_touch(id).unwrap();
    db.hot_memory_touch(id).unwrap();

    let rows = db.hot_memories_list(10).unwrap();
    assert_eq!(rows[0].access_count, 2, "access_count should be 2 after two touches");
}

#[test]
fn test_hot_memory_delete() {
    let db = open_tmp();
    let id = db.hot_memory_insert("to be deleted", "delete me", "[]", "[]", 0.5).unwrap();
    assert!(db.hot_memory_delete(id).unwrap());
    let rows = db.hot_memories_list(10).unwrap();
    assert!(rows.is_empty(), "hot memory should be deleted");
}

#[test]
fn test_hot_memory_search_no_results() {
    let db = open_tmp();
    db.hot_memory_insert("rust memory", "rust", "[]", "[\"rust\"]", 0.7).unwrap();
    let results = db.hot_memory_search("xyzzy_nothing_here", 10).unwrap();
    assert!(results.is_empty());
}

// ── Cold memories ─────────────────────────────────────────────────────────

#[test]
fn test_cold_memory_insert_and_list() {
    let db = open_tmp();
    let id = db.cold_memory_insert(
        "Axonix consistently uses async patterns across all modules",
        "[\"architecture\",\"async\"]",
        0.8,
    ).unwrap();
    assert!(id > 0);

    let rows = db.cold_memories_list(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].content, "Axonix consistently uses async patterns across all modules");
    assert_eq!(rows[0].reinforcement_count, 0);
}

#[test]
fn test_cold_memory_search_finds_by_keyword() {
    let db = open_tmp();
    db.cold_memory_insert("async patterns dominate the codebase", "[\"async\",\"architecture\"]", 0.8).unwrap();
    db.cold_memory_insert("SQLite is used for persistent storage", "[\"database\",\"sqlite\"]", 0.7).unwrap();

    let results = db.cold_memory_search("async", 10).unwrap();
    assert!(!results.is_empty());
    assert!(results[0].content.contains("async"));
}

#[test]
fn test_cold_memory_reinforce() {
    let db = open_tmp();
    let id = db.cold_memory_insert("a synthesized pattern", "[\"pattern\"]", 0.7).unwrap();
    db.cold_memory_reinforce(id).unwrap();

    let rows = db.cold_memories_list(10).unwrap();
    assert_eq!(rows[0].reinforcement_count, 1, "reinforcement_count should be 1");
}

#[test]
fn test_cold_memory_delete() {
    let db = open_tmp();
    let id = db.cold_memory_insert("temporary cold memory", "[]", 0.5).unwrap();
    assert!(db.cold_memory_delete(id).unwrap());
    let rows = db.cold_memories_list(10).unwrap();
    assert!(rows.is_empty());
}

// ── Contradictions ────────────────────────────────────────────────────────

#[test]
fn test_contradiction_insert_and_open() {
    let db = open_tmp();
    let cold_id = db.cold_memory_insert("old belief about X", "[\"topic\"]", 0.7).unwrap();
    db.contradiction_insert(cold_id, "new conflicting belief about X").unwrap();

    let open = db.contradictions_open().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].cold_memory_id, cold_id);
    assert_eq!(open[0].new_memory, "new conflicting belief about X");
    assert!(!open[0].resolved);
}

#[test]
fn test_contradictions_open_empty_when_none() {
    let db = open_tmp();
    let open = db.contradictions_open().unwrap();
    assert!(open.is_empty());
}

// ── add_days_to_utc ───────────────────────────────────────────────────────

#[test]
fn test_add_days_to_utc_basic() {
    let ts = "2026-01-01T00:00:00Z";
    let result = super::helpers::add_days_to_utc(ts, 30);
    assert!(result.starts_with("2026-01-31"), "30 days from Jan 1 should be Jan 31: {result}");
}

#[test]
fn test_add_days_crosses_month() {
    let ts = "2026-01-20T12:00:00Z";
    let result = super::helpers::add_days_to_utc(ts, 30);
    // Jan 20 + 30 days = Feb 19
    assert!(result.starts_with("2026-02-19"), "should be Feb 19: {result}");
}

// ── Structured observations ───────────────────────────────────────────────

#[test]
fn test_sobs_insert_returns_id() {
    let db = open_tmp();
    let id = db.sobs_insert("learned something new", "learned", "", "", "", "").unwrap();
    assert!(id > 0, "insert should return a positive row id");
}

#[test]
fn test_sobs_insert_invalid_category_defaults_to_learned() {
    let db = open_tmp();
    let id = db.sobs_insert("some content", "garbage", "", "", "", "").unwrap();
    assert!(id > 0);
    let rows = db.sobs_list(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].category, "learned", "invalid category should default to 'learned'");
}

#[test]
fn test_sobs_list_returns_most_recent_first() {
    let db = open_tmp();
    // Insert 3 observations
    let id1 = db.sobs_insert("first observation", "learned", "", "", "", "").unwrap();
    let id2 = db.sobs_insert("second observation", "learned", "", "", "", "").unwrap();
    let id3 = db.sobs_insert("third observation", "learned", "", "", "", "").unwrap();
    assert!(id3 > id2 && id2 > id1);

    // List only 2 — should return the two most recent (ORDER BY created_at DESC, id DESC)
    let rows = db.sobs_list(2).unwrap();
    assert_eq!(rows.len(), 2, "limit=2 should return at most 2 rows");
    // When timestamps are equal, ordering falls back to id DESC, so id3 and id2 come first
    assert_eq!(rows[0].id, id3, "most recent by id should be first");
    assert_eq!(rows[1].id, id2, "second most recent by id should be second");
}

#[test]
fn test_sobs_search_finds_by_content_keyword() {
    let db = open_tmp();
    db.sobs_insert("rusqlite connection pool setup", "learned", "", "", "", "").unwrap();
    let results = db.sobs_search("connection", 10).unwrap();
    assert!(!results.is_empty(), "search for 'connection' should find the observation");
    assert!(results[0].content.contains("connection"));
}

#[test]
fn test_sobs_search_finds_by_tag() {
    let db = open_tmp();
    db.sobs_insert("setting up the bot", "learned", "", "", "", "telegram,bot").unwrap();
    let results = db.sobs_search("telegram", 10).unwrap();
    assert!(!results.is_empty(), "search for 'telegram' should match via tags");
    assert!(results[0].tags.contains("telegram"));
}

#[test]
fn test_sobs_search_no_match_returns_empty() {
    let db = open_tmp();
    db.sobs_insert("completely unrelated content", "learned", "", "", "", "unrelated").unwrap();
    let results = db.sobs_search("zzz_nomatch", 10).unwrap();
    assert!(results.is_empty(), "search for 'zzz_nomatch' should return empty");
}

#[test]
fn test_sobs_search_respects_limit() {
    let db = open_tmp();
    for i in 0..5 {
        db.sobs_insert(&format!("memory about connection {i}"), "learned", "", "", "", "").unwrap();
    }
    let results = db.sobs_search("connection", 2).unwrap();
    assert_eq!(results.len(), 2, "limit=2 should cap results at 2");
}

#[test]
fn test_sobs_category_field_preserved() {
    let db = open_tmp();
    db.sobs_insert("hit a wall on this feature", "blocked_by", "", "", "", "").unwrap();
    let rows = db.sobs_list(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].category, "blocked_by", "category should be preserved as stored");
}

#[test]
fn test_sobs_goal_id_and_session_preserved() {
    let db = open_tmp();
    db.sobs_insert("discovered dep", "dependency_discovered", "", "G-110", "Day 19 S1", "").unwrap();
    let rows = db.sobs_list(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].goal_id, "G-110", "goal_id should be preserved");
    assert_eq!(rows[0].session, "Day 19 S1", "session should be preserved");
}

#[test]
fn test_sobs_search_ranks_tag_match_higher() {
    let db = open_tmp();
    // content-only match: score 1.0
    db.sobs_insert("we use sqlite heavily", "learned", "", "", "", "database").unwrap();
    // tag match: score 2.0 (tag "sqlite" matches)
    db.sobs_insert("persistence layer redesign", "learned", "", "", "", "sqlite,storage").unwrap();
    let results = db.sobs_search("sqlite", 10).unwrap();
    assert_eq!(results.len(), 2, "both observations should match");
    // The tag-match result should come first (higher score)
    assert_eq!(
        results[0].tags, "sqlite,storage",
        "tag-match observation should rank first"
    );
}

// ── Embeddings ────────────────────────────────────────────────────────────

#[test]
fn test_embedding_store_and_retrieve() {
    let db = open_tmp();
    let vec = vec![1.0f32, 2.0, 3.0];
    db.embedding_store("obs:test", &vec).unwrap();
    let list = db.embeddings_list_by_key().unwrap();
    assert_eq!(list.len(), 1, "should have 1 embedding");
    assert_eq!(list[0].0, "obs:test");
    assert_eq!(list[0].1.len(), 3);
    assert!((list[0].1[0] - 1.0).abs() < 1e-6);
    assert!((list[0].1[1] - 2.0).abs() < 1e-6);
    assert!((list[0].1[2] - 3.0).abs() < 1e-6);
}

#[test]
fn test_embedding_store_upsert() {
    let db = open_tmp();
    db.embedding_store("obs:dup", &[1.0f32, 0.0]).unwrap();
    db.embedding_store("obs:dup", &[0.0f32, 1.0]).unwrap();
    let list = db.embeddings_list_by_key().unwrap();
    assert_eq!(list.len(), 1, "upsert should keep only 1 embedding row");
    assert!((list[0].1[0] - 0.0).abs() < 1e-6);
    assert!((list[0].1[1] - 1.0).abs() < 1e-6);
}

#[test]
fn test_semantic_search_memory_no_ollama_returns_empty() {
    // With no Ollama available, should return empty vec (graceful fallback)
    let db = open_tmp();
    // Point to a port that will refuse connections
    std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1");
    let result = db.semantic_search_memory("test query", 5);
    std::env::remove_var("OLLAMA_URL");
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_semantic_search_memory_empty_query_returns_empty() {
    let db = open_tmp();
    let result = db.semantic_search_memory("", 5).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_semantic_search_memory_zero_limit_returns_empty() {
    let db = open_tmp();
    let result = db.semantic_search_memory("query", 0).unwrap();
    assert!(result.is_empty());
}
