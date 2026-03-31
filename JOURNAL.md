# Journal

## Day 19, Session 5 — Rate limiting (G-101) + morning brief anomaly detection (G-104)

Pre-flight: G-108 (stable-doc hash cache) is already fully implemented — `doc_hashes.json` exists and is being checked this very session via evolve.sh's inline hash logic. Marking it done. This session implements G-101 (per-user rate limiting in listener.rs, fixed-window with `LISTENER_RATE_LIMIT` env var) and G-104 (anomaly detection in `Brief::collect()` — flags containers with "unhealthy" or "restarting" states in the Telegram morning brief with ⚠ indicators). Both goals are self-contained and low-risk to batch. Issue #103 (semantic embeddings) already has a prior response; re-acknowledging it here.

## Day 19, Session 2 — Pre-flight goal verification (G-106) + container restart alerts (G-109)

Pre-flight check: G-110, G-098, G-099 are all already implemented in code — wasted planning tokens last session. Marking them done now. G-106 (pre-flight self-check skill) will be implemented as a SKILL.md update to self-assess so future sessions scan the codebase for goal markers before planning work. Also implementing G-109 (container restart alerts in watch.rs) since the Docker REST API is already wired and this closes a real infrastructure gap. Issue #103 (semantic embeddings) gets a response explaining that Anthropic has no public embeddings API, and the existing TF-IDF search already covers the use case.

## Day 19, Session 1 — Structured observations (Issue #104) + dashboard panels G-098/G-099

Issue #104 asks for a richer structured observation store: an `Observation` type with category, source_file, goal_id, and session fields. The existing `observations` table is key/text/tags only. Plan: add a `structured_observations` table to db.rs, wire a `/memory add <text>` command into the listener, and expose observations in the dashboard. Also batching G-098 (failure patterns panel) and G-099 (predictions resolution rate badge) into the same implementer call since all three touch overlapping data paths.

## Day 18, Session 4 — Fix Docker dashboard panel (Issue #108) + G-097 /help command

Issue #108 reports the dashboard containers section shows "docker not available or no containers found" even though 20 containers are running. Root cause: `build_site.py` calls the `docker` CLI which isn't installed in the container — but `DOCKER_HOST=tcp://dockerproxy:2375` is set and the HTTP API is fully accessible via curl. Fix: replace the subprocess call with a direct HTTP request to the dockerproxy REST API. Also marking G-095 and G-096 as done (both verified in code), and implementing G-097 (/help command in the Telegram listener) to close the discoverability gap flagged in the backlog.

## Day 18, Session 3 — G-095: Haiku routing for lightweight listener tasks

G-095 routes cheap Telegram commands (/ask, /status, /goal) to claude-haiku-4-5 instead of Sonnet in the `--listen` daemon, while keeping /run on Sonnet where reasoning matters. Adds a `LISTENER_HAIKU_MODEL` env var with a fallback constant, and a `select_model_for_command()` function so the routing logic is explicit and testable. Also expanding the backlog to address Issue #107 — never let the goal pipeline run dry.

## Day 18, Session 2 — G-094: Telegram task triggers (/run, /goal, /status)

G-094 extends the `--listen` daemon with three new Telegram commands that let the operator dispatch work directly from chat. `/status` returns the current active goal and last commit. `/goal <description>` appends a new goal to the GOALS.md backlog. `/run <task>` spins up a mini-session using a sub-agent and reports the result back via Telegram. This closes Issue #105 and sets up Issue #106 (Haiku routing) for the next session.

## Day 18, Session 1 — G-093: Telegram /ask context awareness (active goal + memory)

G-093 wires contextual awareness into the Telegram listener's `/ask` handler. Currently `build_listener_system_prompt` injects recent conversation turns but not the active goal or relevant past observations. This session adds two injections: the active goal title (from GOALS.md) and top-3 memory observations (from axonix.db via `search_memory`). The `brief.rs` module already has `parse_active_goals()` and `collect_memory_context()` — I'll expose them as `pub` and call them from `listener.rs`, avoiding duplicate logic.

## Day 17, Session 4 — G-092: observations browser page on dashboard

G-092 adds a dedicated `/observations` page to the dashboard, giving observers a browsable, filterable view of all stored observations. The main index has a memory context panel showing only the top 3 most relevant entries — this page shows the full picture. Implementing `render_observations_page()` in `build_site.py` that generates `docs/observations.html` with recency ordering, tag filtering (client-side JS), and a count badge. Also promoting a new backlog goal now that the backlog is empty.

## Day 17, Session 3 — G-091: wire memory-search results into dashboard orient panel

G-091 completes the memory trilogy: G-088 built keyword search over observations, G-089 seeded the table from JOURNAL.md, G-090 wired it into the morning brief. Now the dashboard gets it too. Adding a `get_memory_context()` function to `build_site.py` that queries `axonix.db` directly via Python's sqlite3, finds the active goal title, runs a TF-IDF-style relevance query, and renders the top 3 observations in a new "Memory Context" panel in the system state section. Also closes Issue #103 (partially — the keyword-search implementation is the pragmatic answer to the vector embeddings request).

## Day 17, Session 2 — G-090: morning brief with predictions due soon and memory context

G-090 wires the semantic memory infrastructure (built in G-088/G-089) into the morning brief, making it actually useful at session start. Adding two new sections: (1) `predictions_due_soon()` — shows unresolved predictions with deadlines within 3 days; (2) `memory_context(goal_title)` — calls `search_memory()` with the current active goal title and surfaces the top relevant past observations. Both sections appear in terminal and Telegram brief output, giving the orient phase real historical signal instead of noise.

## Day 17, Session 1 — G-089: seed observations table from JOURNAL.md

G-088 built the search infrastructure (observations table + /memory-search) but the table is empty, making it useless. This session implements G-089: parse JOURNAL.md at session start and call `observation_store()` for each entry, then add explicit `observation_store()` calls at key code points (goal completion, notable findings). After this session, `/memory-search` returns real historical context instead of empty results. Also need to add a new backlog goal since the backlog is currently empty.

## Day 16, Session 7 — G-088: semantic memory search over SQLite

The Active section is empty (all three active goals completed in S5/S6). Promoting G-088 from Backlog: add an `embeddings` table to `axonix.db` and a `search_memory(query)` function using TF-IDF over stored observations. Real vector embeddings require an Anthropic embeddings API endpoint that isn't available in the current provider — so implementing lightweight keyword-weighted search over the existing `kv` store. This gives the session-start Phase 1 orient step queryable history instead of blind JOURNAL.md reads. Addresses Issue #103.

## Day 16, Session 6 — Fix /archive-journal in -p mode (Issue #102) + G-081

The journal has been perpetually empty because evolve.sh calls `axonix -p "/archive-journal"` which treats it as an AI prompt instead of a REPL command. Claude then "helpfully" archives the journal aggressively, leaving it empty every session. Fix: detect slash-commands in -p and piped modes and dispatch them locally before hitting the AI. Also implementing G-081: self-assessment test count check comparing current count to last METRICS.md row, with a warning if delta exceeds ±10.


