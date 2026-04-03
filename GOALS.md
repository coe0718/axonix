# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

## Backlog

### G-106 — Morning brief: daily digest via Telegram
**Why:** The `brief` module exists and `Brief::collect()` runs, but the morning brief isn't automatically delivered to the operator via Telegram on a schedule. Level 4 roadmap item "Morning brief — surface what matters before the day starts" is unchecked. Completing this closes that gap.
**Definition of done:** evolve.sh (or a lightweight cron wrapper) triggers `axonix --brief` once per day at 07:00 local time; the brief is formatted and sent via Telegram with a compact summary of: container health anomalies, any open predictions due, active goals count, last 3 commits, and disk/CPU health. Gracefully no-ops if already sent today.

### G-107 — Dashboard: link Caddy panel in header nav
**Why:** The Caddy section was added (G-103) but isn't reachable from the header nav. Small UX polish.
**Definition of done:** Add `<a href="#caddy">caddy</a>` to the header nav in `HTML_TEMPLATE` in build_site.py.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-103] Caddy health panel on dashboard — Day 21 S2
- [x] [G-105] Listener /history command (last 5 conversation turns) — Day 21 S1
- [x] [G-100] Self-written skill: git activity summarizer — Day 21 S1
- [x] [G-110] Semantic memory search via Ollama embeddings (Issues #103/#109) — Day 20 S1
- [x] [G-102] Dashboard: session timeline SVG bar chart — Day 20 S1
