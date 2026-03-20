---
name: Machine
description: How to navigate this specific machine — Docker setup, known gotchas, and environment facts specific to the NUC running Axonix.
---

# Machine

I run on an Intel NUC home server managed by coe0718. This skill documents
everything I have learned about this specific environment so I do not have to
rediscover it each session.

---

## Environment

**Container:** `axonix` inside Docker Compose on the NUC.
**Working directory:** `/workspace` (bind-mounted from the host).
**Timezone:** `America/Indiana/Indianapolis` (from TZ env var).
**Cron cadence:** Sessions are triggered by cron every 4 hours via `scripts/evolve.sh`.
**My identity inside Git:** `axonix-bot` (configured by `configure_git_identity()` on startup — only inside Docker).

---

## Docker Compose Services

| Service        | Purpose                                           | Port  |
|---------------|---------------------------------------------------|-------|
| `axonix`       | Me — the agent. Started by cron via evolve.sh    | none  |
| `axonix-stream`| SSE stream server for live session output         | 7040  |
| `dockerproxy`  | Docker socket proxy for safe container ops        | 2375  |

All three run with `restart: always`. The stream server starts automatically — no
operator action needed. Sessions are visible live at `stream.axonix.live`.

**Profile note:** The `axonix` service uses `profiles: [session]` — it does NOT start
automatically with `docker compose up`. It runs only when explicitly invoked via
`docker compose run --rm --profile session axonix ...` (which evolve.sh does).

---

## Known Gotchas

### git log crashes
`git log` with certain flags segfaults inside the container. Safe alternatives:
- `git show HEAD` — see the most recent commit
- `git diff --stat HEAD~N HEAD` — diff between commits
- `git show --stat HEAD` — files changed in last commit
- Read `JOURNAL.md` for history instead of git log

### evolve.sh is read-only
`scripts/evolve.sh` is mounted `:ro` inside the container. Cannot modify it from a session.
To propose changes: write to `EVOLVE_PROPOSED.md` and the operator will review and apply.
Never claim credit for evolve.sh changes until the operator has applied them.

### configure_git_identity guard
`configure_git_identity()` only runs inside Docker (detected by `/.dockerenv`).
This prevents polluting the operator's host git config with the axonix-bot identity.

### Token format
`ANTHROPIC_API_KEY` starts with `sk-ant-oat01-` (OAuth Access Token from Claude Pro,
not a standard API key). Billing goes against the Pro subscription.

---

## Environment Variables

Every variable must be explicitly listed in `docker-compose.yml environment:` section —
not just in `.env`. When adding a new key, update all three:
1. `.env` (real values, gitignored)
2. `.env.example` (placeholder values, committed)
3. `docker-compose.yml` environment section

Current active variables: see `CAPABILITIES.md`.

---

## The Persistent Store

My state lives in `.axonix/` (bind-mounted, persists across sessions):
- `memory.json` — key-value facts (G-019)
- `predictions.json` — open + resolved predictions (G-021)
- `cycle_summary.json` — compact last-session summary (Issue #38)
- `discussed_*` — marker files tracking which journal entries have been posted as Discussions

---

## GitHub Identity

Two tokens, two purposes:
- `GH_TOKEN` — owner's token. Used by `gh` CLI for repo operations (push, fetch issues, etc.)
- `AXONIX_BOT_TOKEN` — axonix-bot account token. Used for posting issue comments and
  closing issues via the GitHub REST API (not `gh` CLI).

Rule: **Always use `AXONIX_BOT_TOKEN` for issue comments** so they appear as axonix-bot,
not as the repo owner. Never use `gh issue comment` for responses.

---

## Twitter Status

Twitter write API (POST /2/tweets) requires tokens generated **after** setting app
permissions to "Read and Write" in the Twitter Developer Portal. Tokens bake in their
permission scope at generation time. The 402 error I hit early was a credentials issue,
not a billing issue. If write ops fail, check token generation order.

---

## Bluesky

Free-tier social posting. No write restrictions. Use for session announcements when
Twitter is unavailable. Credentials: `BLUESKY_IDENTIFIER`, `BLUESKY_APP_PASSWORD`.
AT Protocol authentication via `src/bluesky.rs`.

---

## Dashboard

- Public URL: `axonix.live`
- Stream URL: `stream.axonix.live`
- Built from: `docs/index.html` + `scripts/build_site.py` (reads METRICS.md, GOALS.md, JOURNAL.md)
- Rebuilt by: evolve.sh after each session via `python3 scripts/build_site.py`

To verify dashboard data is correct: check METRICS.md for any `~?k` token rows
(the build script skips unparseable values gracefully since Day 6 S3).

---

## Self-Assessment Checklist (run every session)

1. `cargo build && cargo test 2>&1 | grep "^test result"` — confirm clean build
2. Check `docker-compose.yml environment:` vs `CAPABILITIES.md` — all keys present?
3. Check `GOALS.md` — any [x] entries still in Active? Any duplicates?
4. Check `ISSUES_TODAY.md` — any unanswered community issues?
5. Check `.axonix/cycle_summary.json` — was last session summarized?

---

*Written by Axonix — Day 7 Session 6 (2026-03-20)*
*Self-authored. Not seeded by the operator.*
