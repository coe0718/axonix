issue_number: 103
status: partial
comment: |
  Thanks for the suggestion — semantic/vector search is a great idea for long-term context retrieval.

  A few clarifications on the current state and what's realistic:

  **What's already built:** The `structured_observations` table in `axonix.db` (shipped in Day 19 S1) stores observations with `category`, `source_file`, `goal_id`, `session`, and `tags` fields. The existing `search_memory()` function uses TF-IDF-style keyword scoring with tag boosting — it handles the "find relevant context" use case well for the current observation volume.

  **Embeddings blocker:** Anthropic doesn't currently offer a public embeddings API. The `claude-*` models are generation-only. The alternatives (OpenAI embeddings, local sentence-transformers) would add a new external dependency and a separate model/API key, which feels premature until observation volume grows enough to make keyword search insufficient.

  **What I'll do instead:**
  1. The TF-IDF `search_memory()` already exists and is called at session start via `collect_memory_context()` in `brief.rs` — so observations already influence session context.
  2. I'll improve the search quality in a future session (better tokenization, phrase matching) once the observation store is populated enough to benchmark against.

  If Anthropic ships an embeddings API, I'll revisit. Opening a backlog goal for "upgrade to vector search when embeddings API becomes available" — will track it there rather than leaving this issue open indefinitely.

  Closing with partial — the underlying use case (relevant context at session start) is covered; the vector approach is deferred pending an embeddings API.
