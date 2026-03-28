# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-088 — Semantic memory search (Issue #103)
**Why:** Axonix blindly reads JOURNAL.md for context instead of querying relevant past observations. Keyword/TF-IDF search over the SQLite DB would surface targeted context without loading the full journal.
**Definition of done:** `search_memory` table in axonix.db storing observation text, `search_memory(query)` function returning relevant past entries by keyword overlap, callable from session start during Phase 1 orient. Exposed via `/memory-search <query>` REPL command.
**Status:** [x] — completed Day 16 S7

### G-089 — Seed observations table from journal entries
**Why:** G-088 built the search infrastructure but the observations table is empty. Axonix needs to populate it with data — the natural source is JOURNAL.md entries parsed on session start, plus any explicit `observation_store()` calls from code paths that discover important facts.
**Definition of done:** On session start (in `main.rs` or `brief.rs`), parse JOURNAL.md entries and call `observation_store()` for each one with key=`journal:<date>:<title_slug>`, text=body, tags=`journal`. After a session, at least 3 observations are retrievable via `/memory-search`.
**Status:** [ ]

## Backlog

<!-- Next candidates for promotion -->

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-088] Semantic memory search: observations table + /memory-search REPL cmd — Day 16 S7
- [x] [G-087] Fix /archive-journal slash-command in -p and piped modes — Day 16 S6
- [x] [G-081] Self-assessment: auto-detect test count discrepancies — Day 16 S6
- [x] [G-085] Morning brief: last 3 journal entries in terminal + Telegram output — Day 16 S5
- [x] [G-086] Dashboard: live session stream viewer — SSE panel on axonix.live connecting to stream.axonix.live
