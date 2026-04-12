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

### G-161 — Post-split: audit all src/ files for remaining violations
**Why:** After many splits, verify no files crept back over 300 lines (including mod.rs files in newly split directories). The tests.rs files (brief/tests.rs at 2184 lines, repl/tests.rs at 1558, cli/tests.rs at 459, listener/tests.rs at 680) are exempt from the split rule.
**Current violations (Day 29 S3 audit, post G-165):** main.rs (564), predictions/store.rs (382), brief/format.rs (358), health/docker.rs (350), meta_health.rs (324), repl/commands.rs (315), telegram/client.rs (313), memory/store.rs (311), repl_loop/input_loop.rs (304), agent_setup.rs (301).
**Definition of done:** All non-test `.rs` files under `src/` are ≤ 300 lines. Document any remaining violations and plan splits.

### G-165 — Split repl_loop/cmd_dispatch.rs (390 lines) into sub-modules ✓
**Why:** cmd_dispatch.rs at 390 lines is the next largest non-exempt violation. It has two clear logical sections: the marker-based async dispatcher and the inline /status//context//tokens handlers.
**Definition of done:** All files under src/repl_loop/ are ≤ 300 lines. All tests pass.
**Status: DONE (Day 29 S3)** — split into handled_result.rs (246) + inline_handlers.rs (153).

## Backlog

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

### G-159 — Split brief/mod.rs into smaller sub-modules if over 300 lines
**Why:** brief/ has multiple sub-modules but mod.rs may still be large. Ensure all brief sub-files stay ≤ 300 lines.
**Definition of done:** All files under src/brief/ are ≤ 300 lines. All tests pass.

### G-162 — Improve /status command with last-commit info
**Why:** The /status command shows health metrics but doesn't show what was last worked on. Adding last git commit message + timestamp would make it more informative at a glance.
**Definition of done:** /status output includes last commit hash, message, and relative time. Tests pass.



## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] G-165 — Split repl_loop/cmd_dispatch.rs (390→8 lines) into handled_result + inline_handlers — Day 29 S3
- [x] G-164 — Split listener/run.rs (448→162 lines via dispatch.rs + proactive.rs) — Day 29 S2
- [x] [G-163] Split cli/mod.rs (465→8 lines) into sub-modules — Day 29 S2
- [x] [G-164] Split listener/run.rs (448→162 lines) into sub-modules — Day 29 S2
- [x] [G-160] Add /files REPL command to list oversized source files — Day 29 S1
