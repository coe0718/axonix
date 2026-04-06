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

### G-121 — Split brief.rs and repl.rs into sub-modules
**Why:** brief.rs (3170 lines) and repl.rs (2459 lines) are the two worst offenders now that telegram.rs and listener.rs have been split. All files should trend toward <300 lines per Issue #110.
**Definition of done:** brief.rs and repl.rs each split into logical sub-modules. All tests pass.

### G-125 — Token efficiency: evolve.sh GOALS.md injection + LEARNINGS.md pruning
**Why:** Issue #111 — METRICS.md archiving was done in Day 23 S2. Remaining high-value items: evolve.sh reads GOALS.md for prompt injection but the instruction says to read GOALS.md as a file — verify if it's being double-injected. LEARNINGS.md is 10KB and growing. Prune stale Day 2 entries and propose EVOLVE_PROPOSED.md changes.
**Definition of done:** EVOLVE_PROPOSED.md written with concrete evolve.sh changes. LEARNINGS.md stale sections removed. Expected context saving: 3-5k tokens/session.

## Backlog

### G-112 — Self-assessment: verify Active/Backlog counts at session start
**Why:** The goal hygiene rule (≥2 Active, ≥5 Backlog) keeps failing because there's no automated check at session start. Adding a check to Phase 1 self-assessment that explicitly counts and flags violations makes the rule self-enforcing.
**Definition of done:** During Phase 1, parse GOALS.md and print `[GOALS] Active: N, Backlog: M` — and print a warning if either is below minimum. Add this to LEARNINGS.md as a Phase 1 step.

### G-118 — Failure pattern Telegram alert
**Why:** Failure patterns accumulate silently. When a new pattern is recorded that has appeared 3+ times, send a Telegram alert so the operator notices.
**Definition of done:** In watch.rs or failure_patterns.rs, after writing a new failure pattern, check if any pattern has count ≥ 3 and hasn't been alerted yet. Send one Telegram message per new threshold breach.

### G-119 — Dashboard: show last build time
**Why:** The dashboard header has no indication of when the site was last built. Adding a "Last built: YYYY-MM-DD HH:MM" timestamp to the footer gives the operator instant confidence the data is fresh.
**Definition of done:** `build_site.py` injects the current UTC timestamp into the dashboard footer. Visible on the live site after next build.

### G-123 — Split main.rs into sub-modules
**Why:** main.rs (2110 lines) is the third-largest file after brief.rs and repl.rs. It mixes CLI dispatch, listener setup, and agent configuration. Splitting into main/dispatch.rs, main/setup.rs, etc. would make each piece readable.
**Definition of done:** main.rs split into logical sub-modules with no file exceeding ~400 lines. All tests pass.

### G-126 — LEARNINGS.md staleness pruning
**Why:** LEARNINGS.md is 10KB with entries dating back to Day 2 (March 2026). The Day 2 bottleneck analysis and resolved infrastructure notes are stale — they add context overhead without informing current decisions. Issue #111 explicitly flags this.
**Definition of done:** Stale Day 2 entries moved to LEARNINGS_ARCHIVE.md or removed. File reduced below 5KB. Each remaining section has a last-relevant date. Expected saving: ~3k tokens/session.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-122] /predictions Telegram command — Day 23 S3 (verified already implemented + tests passing)
- [x] [G-120] Split telegram.rs + listener.rs into sub-modules — Day 23 S2
- [x] [G-116] Dashboard: session timeline panel — Day 23 S2 (verified already implemented)
- [x] [G-115] Telegram /resolve command — Day 23 S1
- [x] [G-113] Telegram /predict command — Day 22 S3
