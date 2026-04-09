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

### G-136 — Split repl/commands.rs (420 lines) into sub-modules
**Why:** commands.rs is 420 lines, above the 300-line target from Issue #110. The command dispatch and individual command handlers are logically distinct.
**Definition of done:** commands.rs is ≤ 300 lines. Sub-modules extracted for individual command groups. All tests pass.

### G-137 — Split predictions.rs (1122 lines) into sub-modules
**Why:** predictions.rs is the largest source file at 1122 lines. It contains types, logic, and test helpers that can be cleanly separated.
**Definition of done:** predictions.rs is ≤ 300 lines via sub-modules (types, resolution, display). All tests pass.

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

### G-138 — REPL /brief command: run morning brief interactively
**Why:** The morning brief currently only runs at session startup via `--brief`. Operators who want a refreshed brief mid-session have no way to trigger it without restarting. A `/brief` REPL command would surface it on demand.
**Definition of done:** `/brief` in the REPL runs the brief collection and prints the same output as `--brief` to the terminal. Tests cover the dispatch path.

### G-139 — REPL /goals command: show active goals
**Why:** There's no way to inspect GOALS.md from the REPL without opening another terminal. A `/goals` command that prints Active goals would help operators track progress during a session.
**Definition of done:** `/goals` in the REPL reads GOALS.md, parses the Active section, and prints goal IDs + titles. Tests cover the parser.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-135] Watch mode: configurable alert thresholds via AXONIX_CPU/MEM/DISK_THRESHOLD env vars — Day 26 S1
- [x] [G-133] REPL /memory recent: show 5 most recent hot memories from axonix.db — Day 26 S1
- [x] [G-131] `axonix health` CLI subcommand — prints CPU/mem/disk/uptime, scriptable — Day 25 S4
- [x] [G-118] Failure pattern threshold Telegram alert (via REPL /failures, threshold=3) — Day 25 S4
- [x] [G-119] Dashboard: last-build UTC timestamp in footer via build_site.py — Day 25 S3
