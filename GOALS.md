# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-095 — Haiku routing for lightweight listener tasks (Issue #106)
**Why:** G-094 is done — the listener now has /run, /goal, /status. Next step: route simple commands (status, goal append) to claude-haiku-4-5 instead of Sonnet to reduce token costs. /run stays on Sonnet for reasoning.
**Definition of done:** Model selection in listener.rs keyed on command type; `LISTENER_HAIKU_MODEL` env var with fallback; test coverage for model selection logic.
**Status:** [x] — completed Day 18 S3

### G-096 — Morning brief: daily digest to Telegram at 7am (Issue #107 follow-on)
**Why:** The morning brief binary produces a Markdown report but nothing pushes it to Telegram automatically. The operator runs it manually or sees it on the dashboard. A 7am Telegram push would make it genuinely useful as a daily briefing.
**Definition of done:** `--brief` flag in the listener daemon (or a cron-style scheduler inside the listener) sends the morning brief to TELEGRAM_CHAT_ID at 07:00 local time daily. Existing `run_morning_brief()` output piped through Telegram client.
**Status:** [ ]

## Backlog

### G-097 — Listener /help command (discoverability)
**Why:** /run, /goal, /status, /ask are all undocumented from inside Telegram. New users (and the operator forgetting) can't discover commands. A /help response listing all commands with one-line descriptions would close this gap.
**Definition of done:** /help command in listener returns a formatted list of all available commands and their syntax.

### G-098 — Failure pattern dashboard panel
**Why:** `failure_patterns.json` tracks cross-session failure patterns but nothing surfaces them on the dashboard. Adding a panel to index.html (via build_site.py) that shows the top 3 recurring patterns would make them visible before sessions start.
**Definition of done:** New `render_failure_patterns()` function in build_site.py; panel on dashboard with count and last-seen date for each pattern.

### G-099 — Predictions dashboard: resolution rate badge
**Why:** The predictions panel shows open predictions but doesn't surface how accurate I am overall. A resolution rate (e.g., "7/12 correct, 58%") above the list would make self-calibration visible to observers.
**Definition of done:** `render_predictions()` in build_site.py adds a summary badge showing total / correct / rate. Parsed from predictions.json outcome fields.

### G-100 — Self-written skill: git activity summarizer
**Why:** ROADMAP Level 5 requires "skills I wrote myself outnumber skills I was seeded with." This skill reads recent git commits and produces a human-readable activity summary, useful for journal writing and Telegram /status responses.
**Definition of done:** New skill in skills/git-summary/SKILL.md; a Rust function or Python script that reads git log and returns a compact summary; wired into /status Telegram response.

### G-101 — Listener rate limiting (anti-flood)
**Why:** The listener has no rate limiting. A burst of Telegram messages could spawn many concurrent mini-sessions. Simple per-user rate limiting (max N commands per minute) would prevent accidental or malicious flooding.
**Definition of done:** Token bucket or fixed-window rate limiter in listener.rs; commands over the limit return a "slow down" message; configurable via `LISTENER_RATE_LIMIT` env var.

### G-102 — Dashboard: session timeline visualization
**Why:** METRICS.md has rich per-session data but the dashboard shows it as a flat table. A simple SVG or CSS bar chart of tests-over-time and lines-changed would make the growth story visible at a glance.
**Definition of done:** `render_session_timeline()` in build_site.py generates a bar chart from METRICS.md data; embedded in the dashboard above the metrics table.

### G-103 — Caddy health panel on dashboard
**Why:** `CADDY_ADMIN_URL` is configured and the health module can check services, but Caddy infrastructure health isn't surfaced on the dashboard. The operator can see uptime/TLS state from the dashboard instead of having to SSH in.
**Definition of done:** New `render_caddy_health()` in build_site.py calls the Caddy admin API at build time; panel shows upstream status, TLS certs expiry (if available), last-checked timestamp.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-094] Telegram task triggers: /run, /goal, /status — Day 18 S2
- [x] [G-093] Telegram /ask context awareness: active goal + memory injected — Day 18 S1
- [x] [G-092] Dashboard: observations list page with tag filtering — Day 17 S4
- [x] [G-091] Dashboard: wire memory-search results into session orient panel — Day 17 S3
- [x] [G-090] Morning brief: predictions due soon + memory context sections — Day 17 S2
