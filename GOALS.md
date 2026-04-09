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

### G-141 — REPL /help: show command categories grouped
**Why:** /help output is a flat wall of text. Grouping by category (navigation, memory, predictions, SSH, GitHub) would make it more scannable.
**Definition of done:** /help output uses category headers. Same commands, just visually grouped.

### G-142 — Split health.rs (945 lines) into sub-modules
**Why:** health.rs is the largest non-test file at 945 lines, well over the 300-line target from Issue #110. CPU, memory, disk, and uptime collection are logically distinct from formatting and the health report struct.
**Definition of done:** health.rs is split into health/{mod,cpu,memory,disk,uptime,format}.rs with each file ≤ 300 lines. All tests pass.

## Backlog

### G-132 — Archive completed goals more aggressively
**Why:** The "Last 5 completed" window at the bottom of GOALS.md is manual and error-prone. Goals older than the last 5 should stay in GOALS_ARCHIVE.md and not bloat GOALS.md across sessions.
**Definition of done:** GOALS.md rolling window stays at exactly 5 completed entries. Any goal verification during Phase 1 checks GOALS_ARCHIVE.md for older completions.

### G-143 — Split watch.rs (622 lines) into sub-modules
**Why:** watch.rs at 622 lines combines alert threshold logic, the watch loop, Telegram notification, and formatting — four distinct concerns.
**Definition of done:** watch.rs split into watch/{mod,thresholds,alerts,loop}.rs with each file ≤ 300 lines. All tests pass.

### G-144 — Split bluesky.rs (747 lines) into sub-modules
**Why:** bluesky.rs at 747 lines is over the 300-line target and combines auth, posting, session management, and history tracking in one file.
**Definition of done:** bluesky.rs split into bluesky/{mod,auth,post,history}.rs with each file ≤ 300 lines. All tests pass.

### G-145 — Split cycle_summary.rs (752 lines) into sub-modules
**Why:** cycle_summary.rs at 752 lines is well over the 300-line target.
**Definition of done:** cycle_summary.rs split into sub-modules with each file ≤ 300 lines. All tests pass.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-140] Split predictions/store.rs (382→275 lines) into store.rs + calibration.rs — Day 26 S4
- [x] [G-139] REPL /goals command — show active goals from GOALS.md — Day 26 S4
- [x] [G-138] REPL /brief command: runs morning brief interactively mid-session — Day 26 S3
- [x] [G-134] REPL /search <query>: semantic similarity search over embeddings store, top-5 results — Day 26 S3
- [x] [G-137] Split predictions.rs (1122 lines) into predictions/{mod,types,helpers,store,tests}.rs — Day 26 S2
