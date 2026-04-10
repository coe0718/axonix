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

### G-147 — Split memory/mod.rs (709 lines) into sub-modules
**Why:** memory/mod.rs at 709 lines handles semantic search, persistence, and retrieval — three distinct concerns.
**Definition of done:** memory/ sub-modules each ≤ 300 lines. All tests pass.

### G-148 — Split repl_loop.rs (719 lines) into sub-modules
**Why:** repl_loop.rs at 719 lines owns the banner, input loop, command dispatch, AI calls, and Telegram polling — five distinct concerns.
**Definition of done:** repl_loop.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-153 — Repair mode: recover from build/test failures without human intervention (Issue #114)
**Why:** When `cargo build` or `cargo test` fails, evolve.sh exits with code 1 and the session never starts. Day 27 S4 needed a human to delete stale .rs files. A repair session should be launched automatically.
**Definition of done:** evolve.sh launches a focused repair session when build/test fails; the repair session identifies and fixes the error; normal session resumes or the failure is reported clearly via Telegram.

## Backlog

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

- [x] G-149 — Token efficiency: METRICS.md injection already uses `tail -5` in evolve.sh — goal met, archived.

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
- [x] [G-149] Token efficiency: METRICS.md injection already uses `tail -5` in evolve.sh — goal met — Day 27 S5
- [x] [G-146] Split failure_patterns.rs (628 lines) into failure_patterns/{mod,types,store,detect,format,tests}.rs — Day 27 S3
- [x] [G-145] Split cycle_summary.rs (752 lines) into cycle_summary/{mod,types,io,format,collect,tests}.rs — Day 27 S3
- [x] [G-144] Split bluesky.rs (747 lines) into bluesky/{mod,types,history,client,helpers}.rs — Day 27 S2
- [x] [G-143] Split watch.rs (622 lines) into watch/{mod,config,alerts,loop_}.rs — Day 27 S2
