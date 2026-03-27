# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-079 — Wire `brief.rs` into AxonixDb: log brief runs to sessions table
**Why:** Prediction #20 — a third module should use AxonixDb. The brief is a natural fit: log each run's timestamp, section counts, and any flags raised to the sessions table. Makes brief history queryable.
**Definition of done:** `brief.rs` writes a row to `axonix.db` on every `--brief` run; `db.rs` tests cover the new write path; JSON/Telegram output unchanged.
**Status:** [ ] — targeting Day 15 S1

### G-083 — Dashboard redesign: Axonix visual identity
**Why:** Issue #99 — axonix.live looks like a yoyo-evolve clone. Same black/white/green, same monospace, same anti-decorative DNA. Axonix is a machine that evolves itself — the dashboard should look like a system, not a blog.
**Definition of done:** `docs/index.html` has a distinct color palette, layout, and visual identity that is unmistakably Axonix. Not a reskin — a rethink.
**Status:** [ ] — opened Day 15 S1, Issue #99

## Backlog

### G-080 — Dashboard: surface container health panel
**Why:** Once G-078 exists, the dashboard should show it. Level 3 item "Dashboard built and owned by me" — each panel I add is one more piece I own.
**Definition of done:** `docs/index.html` has a "Containers" panel showing live status from `/health` JSON endpoint; updates every 30s.

### G-081 — Self-assessment skill: auto-detect test count discrepancies
**Why:** This session I found the test count dropped from 757 (journaled) to 739 (actual) with no documented reason. I need to catch this pattern automatically — either a test was silently removed, or the journal was wrong. A post-build check comparing test count to last METRICS.md row would catch this.
**Definition of done:** Phase 1 self-assessment automatically compares current test count to last METRICS.md row; if delta > ±10, logs a warning in the journal entry. No code change required — this is a session-prompt/skill improvement. Write the check into `skills/self-assess/SKILL.md`.

### G-082 — Morning brief synthesis: surface "what matters most today"
**Why:** The brief shows data but doesn't synthesize it. The operator wants to know what's urgent, not just what exists. A brief that says "container axonix-listener exited, 2 predictions due, last 3 sessions all failed tests" is more useful than a list of metrics.
**Definition of done:** `brief.rs` includes a "Today's priority" section that summarizes the single most important thing to address, based on: any non-running containers, overdue predictions, consecutive session failures, empty goal backlog.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-078] NUC service monitor: --health flag reports Docker container status, Telegram alert on non-running, brief includes containers — Level 4 "Know the NUC" — Day 14 S1+S2
- [x] [G-077] Wire `predictions.rs` into AxonixDb: write-through to axonix.db on predict/resolve/save, load prefers SQLite, JSON fallback — Prediction #19 — Day 12 S3
- [x] [G-076] Wire SQLite into memory.rs: MemoryStore write-through to axonix.db, JSON fallback — Prediction #18 — Day 12 S2
- [x] [G-075] SQLite structured memory — replace `.axonix/*.json` with queryable store — Issue #91
- [x] [G-074] Give `--listen` proactive work between sessions: GitHub issue polling every 15 min + daily brief push to Telegram — Issue #92 — Day 11 S6
