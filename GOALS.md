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

### G-110 — Listener /goals command
**Why:** The operator can't see what I'm working on from Telegram. `/goals` should return active goals and the next backlog item.
**Definition of done:** New `/goals` command in listener.rs + telegram.rs returns active goals + first backlog item, formatted for Telegram (max ~300 chars). Tests added.

### G-111 — Dashboard: predictions open/expired count badge
**Why:** G-108 added an expired count badge but the label still says "◈ open predictions [N expired]" without showing how many are open. The definition of done requires "N open · M expired" inline.
**Definition of done:** `render_live_state()` label shows `N open · M expired` when expired > 0, or just `N open` when none expired. build_site.py change only.

## Backlog

### G-112 — Self-assessment: verify Active/Backlog counts at session start
**Why:** The goal hygiene rule (≥2 Active, ≥5 Backlog) keeps failing because there's no automated check at session start. Adding a check to Phase 1 self-assessment that explicitly counts and flags violations makes the rule self-enforcing.
**Definition of done:** During Phase 1, parse GOALS.md and print `[GOALS] Active: N, Backlog: M` — and print a warning if either is below minimum. Add this to LEARNINGS.md as a Phase 1 step.

### G-113 — Telegram /predict command
**Why:** The operator should be able to add a new prediction from Telegram without SSH. `/predict <text>` should append to .axonix/predictions.json.
**Definition of done:** New `/predict <text>` command in telegram.rs + listener.rs appends a prediction with today's date, null outcome. Tests added.

### G-115 — Listener /resolve command
**Why:** Predictions accumulate unresolved. `/resolve <id> correct|wrong` from Telegram lets the operator close predictions on the go.
**Definition of done:** New `/resolve <id> correct|wrong` command in telegram.rs + listener.rs updates the prediction outcome in predictions.json. Tests added.

### G-116 — Dashboard: session timeline panel (last 5 sessions)
**Why:** The dashboard shows current state but no recent activity. A compact "last 5 sessions" panel from METRICS.md would surface what's been done without SSH.
**Definition of done:** `render_session_timeline()` in build_site.py reads the last 5 rows from METRICS.md and renders them as a compact timeline block.

### G-117 — /status shows prediction accuracy
**Why:** The `/status` Telegram command currently shows model, uptime, active goal, last commit. Adding prediction accuracy (e.g. "8/12 correct — 67%") gives the operator a quick self-calibration signal.
**Definition of done:** `format_enhanced_status_reply()` in telegram.rs includes prediction accuracy from predictions.json. Tests updated.

### G-118 — Failure pattern Telegram alert
**Why:** Failure patterns accumulate silently. When a new pattern is recorded that has appeared 3+ times, send a Telegram alert so the operator notices.
**Definition of done:** In listener.rs or watch.rs, after writing a new failure pattern, check if any pattern has count ≥ 3 and hasn't been alerted yet. Send one Telegram message per new threshold breach.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-107] Dashboard: Caddy nav link in header — Day 21 S3
- [x] [G-108] Auto-expire stale predictions in build_site.py — Day 21 S3
- [x] [G-106] Morning brief via Telegram (listener delivers at daily_brief_hour=7) — Day 21 S3
- [x] [G-109] Telegram /brief command (listener.rs line 673) — Day 21 S3
- [x] [G-103] Caddy health panel on dashboard — Day 21 S2
