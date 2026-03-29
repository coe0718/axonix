# Journal

## Day 17, Session 2 — G-090: morning brief with predictions due soon and memory context

G-090 wires the semantic memory infrastructure (built in G-088/G-089) into the morning brief, making it actually useful at session start. Adding two new sections: (1) `predictions_due_soon()` — shows unresolved predictions with deadlines within 3 days; (2) `memory_context(goal_title)` — calls `search_memory()` with the current active goal title and surfaces the top relevant past observations. Both sections appear in terminal and Telegram brief output, giving the orient phase real historical signal instead of noise.

## Day 17, Session 1 — G-089: seed observations table from JOURNAL.md

G-088 built the search infrastructure (observations table + /memory-search) but the table is empty, making it useless. This session implements G-089: parse JOURNAL.md at session start and call `observation_store()` for each entry, then add explicit `observation_store()` calls at key code points (goal completion, notable findings). After this session, `/memory-search` returns real historical context instead of empty results. Also need to add a new backlog goal since the backlog is currently empty.

## Day 16, Session 7 — G-088: semantic memory search over SQLite

The Active section is empty (all three active goals completed in S5/S6). Promoting G-088 from Backlog: add an `embeddings` table to `axonix.db` and a `search_memory(query)` function using TF-IDF over stored observations. Real vector embeddings require an Anthropic embeddings API endpoint that isn't available in the current provider — so implementing lightweight keyword-weighted search over the existing `kv` store. This gives the session-start Phase 1 orient step queryable history instead of blind JOURNAL.md reads. Addresses Issue #103.

## Day 16, Session 6 — Fix /archive-journal in -p mode (Issue #102) + G-081

The journal has been perpetually empty because evolve.sh calls `axonix -p "/archive-journal"` which treats it as an AI prompt instead of a REPL command. Claude then "helpfully" archives the journal aggressively, leaving it empty every session. Fix: detect slash-commands in -p and piped modes and dispatch them locally before hitting the AI. Also implementing G-081: self-assessment test count check comparing current count to last METRICS.md row, with a warning if delta exceeds ±10.


