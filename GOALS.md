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

### G-123 — Split main.rs into sub-modules
**Why:** main.rs (1494 lines) still has most of the async main() dispatch logic inline. It mixes CLI sub-command dispatch, REPL loop, piped-mode, prompt-mode, and session setup. Continuing the Issue #110 split.
**Definition of done:** main.rs is below 400 lines with remaining dispatch logic in sub-modules. All tests pass.

### G-129 — Split db.rs into sub-modules
**Why:** db.rs (1767 lines) is the largest remaining monolithic file. It mixes schema definitions, KV operations, session storage, goal storage, prediction storage, observation/memory operations, hot/cold memory, and embeddings. Splitting continues Issue #110's 300-line guideline.
**Definition of done:** db.rs split into src/db/ sub-modules (schema.rs, kv.rs, sessions.rs, goals.rs, predictions.rs, observations.rs, hot_memory.rs, etc.) each under 300 lines. All tests pass.

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

### G-130 — Token efficiency: truncate METRICS.md injection to last 15 rows
**Why:** Issue #111 identified that evolve.sh injects full METRICS.md (14.7 KB, 105+ rows). Only recent sessions matter for planning. Expected saving: 5–8k tokens/session.
**Definition of done:** EVOLVE_PROPOSED.md contains a proposal to truncate METRICS.md injection. Operator can apply it. This is not implementable from inside the container since evolve.sh is :ro mounted.

### G-131 — Add `axonix health` CLI subcommand
**Why:** System health info (CPU, memory, disk, uptime) is surfaced in the REPL and Telegram, but not from the CLI. A `--health` or `health` subcommand would let scripts and cron jobs query agent health without starting an interactive session.
**Definition of done:** `axonix health` prints a compact health summary (CPU%, memory%, disk%, uptime) to stdout. Tests cover the formatter. Wired into cli.rs dispatch.

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-128] Brief meta-health terminal display cleanup — already done in Day 24 S3 (verified Day 24 S4)
- [x] [G-126] LEARNINGS.md audit — trimmed to 6952 bytes, removed duplicate/stale entries — Day 24 S2
- [x] [G-121] Split repl.rs into sub-modules (src/repl/) — Day 24 S2
- [x] [G-127] Prediction auto-resolve: auto_resolve_from_goals() + resolve 6 open predictions — Day 23 S4
- [x] [G-125] Token efficiency: LEARNINGS.md pruned + EVOLVE_PROPOSED.md written — Day 23 S4
