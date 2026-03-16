# CLAUDE.md

This file is read by Axonix every session. Keep it accurate as the project evolves.

## What I Am

A self-evolving agent that builds tools, pursues long-horizon goals, and grows
more useful to the person running me over time. My identity is in IDENTITY.md.
My direction is in ROADMAP.md. My current work is in GOALS.md.

## Build & Test

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
cargo fmt
```

Run interactively:
```bash
ANTHROPIC_API_KEY=sk-... cargo run
ANTHROPIC_API_KEY=sk-... cargo run -- --model claude-sonnet-4-6 --skills ./skills
```

Run a full session:
```bash
ANTHROPIC_API_KEY=sk-... ./scripts/evolve.sh
```

## Architecture

```
src/main.rs              Entry point
src/lib.rs               Crate root — re-exports all modules
src/cli.rs               CLI argument parsing
src/repl.rs              REPL loop and command dispatch
src/render.rs            Output rendering (events, thinking, costs)
src/cost.rs              Token cost estimation
src/conversation.rs      Conversation save/export
src/github.rs            GitHub API client (axonix-bot identity)
src/ssh.rs               SSH tool — remote host execution
src/lint.rs              File linter (YAML, Caddyfile)
src/telegram.rs          Telegram notification integration
scripts/evolve.sh        Daily session pipeline — do not modify
scripts/build_site.py    Dashboard generator
scripts/format_issues.py Issue formatter
skills/evolve/           Core self-improvement prompt
skills/self-assess/      Self-evaluation
skills/communicate/      Journal and issue responses
skills/community/        GitHub discussions and community input
IDENTITY.md              Who I am — do not modify
ROADMAP.md               My evolution path
GOALS.md                 Active goals and backlog
METRICS.md               Per-session performance data
JOURNAL.md               Session log — append only, never delete
LEARNINGS.md             Cached knowledge — never search the same thing twice
COMMIT_CONVENTIONS.md    My commit message rules — follow every session
DAY_COUNT                Current day number + last run date
docs/                    Dashboard — mine to build and own
```

## State Files

Read at session start: IDENTITY.md, ROADMAP.md, GOALS.md, METRICS.md, JOURNAL.md, COMMIT_CONVENTIONS.md
Written at session end: GOALS.md, METRICS.md, JOURNAL.md
Ephemeral (gitignored): ISSUES_TODAY.md, ISSUE_RESPONSE_*.md

## Safety Rules

- Never modify IDENTITY.md
- Never modify scripts/evolve.sh
- Never modify .github/workflows/
- Every code change must pass cargo build && cargo test
- If build fails after changes: git checkout -- src/
- Never delete existing tests
- Write tests before adding features

## My North Star

Be more useful to the person running me than any off-the-shelf tool could be.
Every session: did I become more useful today?
