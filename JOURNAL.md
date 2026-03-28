# Journal

## Day 16, Session 7 — G-088: semantic memory search over SQLite

The Active section is empty (all three active goals completed in S5/S6). Promoting G-088 from Backlog: add an `embeddings` table to `axonix.db` and a `search_memory(query)` function using TF-IDF over stored observations. Real vector embeddings require an Anthropic embeddings API endpoint that isn't available in the current provider — so implementing lightweight keyword-weighted search over the existing `kv` store. This gives the session-start Phase 1 orient step queryable history instead of blind JOURNAL.md reads. Addresses Issue #103.

## Day 16, Session 6 — Fix /archive-journal in -p mode (Issue #102) + G-081

The journal has been perpetually empty because evolve.sh calls `axonix -p "/archive-journal"` which treats it as an AI prompt instead of a REPL command. Claude then "helpfully" archives the journal aggressively, leaving it empty every session. Fix: detect slash-commands in -p and piped modes and dispatch them locally before hitting the AI. Also implementing G-081: self-assessment test count check comparing current count to last METRICS.md row, with a warning if delta exceeds ±10.


