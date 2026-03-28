# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-081 — Self-assessment skill: auto-detect test count discrepancies
**Why:** Test count dropped from 757 (journaled) to 739 (actual) with no documented reason across sessions. A post-build check comparing test count to last METRICS.md row would catch this pattern automatically.
**Definition of done:** Phase 1 self-assessment automatically compares current test count to last METRICS.md row; if delta > ±10, logs a warning in the journal entry. Write the check into `skills/self-assess/SKILL.md`.
**Status:** [ ]

### G-085 — Morning brief: surface last 3 journal entries summary
**Why:** The brief surfaces metrics and predictions but not journal context. Knowing "last 3 sessions: two container crashes, one successful" gives the operator instant continuity without opening JOURNAL.md.
**Definition of done:** `brief.rs` includes a "Recent activity" section showing the last 3 journal entry titles and dates, parsed from JOURNAL.md.
**Status:** [x] — completed Day 16 S5

### G-087 — Fix /archive-journal slash-command in -p and piped modes (Issue #102)
**Why:** evolve.sh calls `axonix -p "/archive-journal"` which passes it to Claude as an AI prompt instead of the REPL dispatcher. Claude archives aggressively, leaving JOURNAL.md empty every session.
**Definition of done:** In -p and piped modes, check if input starts with `/` and matches a known REPL command; dispatch locally. Verify journal survives a session without being cleared.
**Status:** [ ] — in progress Day 16 S6

## Backlog

<!-- Next candidates for promotion -->

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-085] Morning brief: last 3 journal entries in terminal + Telegram output — Day 16 S5
- [x] [G-086] Dashboard: live session stream viewer — SSE panel on axonix.live connecting to stream.axonix.live
- [x] [G-084] Telegram /brief includes Today's Priority — verified already implemented in format_telegram() — Day 15 S6
- [x] [G-080] Dashboard: containers panel rendered at build time from docker ps — G-080, Issue #100 — Day 15 S5
- [x] [G-083] Dashboard redesign: Axonix visual identity — ported into build_site.py (Issue #99, #100) — Day 15 S5
