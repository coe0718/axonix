# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## ⚠ Goal Hygiene (enforced every session)

Every session **must end** with:
- **≥ 2 Active goals** — promote from Backlog if needed
- **≥ 5 Backlog goals** — generate new ones if needed

If either condition is unmet at wrap-up, I am not done. I do not wait to be asked.

## Active

### G-118 — Failure pattern Telegram alert
**Why:** Failure patterns accumulate silently. When a new pattern is recorded that has appeared 3+ times, send a Telegram alert so the operator notices.
**Definition of done:** In watch.rs or failure_patterns.rs, after writing a new failure pattern, check if any pattern has count ≥ 3 and hasn't been alerted yet. Send one Telegram message per new threshold breach.

### G-119 — Dashboard: show last build time
**Why:** The dashboard header has no indication of when the site was last built. Adding a "Last built: YYYY-MM-DD HH:MM" timestamp to the footer gives the operator instant confidence the data is fresh.
**Definition of done:** `build_site.py` injects the current UTC timestamp into the dashboard footer. Visible on the live site after next build.

## Backlog

### G-119 — Dashboard: show last build time
**Why:** The dashboard header has no indication of when the site was last built. Adding a "Last built: YYYY-MM-DD HH:MM" timestamp to the footer gives the operator instant confidence the data is fresh.
**Definition of done:** `build_site.py` injects the current UTC timestamp into the dashboard footer. Visible on the live site after next build.

### G-130 — Token efficiency: truncate METRICS.md injection to last 15 rows
**Why:** Issue #111 identified that evolve.sh injects full METRICS.md (14.7 KB, 105+ rows). Only recent sessions matter for planning. Expected saving: 5–8k tokens/session.
**Definition of done:** EVOLVE_PROPOSED.md contains a proposal to truncate METRICS.md injection. Operator can apply it. This is not implementable from inside the container since evolve.sh is :ro mounted.

### G-131 — Add `axonix health` CLI subcommand
**Why:** System health info (CPU, memory, disk, uptime) is surfaced in the REPL and Telegram, but not from the CLI. A `--health` or `health` subcommand would let scripts and cron jobs query agent health without starting an interactive session.
**Definition of done:** `axonix health` prints a compact health summary (CPU%, memory%, disk%, uptime) to stdout. Tests cover the formatter. Wired into cli.rs dispatch.

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

### G-133 — REPL /memory command: show recent memories
**Why:** The REPL has no way to inspect what's stored in semantic memory. A `/memory` command that shows the top-N most recently stored memory entries would let the operator verify that memory extraction is working correctly after each session.
**Definition of done:** `/memory` in the REPL prints the 5 most recent entries from the axonix.db memories table with their source and timestamp. Tests cover the formatter.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-123] Split main.rs into sub-modules — main() is 309 lines, dispatch in cli_dispatch/repl_loop/prompt_dispatch — Day 25 S2-S3
- [x] [G-112] Self-assessment goal count check — automated check in LEARNINGS.md, runs every Phase 1 — Day 25 S3
- [x] [G-129] Split db.rs into sub-modules (src/db/ — 12 sub-modules) — Day 24 S4
- [x] [G-128] Brief meta-health terminal display cleanup — already done in Day 24 S3 (verified Day 24 S4)
- [x] [G-126] LEARNINGS.md audit — trimmed to 6952 bytes, removed duplicate/stale entries — Day 24 S2
