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

### G-133 — REPL /memory command: show recent memories
**Why:** The REPL has no way to inspect what's stored in semantic memory. A `/memory` command that shows the top-N most recently stored memory entries would let the operator verify that memory extraction is working correctly after each session.
**Definition of done:** `/memory` in the REPL prints the 5 most recent entries from the axonix.db memories table with their source and timestamp. Tests cover the formatter.

### G-135 — Watch mode: configurable alert thresholds via env vars
**Why:** CPU/memory/disk thresholds are hardcoded in watch.rs. Operators with different hardware (e.g. RAM-constrained devices) need to tune them without recompiling.
**Definition of done:** watch.rs reads `AXONIX_CPU_THRESHOLD`, `AXONIX_MEM_THRESHOLD`, `AXONIX_DISK_THRESHOLD` from env with sensible defaults. Documented in CAPABILITIES.md.

## Backlog

### G-130 — Token efficiency: truncate METRICS.md injection to last 15 rows
**Why:** Issue #111 identified that evolve.sh injects full METRICS.md (14.7 KB, 105+ rows). Only recent sessions matter for planning. Expected saving: 5–8k tokens/session.
**Definition of done:** EVOLVE_PROPOSED.md contains a proposal to truncate METRICS.md injection. Operator can apply it. This is not implementable from inside the container since evolve.sh is :ro mounted.

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

### G-134 — Semantic search REPL command
**Why:** Embeddings are stored but never queried interactively. A `/search <query>` command in the REPL would let the operator find relevant past conversations or journal entries by meaning, not just keyword.
**Definition of done:** `/search <query>` in the REPL returns the top-5 semantically similar results from the embeddings store with source labels.

### G-136 — Split repl_loop.rs (678 lines) into sub-modules
**Why:** repl_loop.rs is 678 lines, above the 300-line target from Issue #110. The main loop, command dispatch, and output formatting are logically distinct.
**Definition of done:** repl_loop.rs is ≤ 300 lines. Sub-modules extracted for loop logic, output formatting, or command handling. All tests pass.

### G-137 — Split predictions.rs (1122 lines) into sub-modules
**Why:** predictions.rs is the largest source file at 1122 lines. It contains types, logic, and test helpers that can be cleanly separated.
**Definition of done:** predictions.rs is ≤ 300 lines via sub-modules (types, resolution, display). All tests pass.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-131] `axonix health` CLI subcommand — prints CPU/mem/disk/uptime, scriptable — Day 25 S4
- [x] [G-118] Failure pattern threshold Telegram alert (via REPL /failures, threshold=3) — Day 25 S4
- [x] [G-119] Dashboard: last-build UTC timestamp in footer via build_site.py — Day 25 S3
- [x] [G-123] Split main.rs into sub-modules — main() is 309 lines, dispatch in cli_dispatch/repl_loop/prompt_dispatch — Day 25 S2-S3
- [x] [G-112] Self-assessment goal count check — automated check in LEARNINGS.md, runs every Phase 1 — Day 25 S3
