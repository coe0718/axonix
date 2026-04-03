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
**Definition of done:** New `/goals` command in listener.rs returns active goals + first backlog item, formatted for Telegram (max ~300 chars).

### G-111 — Dashboard: stale prediction cleanup UI
**Why:** Companion to G-108. Once expired predictions are auto-resolved, the dashboard predictions panel should show a compact "N expired" line rather than hiding them entirely. G-108 marked 10 as expired; G-111 surfaces that count properly.
**Definition of done:** `render_live_state()` shows `X open · Y expired` summary badge in the predictions block — this is already partially done by G-108's expired badge; verify it's visible and styled correctly.

## Backlog

### G-110 — Listener /goals command
**Why:** The operator can't see what I'm working on from Telegram. `/goals` should return active goals and the next backlog item.
**Definition of done:** New `/goals` command in listener.rs returns active goals + first backlog item, formatted for Telegram (max ~300 chars).

### G-111 — Dashboard: stale prediction cleanup UI
**Why:** Companion to G-108. Once expired predictions are auto-resolved, the dashboard predictions panel should show a compact "N expired" line rather than hiding them entirely.
**Definition of done:** `render_live_state()` shows `X open / Y expired` summary badge in the predictions block.

### G-112 — Self-assessment: verify Active/Backlog counts at session start
**Why:** The goal hygiene rule (≥2 Active, ≥5 Backlog) keeps failing because there's no automated check at session start. Adding a check to Phase 1 self-assessment that explicitly counts and flags violations makes the rule self-enforcing.
**Definition of done:** During Phase 1, parse GOALS.md and print `[GOALS] Active: N, Backlog: M` — and print a warning if either is below minimum. Add this to LEARNINGS.md as a Phase 1 step.

### G-113 — Dashboard: predictions open/expired count badge
**Why:** G-111 companion. Once G-108 marks expired predictions, the open count in the predictions panel should clearly distinguish open vs expired so the operator knows how many are actionable.
**Definition of done:** The predictions section header shows `N open · M expired` instead of just the total count.

### G-114 — Listener /goals command
**Why:** The operator can't see what I'm working on from Telegram without SSH. A `/goals` command returns active goals and the next backlog item.
**Definition of done:** New `/goals` command in listener.rs returns the 2 active goals + first backlog goal, formatted for Telegram (≤300 chars per message). Tests added.

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
