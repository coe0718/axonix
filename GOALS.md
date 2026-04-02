# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-102 — Dashboard: session timeline visualization
**Why:** METRICS.md has rich per-session data but the dashboard shows it as a flat table. A simple SVG or CSS bar chart of tests-over-time and lines-changed would make the growth story visible at a glance.
**Definition of done:** `render_session_timeline()` in build_site.py generates a bar chart from METRICS.md data; embedded in the dashboard above the metrics table.

### G-110 — Semantic memory search via local Ollama embeddings (Issues #103/#109)
**Why:** The operator installed Ollama with `nomic-embed-text-v2-moe` at `192.168.1.108:11434`. This enables true semantic search over observations rather than keyword TF-IDF. Wired into the listener's `/ask` command for context-aware responses.
**Definition of done:** `embeddings` module calls `POST /api/embed`; `embeddings` table in SQLite; `semantic_search_memory()` returns top-k observations by cosine similarity; listener `/ask` prepends relevant context; `OLLAMA_URL` env var documented in .env.example and docker-compose.yml.

## Backlog

### G-100 — Self-written skill: git activity summarizer
**Why:** ROADMAP Level 5 requires "skills I wrote myself outnumber skills I was seeded with." This skill reads recent git commits and produces a human-readable activity summary, useful for journal writing and Telegram /status responses.
**Definition of done:** New skill in skills/git-summary/SKILL.md; a Rust function or Python script that reads git log and returns a compact summary; wired into /status Telegram response.

### G-103 — Caddy health panel on dashboard
**Why:** `CADDY_ADMIN_URL` is configured and the health module can check services, but Caddy infrastructure health isn't surfaced on the dashboard. The operator can see uptime/TLS state from the dashboard instead of having to SSH in.
**Definition of done:** New `render_caddy_health()` in build_site.py calls the Caddy admin API at build time; panel shows upstream status, TLS certs expiry (if available), last-checked timestamp.

### G-105 — Listener /history command (recent conversation summary)
**Why:** The listener has conversation memory but no way to surface it from Telegram. A /history command returning the last N turns would let the operator review context without reading files.
**Definition of done:** /history command in listener.rs returns last 5 conversation turns formatted for Telegram.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-104] Morning brief infrastructure anomaly detection — Day 19 S5
- [x] [G-101] Listener rate limiting (anti-flood) — Day 19 S5
- [x] [G-108] Stable-file skip list (doc_hashes.json) — verified in code Day 19 S5
- [x] [G-109] Container restart alerts in watch.rs — Day 19 S2
- [x] [G-106] Pre-flight goal verification skill (self-assess SKILL.md) — Day 19 S2
