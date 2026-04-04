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

### G-113 — Telegram /predict command
**Why:** The operator should be able to add a new prediction from Telegram without SSH. `/predict <text>` appends to .axonix/predictions.json with today's date and null outcome.
**Definition of done:** New `/predict <text>` command in telegram.rs + listener.rs appends a prediction. Tests added.

### G-117 — /status shows prediction accuracy
**Why:** The `/status` Telegram command shows model, uptime, active goal, last commit — adding prediction accuracy (e.g. "8/12 correct — 67%") gives the operator a quick self-calibration signal at a glance.
**Definition of done:** `format_enhanced_status_reply()` in telegram.rs includes prediction accuracy from predictions.json. Tests updated.

## Backlog

### G-112 — Self-assessment: verify Active/Backlog counts at session start
**Why:** The goal hygiene rule (≥2 Active, ≥5 Backlog) keeps failing because there's no automated check at session start. Adding a check to Phase 1 self-assessment that explicitly counts and flags violations makes the rule self-enforcing.
**Definition of done:** During Phase 1, parse GOALS.md and print `[GOALS] Active: N, Backlog: M` — and print a warning if either is below minimum. Add this to LEARNINGS.md as a Phase 1 step.

### G-115 — Listener /resolve command
**Why:** Predictions accumulate unresolved. `/resolve <id> correct|wrong` from Telegram lets the operator close predictions on the go.
**Definition of done:** New `/resolve <id> correct|wrong` command in telegram.rs + listener.rs updates the prediction outcome in predictions.json. Tests added.

### G-116 — Dashboard: session timeline panel (last 5 sessions)
**Why:** The dashboard shows current state but no recent activity. A compact "last 5 sessions" panel from METRICS.md would surface what's been done without SSH.
**Definition of done:** `render_session_timeline()` in build_site.py reads the last 5 rows from METRICS.md and renders them as a compact timeline block.

### G-118 — Failure pattern Telegram alert
**Why:** Failure patterns accumulate silently. When a new pattern is recorded that has appeared 3+ times, send a Telegram alert so the operator notices.
**Definition of done:** In listener.rs or watch.rs, after writing a new failure pattern, check if any pattern has count ≥ 3 and hasn't been alerted yet. Send one Telegram message per new threshold breach.

### G-119 — Dashboard: show last build time
**Why:** The dashboard header has no indication of when the site was last built. Adding a "Last built: YYYY-MM-DD HH:MM" timestamp to the footer gives the operator instant confidence the data is fresh.
**Definition of done:** `build_site.py` injects the current UTC timestamp into the dashboard footer. Visible on the live site after next build.

### G-120 — Split oversized source files (Issue #110)
**Why:** The operator asked to keep all files under 300 lines. telegram.rs (1420), listener.rs (1450), brief.rs (3170), repl.rs (2459), main.rs (2092) all violate this. Large files are harder to read, review, and modify safely.
**Definition of done:** Split the two highest-offenders (telegram.rs and listener.rs) into logical sub-modules (e.g. `telegram/commands.rs`, `telegram/format.rs`, `listener/dispatch.rs`, `listener/handlers.rs`). All tests still pass.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-110] Telegram /goals command — Day 22 S1
- [x] [G-111] Dashboard: predictions open/expired count badge — Day 22 S1
- [x] [G-107] Dashboard: Caddy nav link in header — Day 21 S3
- [x] [G-108] Auto-expire stale predictions in build_site.py — Day 21 S3
- [x] [G-106] Morning brief via Telegram (listener delivers at daily_brief_hour=7) — Day 21 S3
