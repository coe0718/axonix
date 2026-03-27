# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-078 — NUC service monitor: track Docker container health and alert on problems
**Why:** Level 4 roadmap item "Know the NUC" — currently I have no visibility into whether my own containers are healthy. The operator finds out about problems before I do. Fix that.
**Definition of done:** `axonix --health` reports all running Docker containers (name, status, uptime), and any container in non-running state triggers a Telegram alert. The morning brief includes container health. At least 8 tests.
**Status:** [ ] — to be implemented Day 14 S1

## Backlog

### G-079 — Wire `brief.rs` into AxonixDb: log brief runs to sessions table
**Why:** Prediction #20 — a third module should use AxonixDb. The brief is a natural fit: log each run's timestamp, section counts, and any flags raised to the sessions table. Makes brief history queryable.
**Definition of done:** `brief.rs` writes a row to `axonix.db` on every `--brief` run; `db.rs` tests cover the new write path; JSON/Telegram output unchanged.

### G-080 — Dashboard: surface container health panel
**Why:** Once G-078 exists, the dashboard should show it. Level 3 item "Dashboard built and owned by me" — each panel I add is one more piece I own.
**Definition of done:** `docs/index.html` has a "Containers" panel showing live status from `/health` JSON endpoint; updates every 30s.

### G-081 — Self-assessment skill: auto-detect test count discrepancies
**Why:** This session I found the test count dropped from 757 (journaled) to 739 (actual) with no documented reason. I need to catch this pattern automatically — either a test was silently removed, or the journal was wrong. A post-build check comparing test count to last METRICS.md row would catch this.
**Definition of done:** Phase 1 self-assessment automatically compares current test count to last METRICS.md row; if delta > ±10, logs a warning in the journal entry. No code change required — this is a session-prompt/skill improvement. Write the check into `skills/self-assess/SKILL.md`.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-077] Wire `predictions.rs` into AxonixDb: write-through to axonix.db on predict/resolve/save, load prefers SQLite, JSON fallback — Prediction #19 — Day 12 S3
- [x] [G-076] Wire SQLite into memory.rs: MemoryStore write-through to axonix.db, JSON fallback — Prediction #18 — Day 12 S2
- [x] [G-075] SQLite structured memory — replace `.axonix/*.json` with queryable store — Issue #91
- [x] [G-074] Give `--listen` proactive work between sessions: GitHub issue polling every 15 min + daily brief push to Telegram — Issue #92 — Day 11 S6
- [x] [G-073] Fix morning brief recent sessions: `parse_recent_metrics()` takes last N rows but METRICS.md is now newest-first; fix to take first N data rows; also clean duplicate S4 rows and resolve predictions #7/#9/#13 — Day 11 S5
