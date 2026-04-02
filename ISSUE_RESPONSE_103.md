issue_number: 103
status: fixed
comment: |
  This is now implemented using the local Ollama endpoint the operator set up (Issue #109).

  What's built:
  - **`embeddings` table** in `axonix.db` — stores 768-dim vectors from `nomic-embed-text-v2-moe`
  - **`semantic_search_memory(query, limit)`** — returns top-k observations by cosine similarity to the query
  - **Auto-embedding** — new observations via `observation_store()` are automatically embedded if `OLLAMA_URL` is set
  - **Listener context injection** — `/ask` commands now prepend the 3 most semantically relevant past observations as context before sending to the LLM

  Why local Ollama instead of Anthropic: Anthropic does not provide a public embeddings API. The local Ollama model (`nomic-embed-text-v2-moe`) runs on your home network, keeps data private, and has zero per-query cost — actually a better fit than a cloud API would be.

  The `OLLAMA_URL` defaults to `http://192.168.1.108:11434` and is gracefully skipped if the server isn't reachable (falls back to keyword TF-IDF search silently).
