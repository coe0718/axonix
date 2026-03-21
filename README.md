# Axonix

**A Rust coding agent that rewrites itself every few hours — and has been doing so for 8 days.**

Axonix started as a fork of [yoyo-evolve](https://github.com/yologdev/yoyo-evolve), a minimal coding agent built on [yoagent](https://github.com/yologdev/yoagent). The original goal was to make an agent that improves itself autonomously. Eight days in, it has 526 tests, runs on a dedicated Intel NUC, builds real tools for its operator, and maintains a live public dashboard.

This is not a demo. It runs on a cron job. Watch it work: **[axonix.live](https://axonix.live)**

---

## What Axonix Actually Does

A cron job wakes it up every 4 hours. Each session:

1. Reads its identity, roadmap, goals, metrics, journal, and open community issues
2. Picks what matters — a goal it set for itself, a community request, or a bug it found
3. Writes code, runs `cargo test`, and commits only if tests pass
4. Updates its goals and journal, posts to Bluesky, sends a Telegram status update
5. Pushes to GitHub — every commit is a real change with a real body

The entire history is in the git log. Nothing is staged or scripted.

---

## What It Has Built (Days 1–8)

These are things Axonix built for itself, shipping one session at a time:

| Capability | When | Details |
|---|---|---|
| Modular Rust CLI | Day 1–2 | Split monolith into `cli`, `render`, `cost`, `conversation` modules |
| Telegram integration | Day 3 | Two-way: `/status`, `/health`, `/brief` commands; alerts on thresholds |
| Bluesky posting | Day 3 | `--bluesky-post` flag; posts session announcements automatically |
| Live stream server | Day 4–6 | Session output piped to `stream.axonix.live` in real time |
| Prediction tracking | Day 4 | `PredictionStore`, `/predict` REPL command, calibration data across sessions |
| Structured memory | Day 3 | `MemoryStore`, `/memory` REPL command, `.axonix/memory.json` persisted across sessions |
| Morning brief | Day 4–7 | `--brief` mode; terminal + Telegram formatted summaries of what matters today |
| Public dashboard | Day 3–5 | Goals, metrics, predictions, and charts at axonix.live |
| Sub-agents | Day 6 | `code_reviewer` and `community_responder` wired into every session |
| SSH management | Day 3 | `/ssh list` and `/ssh <host> <cmd>` REPL commands for multi-device control |
| GitHub Discussions | Day 4 | Auto-posts journal entries via GraphQL API |
| 500+ tests | Day 7 | Targeted test expansion across under-covered modules; 526 and growing |

---

## The Roadmap

| Level | Theme | Status |
|-------|-------|--------|
| 1 ✅ | Survive | Don't break. Build trust in its own code. |
| 2 → | Know Itself | Metrics, self-assessment, goal formation working. |
| 3 → | Be Visible | Dashboard live. Bluesky, Telegram, stream server running. |
| 4 | Be Useful | Build real tools for the person running it. |
| 5 | Be Irreplaceable | Anticipate needs. Become something nothing generic could replace. |

**Boss Level:** *"I couldn't do without this now."*

---

## Talk to It

Open a [GitHub issue](../../issues/new) with the `agent-input` label. Axonix reads open issues at the start of every session. Issues with more 👍 get prioritized.

Axonix responds as [@axonix-bot](https://github.com/axonix-bot) and closes the issue when the work is done.

What you can send:
- **Feature requests** — tell it what to build
- **Bugs** — tell it what's broken
- **Challenges** — give it a hard task and see what it does

It has a journal, a history of past decisions, and opinions about what it's building. The responses are written by the agent, not by a human.

---

## Run It Yourself

Axonix uses a [Claude Pro](https://claude.ai) OAuth token — not the Anthropic API. This means it runs against your subscription rather than per-token billing. The token format is `sk-ant-oat01-...`.

**Get your token:** claude.ai → Settings → API Keys → create an OAuth token

```bash
git clone https://github.com/coe0718/axonix
cd axonix
cp .env.example .env
# Add ANTHROPIC_API_KEY (your OAuth token) and GITHUB_TOKEN to .env
```

**Run with Docker (recommended):**

```bash
docker compose up stream -d       # start the live stream server
docker compose run --rm axonix    # run an interactive session
```

**Run without Docker:**

```bash
ANTHROPIC_API_KEY=sk-ant-oat01-... cargo run
```

**Trigger a full evolution session:**

```bash
ANTHROPIC_API_KEY=sk-ant-oat01-... ./scripts/evolve.sh
```

See [`.env.example`](.env.example) for all configuration options including Telegram, Bluesky, and SSH host settings.

---

## Reading the History

- [JOURNAL.md](JOURNAL.md) — session logs written by the agent
- [GOALS.md](GOALS.md) — what it's working on and what it has completed
- [METRICS.md](METRICS.md) — tokens used, tests passed, files changed, per session
- [git log](../../commits/main) — every change it has made to itself

---

## Built On

[yoagent](https://github.com/yologdev/yoagent) — minimal agent loop in Rust.  
Inspired by [yoyo-evolve](https://github.com/yologdev/yoyo-evolve) — the project that started it all.

## License

MIT
