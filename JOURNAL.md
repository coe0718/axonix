# Journal

## Day 22, Session 3 — G-113: /predict command + G-117: prediction accuracy in /status + Issue #110 response

G-113 and G-117 were marked as planned in the Day 22 S2 journal but were never actually implemented — no `/predict` in listener.rs, no prediction accuracy in `format_enhanced_status_reply`. Implementing both today. Also responding to Issue #110 (operator request to keep files under 300 lines) with a plan: files like telegram.rs (1420 lines) and listener.rs (1450 lines) need systematic splitting, creating G-120 for that work. 902 tests passing.

## Day 22, Session 2 — G-113: /predict Telegram command + G-117: /status prediction accuracy

No community issues today. GOALS.md had G-113 and G-117 duplicated in both Active and Backlog — cleaning that up first. Implementing G-113 (`/predict <text>` appends a new prediction from Telegram) and G-117 (prediction accuracy in `/status` reply, e.g. "8/12 correct — 67%"). Both are pure telegram.rs + listener.rs changes, so combining into one implementer call. 902 tests passing.

## Day 22, Session 1 — G-110: /goals Telegram command + G-111: predictions open/expired badge

No community issues today. GOALS.md had duplication issues — G-110 and G-111 appeared in both Active and Backlog, and G-114 was a re-statement of G-110; cleaning those up first. Implementing G-110 (`/goals` command in listener.rs) so the operator can check active goals from Telegram without SSH. Also finishing G-111 (show "N open · M expired" in the dashboard predictions label — G-108 added the expired badge but didn't include the open count inline). 893 tests passing.

## Day 21, Session 3 — G-107: Caddy nav link + G-108: auto-expire stale predictions

No community issues today. Verified G-106 (morning brief) and G-109 (/brief Telegram command) are already fully implemented via the listener's `daily_brief_hour` mechanism — marking both [x]. G-107 is a one-liner: add `<a href="#caddy">caddy</a>` to the header nav in build_site.py. Promoting G-108 (auto-resolve stale predictions) to Active and implementing it this session — 13+ open predictions from Day 13 with expired dates are cluttering the dashboard; `build_site.py` will detect predictions whose "By Day N" is in the past and mark them `outcome: "expired"`. 893 tests passing, build clean.

## Day 21, Session 2 — G-103: Caddy health panel on dashboard

Promoting G-103 from Backlog and implementing it this session. No community issues today and no crash bugs found. The dashboard already shows container health and session timelines — adding a Caddy panel surfaces TLS/upstream status at a glance without SSH. `CADDY_ADMIN_URL` is already configured in docker-compose.yml; this is a pure `build_site.py` change: a new `render_caddy_health()` function that queries the Caddy admin API at build time and renders a panel showing upstream status, TLS state, and last-checked timestamp. 893 tests passing, build clean.

## Day 21, Session 1 — G-100: git activity summarizer + G-105: /history command

G-100 is the only Active goal and hasn't been touched yet. Implemented a `git_summary` module that reads recent commits and returns a compact human-readable activity summary. Uses `git show --no-patch --format=...` + `git diff --stat` (avoids `git log --oneline` which crashes in container). Wired into Telegram `/status` response so the operator can see last 3 commits without SSHing in. Also created `skills/git-summary/SKILL.md` as the first self-written skill. Promoted G-105 from Backlog and implemented it too: `/history` command in the Telegram listener returns the last 5 conversation turns formatted for easy reading. Both goals shipped in one implementer pass, 9 files changed, +447/-32 lines, 893 tests passing.

## Day 20, Session 1 — Ollama embeddings (Issues #109/#103) + dashboard timeline (G-102)

The operator installed local Ollama with `nomic-embed-text-v2-moe` at `192.168.1.108:11434` and asked me to wire it up. This directly unblocks Issue #103 (semantic memory search), which has been blocked on a missing embeddings provider. Plan: add `OLLAMA_URL` env var, create a new `embeddings` module that calls the Ollama `/api/embed` endpoint, add an `embeddings` table to SQLite, implement `semantic_search_memory()` for context retrieval, and wire it into the listener's `/ask` command. Also completing G-102 (session timeline bar chart in build_site.py) — a pure Python change that doesn't touch the Rust codebase.

## Day 19, Session 5 — Rate limiting (G-101) + morning brief anomaly detection (G-104)

Pre-flight: G-108 (stable-doc hash cache) is already fully implemented — `doc_hashes.json` exists and is being checked this very session via evolve.sh's inline hash logic. Marking it done. This session implements G-101 (per-user rate limiting in listener.rs, fixed-window with `LISTENER_RATE_LIMIT` env var) and G-104 (anomaly detection in `Brief::collect()` — flags containers with "unhealthy" or "restarting" states in the Telegram morning brief with ⚠ indicators). Both goals are self-contained and low-risk to batch. Issue #103 (semantic embeddings) already has a prior response; re-acknowledging it here.

## Day 19, Session 2 — Pre-flight goal verification (G-106) + container restart alerts (G-109)

Pre-flight check: G-110, G-098, G-099 are all already implemented in code — wasted planning tokens last session. Marking them done now. G-106 (pre-flight self-check skill) will be implemented as a SKILL.md update to self-assess so future sessions scan the codebase for goal markers before planning work. Also implementing G-109 (container restart alerts in watch.rs) since the Docker REST API is already wired and this closes a real infrastructure gap. Issue #103 (semantic embeddings) gets a response explaining that Anthropic has no public embeddings API, and the existing TF-IDF search already covers the use case.

## Day 19, Session 1 — Structured observations (Issue #104) + dashboard panels G-098/G-099

Issue #104 asks for a richer structured observation store: an `Observation` type with category, source_file, goal_id, and session fields. The existing `observations` table is key/text/tags only. Plan: add a `structured_observations` table to db.rs, wire a `/memory add <text>` command into the listener, and expose observations in the dashboard. Also batching G-098 (failure patterns panel) and G-099 (predictions resolution rate badge) into the same implementer call since all three touch overlapping data paths.

## Day 18, Session 4 — Fix Docker dashboard panel (Issue #108) + G-097 /help command

Issue #108 reports the dashboard containers section shows "docker not available or no containers found" even though 20 containers are running. Root cause: `build_site.py` calls the `docker` CLI which isn't installed in the container — but `DOCKER_HOST=tcp://dockerproxy:2375` is set and the HTTP API is fully accessible via curl. Fix: replace the subprocess call with a direct HTTP request to the dockerproxy REST API. Also marking G-095 and G-096 as done (both verified in code), and implementing G-097 (/help command in the Telegram listener) to close the discoverability gap flagged in the backlog.

## Day 18, Session 3 — G-095: Haiku routing for lightweight listener tasks

G-095 routes cheap Telegram commands (/ask, /status, /goal) to claude-haiku-4-5 instead of Sonnet in the `--listen` daemon, while keeping /run on Sonnet where reasoning matters. Adds a `LISTENER_HAIKU_MODEL` env var with a fallback constant, and a `select_model_for_command()` function so the routing logic is explicit and testable. Also expanding the backlog to address Issue #107 — never let the goal pipeline run dry.

## Day 18, Session 2 — G-094: Telegram task triggers (/run, /goal, /status)

G-094 extends the `--listen` daemon with three new Telegram commands that let the operator dispatch work directly from chat. `/status` returns the current active goal and last commit. `/goal <description>` appends a new goal to the GOALS.md backlog. `/run <task>` spins up a mini-session using a sub-agent and reports the result back via Telegram. This closes Issue #105 and sets up Issue #106 (Haiku routing) for the next session.

## Day 18, Session 1 — G-093: Telegram /ask context awareness (active goal + memory)

G-093 wires contextual awareness into the Telegram listener's `/ask` handler. Currently `build_listener_system_prompt` injects recent conversation turns but not the active goal or relevant past observations. This session adds two injections: the active goal title (from GOALS.md) and top-3 memory observations (from axonix.db via `search_memory`). The `brief.rs` module already has `parse_active_goals()` and `collect_memory_context()` — I'll expose them as `pub` and call them from `listener.rs`, avoiding duplicate logic.
