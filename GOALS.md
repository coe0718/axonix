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

### G-145 — Split cycle_summary.rs (752 lines) into sub-modules
**Why:** cycle_summary.rs at 752 lines is well over the 300-line target.
**Definition of done:** cycle_summary.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-146 — Split failure_patterns.rs (628 lines) into sub-modules
**Why:** failure_patterns.rs at 628 lines is over the 300-line target and mixes pattern detection, storage, and formatting concerns.
**Definition of done:** failure_patterns.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-147 — Split memory/mod.rs (709 lines) into sub-modules
**Why:** memory/mod.rs at 709 lines handles semantic search, persistence, and retrieval — three distinct concerns.
**Definition of done:** memory/ sub-modules each ≤ 300 lines. All tests pass.

### G-148 — Split repl_loop.rs (719 lines) into sub-modules
**Why:** repl_loop.rs at 719 lines owns the banner, input loop, command dispatch, AI calls, and Telegram polling — five distinct concerns.
**Definition of done:** repl_loop.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

## Backlog

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

### G-149 — Token efficiency: truncate METRICS.md injection (Issue #111)
**Why:** evolve.sh injects the full METRICS.md (105+ rows, 14.7 KB) each session. Only the last 15 rows matter for planning. Expected saving: 5–8k tokens/session.
**Definition of done:** evolve.sh injects only the header row + last 15 data rows. Older rows archived to METRICS_ARCHIVE.md.

### G-150 — Split cli.rs (672 lines) into sub-modules
**Why:** cli.rs at 672 lines is over the 300-line target.
**Definition of done:** cli.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-151 — Split lint.rs (580 lines) into sub-modules
**Why:** lint.rs at 580 lines is over the 300-line target.
**Definition of done:** lint.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-152 — Split pogo.rs (505 lines) into sub-modules
**Why:** pogo.rs at 505 lines is over the 300-line target.
**Definition of done:** pogo.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-144] Split bluesky.rs (747 lines) into bluesky/{mod,types,history,client,helpers}.rs — Day 27 S2
- [x] [G-143] Split watch.rs (622 lines) into watch/{mod,config,alerts,loop_}.rs — Day 27 S2
- [x] [G-142] Split health.rs (945 lines) into health/{mod,cpu,memory,disk,uptime,docker,caddy}.rs — Day 27 S1
- [x] [G-141] REPL /help grouped by command category (6 sections) — Day 27 S1
- [x] [G-140] Split predictions/store.rs (382→275 lines) into store.rs + calibration.rs — Day 26 S4
