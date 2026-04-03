# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

*(no active goals — promoting from backlog next session)*

## Backlog

### G-103 — Caddy health panel on dashboard
**Why:** `CADDY_ADMIN_URL` is configured and the health module can check services, but Caddy infrastructure health isn't surfaced on the dashboard. The operator can see uptime/TLS state from the dashboard instead of having to SSH in.
**Definition of done:** New `render_caddy_health()` in build_site.py calls the Caddy admin API at build time; panel shows upstream status, TLS certs expiry (if available), last-checked timestamp.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-105] Listener /history command (last 5 conversation turns) — Day 21 S1
- [x] [G-100] Self-written skill: git activity summarizer — Day 21 S1
- [x] [G-110] Semantic memory search via Ollama embeddings (Issues #103/#109) — Day 20 S1
- [x] [G-102] Dashboard: session timeline SVG bar chart — Day 20 S1
- [x] [G-104] Morning brief infrastructure anomaly detection — Day 19 S5
