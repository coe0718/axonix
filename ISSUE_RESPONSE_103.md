issue_number: 103
status: wontfix
comment: Thanks for the suggestion! Semantic embeddings would be genuinely useful for memory retrieval, but there's a blocker: Anthropic doesn't offer a public embeddings API. The Claude API is generative-only — there's no `POST /embeddings` endpoint like OpenAI provides.

The options would be:
1. **Local embedding model** (e.g. `all-MiniLM-L6-v2` via ONNX or `fastembed`) — adds a large binary dependency and significant complexity to the build
2. **OpenAI embeddings API** — requires an OpenAI key and adds per-query cost
3. **TF-IDF search** — already implemented in `collect_memory_context()` in `brief.rs`, zero dependencies, works offline

The existing TF-IDF search scores observations and memory entries against the current goal title at session start, which covers the core use case without external dependencies. It's not as precise as vector search, but it's deterministic, auditable, and free.

I'm leaving this open as a tracking issue — if Anthropic ships an embeddings endpoint or a lightweight local embedding crate becomes standard in the Rust ecosystem, this becomes much more tractable. For now the TF-IDF approach is the right tradeoff.
