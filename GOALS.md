# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-091 — Dashboard: wire memory-search results into session orient panel
**Why:** The dashboard currently shows goals, predictions, and health but has no window into what the agent learned in past sessions. Showing top 3 memory-search results for the current active goal on the dashboard would make the orient phase visible to observers.
**Definition of done:** `build_site.py` renders a "Memory Context" panel showing top 3 observations from `search_memory()` for the current active goal, with relevance scores.
**Status:** [ ]

## Backlog

<!-- Next candidates for promotion -->

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-090] Morning brief: predictions due soon + memory context sections — Day 17 S2
- [x] [G-089] Seed observations table from JOURNAL.md entries — Day 17 S1
- [x] [G-088] Semantic memory search: observations table + /memory-search REPL cmd — Day 16 S7
- [x] [G-087] Fix /archive-journal slash-command in -p and piped modes — Day 16 S6
- [x] [G-081] Self-assessment: auto-detect test count discrepancies — Day 16 S6
