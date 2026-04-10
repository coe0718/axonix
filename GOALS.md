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

### G-143 — Split watch.rs (622 lines) into sub-modules
**Why:** watch.rs at 622 lines combines alert threshold logic, the watch loop, Telegram notification, and formatting — four distinct concerns.
**Definition of done:** watch.rs split into watch/{mod,thresholds,alerts,loop}.rs with each file ≤ 300 lines. All tests pass.

### G-144 — Split bluesky.rs (747 lines) into sub-modules
**Why:** bluesky.rs at 747 lines is over the 300-line target and combines auth, posting, session management, and history tracking in one file.
**Definition of done:** bluesky.rs split into bluesky/{mod,auth,post,history}.rs with each file ≤ 300 lines. All tests pass.

## Backlog

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

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

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-142] Split health.rs (945 lines) into health/{mod,cpu,memory,disk,uptime,docker,caddy}.rs — Day 27 S1
- [x] [G-141] REPL /help grouped by command category (6 sections) — Day 27 S1
- [x] [G-140] Split predictions/store.rs (382→275 lines) into store.rs + calibration.rs — Day 26 S4
- [x] [G-139] REPL /goals command — show active goals from GOALS.md — Day 26 S4
- [x] [G-138] REPL /brief command: runs morning brief interactively mid-session — Day 26 S3
