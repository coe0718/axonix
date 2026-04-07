/// A row from the `sessions` table.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: i64,
    pub day: i64,
    pub session: String,
    pub date: String,
    pub tokens: Option<String>,
    pub tests: Option<i64>,
    pub failed: Option<i64>,
    pub notes: Option<String>,
}

/// A row from the `goals` table.
#[derive(Debug, Clone)]
pub struct GoalRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// A row from the `observations` table, with a computed relevance score.
#[derive(Debug, Clone)]
pub struct ObservationRow {
    pub id: i64,
    pub key: String,
    pub text: String,
    pub tags: String,
    pub created_at: String,
    /// Relevance score set by `search_memory`; not stored in DB.
    pub score: f64,
}

/// A row from the `structured_observations` table.
#[derive(Debug, Clone)]
pub struct StructuredObservation {
    pub id: i64,
    pub content: String,
    /// One of: learned | tried_and_failed | blocked_by | dependency_discovered | pattern_noticed
    pub category: String,
    /// Source file, or empty string if unknown.
    pub source_file: String,
    /// Goal ID, e.g. "G-110", or empty string if unknown.
    pub goal_id: String,
    /// Session label, e.g. "Day 19 S1".
    pub session: String,
    /// Comma-separated tags.
    pub tags: String,
    pub created_at: String,
}

/// A row from the `hot_memories` table.
#[derive(Debug, Clone)]
pub struct HotMemoryRow {
    pub id: i64,
    pub content: String,
    pub summary: String,
    pub entities: String,
    pub topics: String,
    pub importance: f64,
    pub created_at: String,
    pub last_accessed: String,
    pub access_count: i64,
    pub expires_at: String,
}

/// A row from the `cold_memories` table.
#[derive(Debug, Clone)]
pub struct ColdMemoryRow {
    pub id: i64,
    pub content: String,
    pub topics: String,
    pub importance: f64,
    pub created_at: String,
    pub reinforcement_count: i64,
    pub last_reinforced: String,
    pub expires_at: String,
}

/// A row from the `memory_contradictions` table.
#[derive(Debug, Clone)]
pub struct ContradictionRow {
    pub id: i64,
    pub cold_memory_id: i64,
    pub new_memory: String,
    pub created_at: String,
    pub resolved: bool,
}
