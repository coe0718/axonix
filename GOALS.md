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

### G-121 — Split repl.rs into sub-modules
**Why:** repl.rs (2459 lines) is still a large file violating Issue #110's 300-line guideline. brief.rs was split in Day 24 S1. Now repl.rs needs the same treatment.
**Definition of done:** repl.rs split into logical sub-modules. All tests pass.

### G-126 — LEARNINGS.md staleness pruning (continued)
**Why:** LEARNINGS.md still has sections dating back to Day 11. The Day 2 bottleneck and OpenClaw sections were removed this session, but the Infrastructure Knowledge section should be audited — some items (e.g. "stream_server is already running — G-004") reference resolved goals. Issue #111.
**Definition of done:** LEARNINGS.md audited for stale goal references; outdated items removed or updated with a "resolved" note. File stays under 5KB.

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

### G-128 — Issue #112 remaining: brief.rs meta-health terminal display cleanup
**Why:** Issue #112 noted that meta-system warnings (stale ~?k tokens) don't belong in the morning brief. The Telegram fix was done in Day 23 S4. The terminal brief still shows a full `🔧 META-SYSTEM` section with all 3 health checks regardless of status. It should only show the section if there are non-metrics warnings.
**Definition of done:** Terminal brief's META-SYSTEM section omits the METRICS.md stale-token warning (same filter as Telegram). Shows nothing if all remaining checks are OK. Issue #112 fully closed.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-121] Split brief.rs into sub-modules (src/brief/) — Day 24 S1
- [x] [G-127] Prediction auto-resolve: auto_resolve_from_goals() + resolve 6 open predictions — Day 23 S4
- [x] [G-125] Token efficiency: LEARNINGS.md pruned + EVOLVE_PROPOSED.md written — Day 23 S4
- [x] [G-122] /predictions Telegram command — Day 23 S3 (verified already implemented + tests passing)
- [x] [G-120] Split telegram.rs + listener.rs into sub-modules — Day 23 S2
