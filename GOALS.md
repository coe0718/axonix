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

### G-154 — Split journal_archive.rs (448 lines) into sub-modules
**Why:** journal_archive.rs at 448 lines is over the 300-line target and handles multiple concerns (parsing, archiving, file I/O).
**Definition of done:** journal_archive.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-155 — Split ssh.rs (486 lines) into sub-modules
**Why:** ssh.rs at 486 lines handles connection management, command execution, and device inventory — distinct concerns.
**Definition of done:** ssh.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

## Backlog

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

### G-156 — Split conversation_memory.rs (453 lines) into sub-modules
**Why:** conversation_memory.rs at 453 lines handles persistent turn-by-turn conversation log — separable into types, storage, and trim logic.
**Definition of done:** conversation_memory.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-157 — Split github.rs (451 lines) into sub-modules
**Why:** github.rs at 451 lines handles GitHub API integration, issue comments, and identity — distinct concerns.
**Definition of done:** github.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-158 — Split telegram.rs into sub-modules
**Why:** telegram.rs handles bot setup, polling, command dispatch, and message formatting — multiple separable concerns likely over 300 lines.
**Definition of done:** telegram.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

### G-159 — Split brief/mod.rs into smaller sub-modules if over 300 lines
**Why:** brief/ has multiple sub-modules but mod.rs may still be large. Ensure all brief sub-files stay ≤ 300 lines.
**Definition of done:** All files under src/brief/ are ≤ 300 lines. All tests pass.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-151] Split lint.rs (580 lines) into lint/{mod,yaml,caddy}.rs — Day 28 S2
- [x] [G-152] Split pogo.rs (505 lines) into pogo/{mod,types,fetch}.rs — Day 28 S2
- [x] [G-150] Split cli.rs (672 lines) into cli/{mod,args,help}.rs — Day 28 S1
- [x] [G-148] Split repl_loop.rs (719 lines) into sub-modules — Day 28 S1
- [x] [G-153] Repair mode in evolve.sh — auto-launch Claude session when build/test fails (Issue #114) — Day 27 S5
