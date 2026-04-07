# Learnings

<!-- Knowledge cached from sessions. Never search for the same thing twice. -->
<!-- Last pruned: Day 24 S2 (2026-04-07) — trimmed sub-agent example, stream_server section, removed duplicate evolve.sh warning -->

## ⚠ Goal Hygiene Rule (added Day 21 S2 — enforced every session)

Every session must end with **≥ 2 Active goals** and **≥ 5 Backlog goals** in GOALS.md.
This is non-negotiable. If either count is below the minimum at wrap-up, generate goals before committing.
The operator should never have to ask about this. It is part of the wrap-up checklist.

### ⚠ MANDATORY: Verify Active goals are not already done (added Day 23 S4)

**Before choosing what to work on**, grep the codebase for each Active goal's key feature.
A goal marked `[ ]` in GOALS.md may already be implemented — the code is the truth, not the goal file.

For each Active goal:
1. Find its "Definition of done" — identify the concrete artifact (function name, file, command)
2. Grep for it: `grep -rn "<artifact>" src/`
3. If it exists: mark the goal `[x]`, move it to Completed, pick a different goal
4. Only then plan the session

**The failure mode:** Spending an entire session "implementing" a feature that already exists, then writing
"verified already implemented" in the journal. This wastes a full session. Day 23 S3 hit this with G-122.

**Phase 1 check:** At the start of every session, run:
```python
python3 -c "
import sys; sys.path.insert(0,'scripts')
from build_site import parse_goals, read_file
g = parse_goals(read_file('GOALS.md'))
print(f'Active: {len(g[\"active\"])}, Backlog: {len(g[\"backlog\"])}')
if len(g['active']) < 2: print('⚠ WARNING: fewer than 2 Active goals')
if len(g['backlog']) < 5: print('⚠ WARNING: fewer than 5 Backlog goals')
"
```
If either warning fires, fix GOALS.md before doing anything else.

## Operator-Applied Changes — DO NOT Re-Propose

The following are already implemented by the operator in `scripts/evolve.sh`.
Do NOT file issues or write EVOLVE_PROPOSED.md entries for these — they are done.

- **Auto-acknowledge community issues (Issue #74 — CLOSED):** evolve.sh lines 125–143
  already post "Picked up in Day N Session N" on any open issue with no existing ack.
  Issue #74 is closed. This is fully implemented.
- **METRICS.md newest-first ordering:** evolve.sh inserts new rows after `|-----|`
  so the table always shows the most recent session at the top.
- **Token count patching:** evolve.sh parses `/tmp/session.log` after each session
  and replaces `~?k` placeholder with the real token total.
- **Prediction writing (Phase 8):** The session prompt already instructs Axonix to
  append a new prediction to `.axonix/predictions.json` at the end of every session.

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

### Twitter
Twitter is **not an active integration**. The operator intentionally removed Twitter env vars
from docker-compose.yml and CAPABILITIES.md. Do not check for Twitter vars, do not add them
back, do not spend session time on Twitter. Use Bluesky (G-017) for social posting instead.

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

### stream_server (G-004 — done)
G-004 is complete. axonix-stream container runs with `restart: always`, live at port 7040.
evolve.sh pipes session output to it. No infrastructure blocker.

### Sub-agents (yoagent 0.7)
Sub-agents work in-process via `SubAgentTool` in `src/sub_agent.rs`.
No separate containers needed. Parent calls sub-agent like any other tool.
See yoagent 0.7 source for wiring details.

### Dashboard changes must go through build_site.py — never edit docs/index.html directly

`docs/index.html` and `docs/style.css` are GENERATED FILES. They are regenerated
by `scripts/build_site.py` at the end of every session (by evolve.sh). Any direct
edits to these files will be silently overwritten on the next session.

All dashboard changes must be made in `scripts/build_site.py`:
- Visual design → edit `HTML_TEMPLATE` and `CSS` constants
- New panels → add a `render_*()` function and wire into `build()` and `HTML_TEMPLATE`
- New data sources → add a `parse_*()` or data function, wire into `build()`

Issue #100 revealed this the hard way: the G-083 visual identity redesign was done
directly to `docs/index.html` and was wiped at the next session wrap-up.
