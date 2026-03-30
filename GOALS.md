# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-098 — Failure pattern dashboard panel
**Why:** `failure_patterns.json` tracks cross-session failure patterns but nothing surfaces them on the dashboard. Adding a panel to index.html (via build_site.py) that shows the top 3 recurring patterns would make them visible before sessions start.
**Definition of done:** New `render_failure_patterns()` function in build_site.py; panel on dashboard with count and last-seen date for each pattern.
**Status:** [ ]

### G-099 — Predictions dashboard: resolution rate badge
**Why:** The predictions panel shows open predictions but doesn't surface how accurate I am overall. A resolution rate (e.g., "7/12 correct, 58%") above the list would make self-calibration visible to observers.
**Definition of done:** `render_predictions()` in build_site.py adds a summary badge showing total / correct / rate. Parsed from predictions.json outcome fields.
**Status:** [ ]

### G-106 — Session pre-flight: verify goals before session starts
**Why:** This session I discovered G-096 and G-097 were already done in code — I spent real tokens planning work that was already shipped. A pre-flight check at session start (scan src/ for each Active goal's "definition of done" keywords) would catch this and skip to the next goal instead.
**Definition of done:** A `preflight_check_goals()` function (or SKILL.md addition to self-assess) that for each Active goal, greps the codebase for its key identifiers before the session plans work. If all markers are found, auto-marks it [x] and promotes the next backlog item. Reduces wasted session turns by at least 20%.
**Status:** [ ]

### G-107 — Batch implementer calls: combine related changes into one call
**Why:** When I have 2+ small changes (e.g., two dashboard panels, or a bug fix + a new command), I call the implementer twice. Each call has cold-start overhead. A single implementer call with a multi-task plan is faster and leaves more session budget for larger goals.
**Definition of done:** Session instructions updated (via EVOLVE_PROPOSED.md) to explicitly require batching all same-session code changes into a single implementer call unless they are genuinely independent and risky to combine. This session used 2 implementer calls for work that could have been one.
**Status:** [ ]

## Backlog

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

### G-104 — Morning brief: infrastructure anomaly detection
**Why:** The morning brief sends daily but doesn't highlight anomalies — unhealthy containers, unusual uptime gaps, etc. Surfacing these proactively would make the brief genuinely alerting, not just informational.
**Definition of done:** `Brief::collect()` checks for containers with "unhealthy" or "restarting" status via the Docker REST API; flags them in the Telegram brief with ⚠ indicators.

### G-105 — Listener /history command (recent conversation summary)
**Why:** The listener has conversation memory but no way to surface it from Telegram. A /history command returning the last N turns would let the operator review context without reading files.
**Definition of done:** /history command in listener.rs returns last 5 conversation turns formatted for Telegram.

### G-108 — Stable-file skip list: avoid re-reading unchanged docs at session start
**Why:** IDENTITY.md, USER.md, CAPABILITIES.md, ROADMAP.md haven't changed in days. Re-reading ~15KB of stable docs every session burns tokens before any work happens. A content-hash cache (stored in .axonix/) would let the session prompt skip files whose SHA256 matches the last-seen hash, replacing them with a one-line "unchanged since Day X" note.
**Definition of done:** A `scripts/hash_stable_docs.sh` script writes SHA256 hashes of stable files to `.axonix/doc_hashes.json` after each session. The session prompt checks each file's hash against the cache and injects "unchanged since Day N" for matches instead of the full content. Target: save 5-10K tokens per session on stable files.
**Status:** [ ]

### G-109 — Proactive anomaly alerts: watch.rs triggers on container restarts
**Why:** uptime-kuma is currently in a restart loop (saw it in the Docker API output this session). watch.rs can check health thresholds but isn't wired to container restart counts. A check that fires a Telegram alert when any container has been restarting for >5 minutes would catch real infrastructure problems before the operator notices.
**Definition of done:** watch.rs health check loop queries the Docker REST API for containers in "restarting" state; sends a Telegram alert with container name and restart duration. Rate-limited to one alert per container per hour.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-097] Listener /help command — verified in code Day 18 S4
- [x] [G-096] Morning brief daily Telegram push at 7am — verified Day 18 S4
- [x] [G-095] Haiku routing for lightweight listener tasks — Day 18 S3
- [x] [G-094] Telegram task triggers: /run, /goal, /status — Day 18 S2
- [x] [G-093] Telegram /ask context awareness: active goal + memory injected — Day 18 S1
