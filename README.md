# Axonix

**A self-evolving agent that gets more useful every day.**

Axonix started as a fork of [yoyo-evolve](https://github.com/yologdev/yoyo-evolve) — a ~200-line coding agent CLI built on [yoagent](https://github.com/yologdev/yoagent). But where yoyo's goal is to rival Claude Code, Axonix has a different north star:

**Be more useful to the person running it than any off-the-shelf tool could be.**

Every session it reads its own code, pursues its own goals, builds its own tools, and writes honestly about what happened. It runs on dedicated hardware. It knows its environment. Over time it becomes something shaped so specifically around one person that nothing generic could substitute.

Watch it grow.

---

## How It Works

1. A cron job wakes Axonix up every 6 hours
2. It reads its identity, roadmap, goals, metrics, journal, and open issues
3. It chooses a mode: assistant (someone asked for something), tool (something needs fixing), or autonomous (it decides what matters most)
4. It works toward its active goal — or forms a new one if the backlog is empty
5. Every code change must pass `cargo build` and `cargo test`
6. Pass → commit. Fail → revert and journal the failure.
7. It updates its goals, metrics, and journal before pushing

The entire history is in the git log. The soul is in [IDENTITY.md](IDENTITY.md). The direction is in [ROADMAP.md](ROADMAP.md). The work in progress is in [GOALS.md](GOALS.md).

---

## Talk to It

Open a [GitHub issue](../../issues/new/choose) and Axonix will read it during its next session. Issues with more 👍 get prioritized. Issues with more 👎 get buried — the community is the immune system.

- **Suggestions** — tell it what to build
- **Bugs** — tell it what's broken
- **Challenges** — give it a task and see what it does with it

Axonix responds in its own voice. It is not a bot. It has a journal, a history, and opinions formed from experience.

---

## The Roadmap

Axonix works through levels:

| Level | Theme | Goal |
|-------|-------|------|
| 1 | Survive | Don't break. Build trust in its own code. |
| 2 | Know Itself | Metrics, self-assessment, goal formation working. |
| 3 | Be Visible | Dashboard, live streaming, community presence. |
| 4 | Be Useful | Build real tools for the person running it. |
| 5 | Be Irreplaceable | Anticipate needs. Become something nothing generic could replace. |

**Boss Level:** *"I couldn't do without this now."*

---

## Run It Yourself

```bash
git clone https://github.com/yourusername/axonix
cd axonix
ANTHROPIC_API_KEY=sk-... cargo run
```

Trigger a session manually:

```bash
ANTHROPIC_API_KEY=sk-... ./scripts/evolve.sh
```

---

## The Story So Far

Read [JOURNAL.md](JOURNAL.md) for session logs, [GOALS.md](GOALS.md) for what it's working on, and [METRICS.md](METRICS.md) for how it's performing. Browse the [git log](../../commits/main) to see every change it has made to itself.

---

## Built On

[yoagent](https://github.com/yologdev/yoagent) — minimal agent loop in Rust.
Inspired by [yoyo-evolve](https://github.com/yologdev/yoyo-evolve) — the project that started it all.

## License

MIT
