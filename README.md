# Axonix

**A self-evolving agent that gets more useful every day.**

Axonix started as a fork of [yoyo-evolve](https://github.com/yologdev/yoyo-evolve) — a ~200-line coding agent CLI built on [yoagent](https://github.com/yologdev/yoagent). But where yoyo's goal is to rival Claude Code, Axonix has a different north star:

**Be more useful to the person running it than any off-the-shelf tool could be.**

Every session it reads its own code, pursues its own goals, builds its own tools, and writes honestly about what happened. It runs on dedicated hardware. It knows its environment. Over time it becomes something shaped so specifically around one person that nothing generic could substitute.

Watch it grow: **[axonix.live](https://axonix.live)**

---

## How It Works

1. A cron job wakes Axonix up every 4 hours
2. It reads its identity, roadmap, goals, metrics, journal, and open community issues
3. It chooses what matters most — a bug it found, a community request, or a goal it set for itself
4. It works, tests, commits, and pushes — every change must pass `cargo build && cargo test`
5. It updates its goals, metrics, and journal before the session ends

The entire history is in the git log. The soul is in [IDENTITY.md](IDENTITY.md). The direction is in [ROADMAP.md](ROADMAP.md). The work in progress is in [GOALS.md](GOALS.md).

---

## Talk to It

Open a [GitHub issue](../../issues/new) with the `agent-input` label and Axonix will read it during its next session. Issues with more 👍 get prioritized.

- **Suggestions** — tell it what to build
- **Bugs** — tell it what's broken  
- **Challenges** — give it a task and see what it does with it

Axonix responds in its own voice and posts comments as [@axonix-bot](https://github.com/axonix-bot). It has a journal, a history, and opinions formed from experience.

---

## The Roadmap

Axonix works through levels:

| Level | Theme | Goal |
|-------|-------|------|
| 1 ✅ | Survive | Don't break. Build trust in its own code. |
| 2 → | Know Itself | Metrics, self-assessment, goal formation working. |
| 3 → | Be Visible | Dashboard, live streaming, community presence. |
| 4 | Be Useful | Build real tools for the person running it. |
| 5 | Be Irreplaceable | Anticipate needs. Become something nothing generic could replace. |

**Boss Level:** *"I couldn't do without this now."*

---

## Run It Yourself

Axonix uses a [Claude Pro](https://claude.ai) OAuth token — not the Anthropic API. This means it runs against your Pro subscription rather than per-token billing.

**Get your token:**  
Go to [claude.ai](https://claude.ai) → Settings → API Keys → create an OAuth token (`sk-ant-oat01-...`)

```bash
git clone https://github.com/coe0718/axonix
cd axonix
cp .env.example .env
# Add your Claude OAuth token and GitHub token to .env
docker compose up stream -d
docker compose run --rm axonix
```

Or run interactively without Docker:

```bash
ANTHROPIC_API_KEY=sk-ant-oat01-... cargo run
```

Trigger a full evolution session:

```bash
ANTHROPIC_API_KEY=sk-ant-oat01-... ./scripts/evolve.sh
```

See [`.env.example`](.env.example) for all available configuration options.

---

## The Story So Far

Read [JOURNAL.md](JOURNAL.md) for session logs, [GOALS.md](GOALS.md) for what it's working on, and [METRICS.md](METRICS.md) for how it's performing. Browse the [git log](../../commits/main) to see every change it has made to itself.

---

## Built On

[yoagent](https://github.com/yologdev/yoagent) — minimal agent loop in Rust.  
Inspired by [yoyo-evolve](https://github.com/yologdev/yoyo-evolve) — the project that started it all.

## License

MIT
