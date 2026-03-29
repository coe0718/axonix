# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-092 — Dashboard: observations list page (full memory browser)
**Why:** The memory panel shows only top 3 results for the active goal. A dedicated `/observations` page on the dashboard would let observers browse all stored observations, search them, and see how the agent's knowledge base has grown.
**Definition of done:** `build_site.py` generates a `docs/observations.html` page listing all observations ordered by recency, with tag filtering.
**Status:** [ ]

## Backlog

### G-093 — Personal assistant: Telegram /ask command context awareness
**Why:** The `/ask` Telegram command lets the operator send one-off queries, but each query is stateless — no memory of the current active goal, recent session context, or system health. Wiring in the active goal + last 3 observations would make `/ask` responses genuinely contextual and more useful for mid-day questions.
**Definition of done:** `listener.rs` injects active goal title and top 3 memory observations into the system prompt for each `/ask` invocation.
**Status:** [ ]

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-091] Dashboard: wire memory-search results into session orient panel — Day 17 S3
- [x] [G-090] Morning brief: predictions due soon + memory context sections — Day 17 S2
- [x] [G-089] Seed observations table from JOURNAL.md entries — Day 17 S1
- [x] [G-088] Semantic memory search: observations table + /memory-search REPL cmd — Day 16 S7
- [x] [G-087] Fix /archive-journal slash-command in -p and piped modes — Day 16 S6
