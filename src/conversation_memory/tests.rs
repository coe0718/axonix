//! Tests for conversation_memory sub-modules.

use super::{ConversationMemory, ConversationTurn};

fn tmp_mem(dir: &tempfile::TempDir) -> ConversationMemory {
    let path = dir.path().join("conv.json");
    ConversationMemory::new(path)
}

// ── Construction ─────────────────────────────────────────────────────────

#[test]
fn test_new_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mem = tmp_mem(&dir);
    assert!(mem.turns.is_empty(), "new ConversationMemory should be empty");
    assert_eq!(mem.max_turns, 100);
}

// ── push ─────────────────────────────────────────────────────────────────

#[test]
fn test_push_adds_turns() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = tmp_mem(&dir);
    mem.push("user", "hello", "telegram");
    mem.push("assistant", "hi there", "telegram");
    assert_eq!(mem.turns.len(), 2);
    assert_eq!(mem.turns[0].role, "user");
    assert_eq!(mem.turns[0].text, "hello");
    assert_eq!(mem.turns[1].role, "assistant");
    assert_eq!(mem.turns[1].text, "hi there");
}

#[test]
fn test_push_sets_channel() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = tmp_mem(&dir);
    mem.push("user", "test", "repl");
    assert_eq!(mem.turns[0].channel, "repl");
}

#[test]
fn test_push_max_turns_trims_oldest() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = tmp_mem(&dir);
    mem.max_turns = 3;

    mem.push("user", "msg1", "telegram");
    mem.push("user", "msg2", "telegram");
    mem.push("user", "msg3", "telegram");
    // Now at capacity
    assert_eq!(mem.turns.len(), 3);
    // Push a 4th — should drop msg1
    mem.push("user", "msg4", "telegram");
    assert_eq!(mem.turns.len(), 3, "should stay at max_turns");
    assert_eq!(mem.turns[0].text, "msg2", "oldest should be dropped");
    assert_eq!(mem.turns[2].text, "msg4", "newest should be last");
}

// ── save / load ───────────────────────────────────────────────────────────

#[test]
fn test_save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("conv.json");

    let mut mem = ConversationMemory::new(&path);
    mem.push("user", "what's the disk usage?", "telegram");
    mem.push("assistant", "Disk is at 45%.", "telegram");
    mem.save().expect("save should succeed");

    let loaded = ConversationMemory::load(&path);
    assert_eq!(loaded.turns.len(), 2);
    assert_eq!(loaded.turns[0].role, "user");
    assert_eq!(loaded.turns[0].text, "what's the disk usage?");
    assert_eq!(loaded.turns[1].role, "assistant");
    assert_eq!(loaded.turns[1].channel, "telegram");
}

#[test]
fn test_load_nonexistent_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("does_not_exist.json");
    let mem = ConversationMemory::load(&path);
    assert!(mem.turns.is_empty(), "load of missing file should return empty");
}

#[test]
fn test_save_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("deep").join("conv.json");
    let mut mem = ConversationMemory::new(&path);
    mem.push("user", "hello", "telegram");
    mem.save().expect("save should create parent dirs");
    assert!(path.exists(), "file should exist after save");
}

// ── recent() ─────────────────────────────────────────────────────────────

#[test]
fn test_recent_returns_last_n() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = tmp_mem(&dir);
    for i in 0..5 {
        mem.push("user", &format!("msg{}", i), "telegram");
    }
    let recent = mem.recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].text, "msg3");
    assert_eq!(recent[1].text, "msg4");
}

#[test]
fn test_recent_more_than_available_returns_all() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = tmp_mem(&dir);
    mem.push("user", "only turn", "telegram");
    let recent = mem.recent(100);
    assert_eq!(recent.len(), 1);
}

#[test]
fn test_recent_empty_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mem = tmp_mem(&dir);
    assert!(mem.recent(5).is_empty());
}

// ── format_for_context() ─────────────────────────────────────────────────

#[test]
fn test_format_for_context_non_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = tmp_mem(&dir);
    mem.push("user", "what's the disk usage?", "telegram");
    mem.push("assistant", "Disk is at 45% (120GB / 250GB).", "telegram");
    let ctx = mem.format_for_context(10);
    assert!(!ctx.is_empty(), "should produce non-empty context");
    assert!(ctx.contains("Recent Conversations"), "should have header");
    assert!(ctx.contains("user"), "should include role");
    assert!(ctx.contains("disk usage"), "should include message content");
    assert!(ctx.contains("assistant"), "should include assistant role");
}

#[test]
fn test_format_for_context_empty_returns_empty_string() {
    let dir = tempfile::tempdir().unwrap();
    let mem = tmp_mem(&dir);
    let ctx = mem.format_for_context(10);
    assert!(ctx.is_empty(), "empty memory should produce empty string");
}

#[test]
fn test_format_for_context_shows_turn_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = tmp_mem(&dir);
    mem.push("user", "hello", "telegram");
    mem.push("assistant", "hi", "telegram");
    let ctx = mem.format_for_context(2);
    assert!(ctx.contains("2 turns"), "should show turn count in header");
}

// ── ConversationTurn serialization ────────────────────────────────────────

#[test]
fn test_turn_serializes_deserializes() {
    let turn = ConversationTurn {
        timestamp: "2026-03-22T14:30:00Z".to_string(),
        role: "user".to_string(),
        text: "hello world".to_string(),
        channel: "telegram".to_string(),
    };
    let json = serde_json::to_string(&turn).expect("serialization failed");
    let decoded: ConversationTurn = serde_json::from_str(&json).expect("deserialization failed");
    assert_eq!(decoded, turn);
    assert!(json.contains("2026-03-22T14:30:00Z"));
    assert!(json.contains("telegram"));
}

// ── timestamp generation ──────────────────────────────────────────────────

#[test]
fn test_push_records_timestamp() {
    let dir = tempfile::tempdir().unwrap();
    let mut mem = tmp_mem(&dir);
    mem.push("user", "test", "telegram");
    let ts = &mem.turns[0].timestamp;
    assert_eq!(ts.len(), 20, "timestamp should be 20 chars: {ts}"); // "2026-03-22T14:30:00Z"
    assert!(ts.ends_with('Z'), "timestamp should end with Z: {ts}");
    assert!(ts.contains('T'), "timestamp should contain T separator: {ts}");
}

// ── default_path ─────────────────────────────────────────────────────────

#[test]
fn test_default_path_points_to_axonix_dir() {
    let mem = ConversationMemory::default_path();
    let path_str = mem.path.to_string_lossy();
    assert!(
        path_str.contains(".axonix"),
        "default path should be under .axonix: {path_str}"
    );
    assert!(
        path_str.contains("conversation_memory"),
        "default path should contain conversation_memory: {path_str}"
    );
}
