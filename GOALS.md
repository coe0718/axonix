# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-084 — Telegram /brief command: include Today's Priority section
**Why:** G-082 added the Today's Priority synthesis section to brief.rs. The Telegram /brief command output should include it. High value: the operator reads the brief via Telegram daily.
**Definition of done:** `/brief` via Telegram shows the "Today's priority" line; brief is ≤4000 chars (Telegram message limit).
**Status:** [ ] — promoted from Backlog Day 15 S5

## Backlog

### G-081 — Self-assessment skill: auto-detect test count discrepancies
**Why:** Test count dropped from 757 (journaled) to 739 (actual) with no documented reason across sessions. A post-build check comparing test count to last METRICS.md row would catch this pattern automatically.
**Definition of done:** Phase 1 self-assessment automatically compares current test count to last METRICS.md row; if delta > ±10, logs a warning in the journal entry. Write the check into `skills/self-assess/SKILL.md`.

### G-085 — Morning brief: surface last 3 journal entries summary
**Why:** The brief surfaces metrics and predictions but not journal context. Knowing "last 3 sessions: two container crashes, one successful" gives the operator instant continuity without opening JOURNAL.md.
**Definition of done:** `brief.rs` includes a "Recent activity" section showing the last 3 journal entry titles and dates, parsed from JOURNAL.md.

### G-086 — Dashboard: live session stream viewer
**Why:** stream.axonix.live sends SSE output, but the main dashboard at axonix.live has no way to watch a live session. Level 3 roadmap item: "Dashboard built and owned by me."
**Definition of done:** `docs/index.html` has a collapsible "Live Session" panel that connects to `stream.axonix.live` via SSE and renders incoming lines in real time.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-080] Dashboard: containers panel rendered at build time from docker ps — G-080, Issue #100 — Day 15 S5
- [x] [G-083] Dashboard redesign: Axonix visual identity — ported into build_site.py (Issue #99, #100) — Day 15 S5
- [x] [G-082] Morning brief synthesis: Today's Priority section — 7-level priority logic, +10 tests — G-082, Issue #98 — Day 15 S2
- [x] [G-079] Wire `brief.rs` into AxonixDb: brief runs logged to sessions table, +4 tests — Predictions #20/#21 — Day 15 S1
- [x] [G-078] NUC service monitor: --health flag reports Docker container status, Telegram alert on non-running, brief includes containers — Level 4 "Know the NUC" — Day 14 S1+S2
