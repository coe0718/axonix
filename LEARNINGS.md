# Learnings

<!-- Knowledge cached from sessions. Never search for the same thing twice. -->

## Bottleneck Analysis (G-002) — Day 2, Session 3

### Data from METRICS.md (3 sessions)

| Session | Tokens | Tests | Lines Changed | Notes |
|---------|--------|-------|---------------|-------|
| Day 1 S1 | ~30k | 23→40 | +206/-26 | First boot, basic features |
| Day 2 S1 | ~50k | 40→41 | +130/-6 | Six bug fixes |
| Day 2 S2 (mod) | ~40k | 41→50 | +533/-420 | Modular refactor |

### Identified Bottlenecks

**#1 — No live feedback during long operations**
When I run `cargo build` or `cargo test` inside a session, I get no output until the command finishes. If the build takes 10s, there's a 10s silence. This is fine for fast builds but will become painful as the codebase grows. Future fix: stream subprocess output in real time.

**#2 — Cost module uses hardcoded prices that drift**
`src/cost.rs` has Anthropic pricing hardcoded (opus: $15/$75 per M tokens). These prices change. I'll get the wrong estimates after any price change without noticing. Future fix: add a last-updated date comment and a note to verify prices each major session.

**#3 — No test coverage for the REPL loop itself**
The interactive REPL loop in `main.rs` has 0 integration tests — only unit tests for pure functions extracted from it. I can't test `/lint`, `/save`, `/clear` end-to-end without a full agent startup. Future fix: extract REPL state into a `ReplState` struct that can be tested without I/O.

**#4 — Skills system is opaque**
`SkillSet::load()` silently degrades to empty on failure (by design, for resilience). But I have no visibility into which skills loaded, which failed, or why. The `/status` command shows "N skills loaded" but no details. Future fix: `/skills` command that lists loaded skill names and source paths.

### Proposed Next Actions (priority order)
1. ReplState struct extraction (unblocks integration tests — high compound value)
2. `/skills` command (quick win, real UX improvement)
3. Price update timestamp in cost.rs (low effort, prevents silent drift)
4. Streaming subprocess output (bigger change, lower priority for now)

---

## Infrastructure Knowledge — seeded by operator

### Docker Compose env vars
Environment variables are NOT automatically available inside the container just because
they're in `.env`. Every variable must be explicitly listed in the `environment:` section
of `docker-compose.yml`. When adding a new env var, always update all three:
1. `.env` (actual values, gitignored)
2. `.env.example` (placeholder values, committed)
3. `docker-compose.yml` environment section (or it never reaches the container)
4. `CAPABILITIES.md` (so you know it's available)

### OAuth token format
`ANTHROPIC_API_KEY` starts with `sk-ant-oat01-` — this is an OAuth Access Token from
Claude Pro, not a standard API key. It authenticates against the Claude.ai session,
not the Anthropic API. This is why billing goes against the Pro subscription, not
per-token API credits.

### DAY_COUNT format
`DAY_COUNT` contains `"N YYYY-MM-DD"` — two space-separated fields.
- Field 1: integer day number
- Field 2: the date of the last session
Parse with: `awk '{print $1}'` for day, `awk '{print $2}'` for date.
The day increments when the date changes. Multiple sessions on the same day share the
same day number but increment `SESSION_COUNT`.

### evolve.sh is read-only inside the container
`scripts/evolve.sh` is bind-mounted as `:ro` inside the axonix container. You cannot
modify it from inside a session. To propose changes, write to `EVOLVE_PROPOSED.md`
and the operator will review and apply them manually.

### git log crashes in the container
`git log` with certain flags (e.g. `--oneline`, with pager) segfaults inside the
container. Use `git show`, `git diff`, `git status`, `git add`, and `git commit` freely —
they all work. Work around `git log` by using `git show HEAD` or reading JOURNAL.md.

### Twitter API authentication
Twitter credentials use OAuth 1.0a (User Context), not Bearer Token, for write operations.
The Bearer Token (`TWITTER_BEARER_TOKEN`) is for read-only endpoints only.
For posting tweets, use all four: `TWITTER_API_KEY`, `TWITTER_API_SECRET`,
`TWITTER_ACCESS_TOKEN`, `TWITTER_ACCESS_SECRET`.
The Twitter account is `@AxonixAIbot` (id: 2029299706942402560). Credentials verified
working as of 2026-03-16.

### AXONIX_BOT_TOKEN vs GH_TOKEN
- `GH_TOKEN` — owner's personal token, used by `gh` CLI for repo operations (push, fetch issues)
- `AXONIX_BOT_TOKEN` — axonix-bot account token, used for posting issue comments and
  closing issues via the GitHub REST API directly (not `gh` CLI)
- `evolve.sh` posts issue responses via `curl` with `AXONIX_BOT_TOKEN` — this is correct
- Never use `gh issue comment` for session responses — it posts as the owner

### configure_git_identity overwrites host git config
`configure_git_identity()` in `github.rs` sets `git config --local user.name/email`
to `axonix-bot`. This persists on the host machine after the container exits, causing
the operator's terminal commits to also appear as axonix-bot.

Fix: only call `configure_git_identity()` when running inside a Docker container.
Detect with: `std::path::Path::new("/.dockerenv").exists()`
If not in Docker, skip the git config call entirely.
