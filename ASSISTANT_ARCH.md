# Personal Assistant Architecture

_G-060 — always-on Telegram listener service_

## Overview

Axonix is evolving from a batch self-improvement agent (runs every few hours via evolve.sh) into an
always-on personal assistant that responds in seconds, not minutes.  The key insight is that these
two modes are **fundamentally different**:

| Dimension          | evolve.sh sessions         | Always-on listener          |
|--------------------|----------------------------|-----------------------------|
| Purpose            | Self-improvement           | Respond to operator now     |
| Latency            | Minutes (long-running)     | Seconds (low-latency)       |
| Context window     | Large (full plan + history)| Small (recent turns only)   |
| Lifetime           | One session, then exit     | 24/7, restart on failure    |
| Cost pressure      | OK to be expensive         | Keep responses brief        |

---

## Two-Process Model

```
┌──────────────────────────────────┐   ┌──────────────────────────────────────┐
│  axonix (evolve.sh session)      │   │  axonix-listener (always-on daemon)  │
│                                  │   │                                      │
│  • Long-running improvement loop │   │  • Polls Telegram every 2 s          │
│  • Full system prompt            │   │  • Handles /ask immediately          │
│  • Reads conversation_memory at  │   │  • Short-context agent response      │
│    session start for context     │   │  • Writes turns to                   │
│  • Commits code, files, tests    │   │    .axonix/conversation_memory.json  │
└──────────────────────────────────┘   └──────────────────────────────────────┘
                          ▲                            │
                          └────────────────────────────┘
                               shared file:
                         .axonix/conversation_memory.json
```

Both processes share the workspace volume.  They communicate via a JSON file — no sockets, no
message queues, no additional infrastructure.

---

## Key Files

### `src/listener.rs` — the always-on listener module

Contains:
- `ListenerConfig` — poll interval, response size limit, memory path, max turns
- `ListenerStats` — runtime counters (messages handled, errors, uptime)
- `build_listener_system_prompt(memory)` — builds a short prompt focused on "be helpful now"
- _(future)_ `async fn run_listener(config, telegram, memory)` — the actual poll loop

The listener uses the existing `TelegramClient` for Telegram I/O and `ConversationMemory` for
persistent turn history.

### `src/conversation_memory.rs` — persistent conversation log

Stores turn-by-turn conversation history at `.axonix/conversation_memory.json`.

**Distinct from `memory.rs`** (key-value facts like `nuc.ip`, `operator.tz`).  This file contains
ordered conversation turns with timestamps, roles, and channel labels.

```json
[
  {
    "timestamp": "2026-03-22T14:30:00Z",
    "role": "user",
    "text": "what's the disk usage?",
    "channel": "telegram"
  },
  {
    "timestamp": "2026-03-22T14:30:02Z",
    "role": "assistant",
    "text": "Disk is at 45% (120GB / 250GB).",
    "channel": "telegram"
  }
]
```

Fields:
- `timestamp` — ISO 8601 UTC
- `role` — `"user"` or `"assistant"`
- `text` — message content
- `channel` — `"telegram"`, `"repl"`, or `"prompt"`

Rolling window: keeps the most recent N turns (default 100) to bound file size.

---

## Session Integration

At the start of every evolve.sh session, the agent reads `conversation_memory.json` and injects
a formatted summary into its system prompt:

```
## Recent Conversations (last 10 turns)
[2026-03-22 14:30] user: what's the disk usage?
[2026-03-22 14:30] assistant: Disk is at 45% (120GB / 250GB).
[2026-03-22 16:01] user: can you add a bluesky rate-limit check?
[2026-03-22 16:02] assistant: Added — see src/bluesky.rs lines 120-145.
```

This means evolve.sh sessions are aware of what the operator asked about during the day, without
requiring the operator to repeat themselves or manually copy context.

The injection is done via `ConversationMemory::format_for_context(n)`.

---

## Docker: Proposed `docker-compose.yml` Service Entry

```yaml
  axonix-listener:
    container_name: axonix-listener
    build: .
    command: ./target/release/axonix --listen
    user: "${UID}:${GID}"
    volumes:
      - .:/workspace
      - ./scripts/evolve.sh:/workspace/scripts/evolve.sh:ro
      - ./deploy_key:/workspace/.ssh/id_ed25519:ro
      - /etc/localtime:/etc/localtime:ro
      - /etc/timezone:/etc/timezone:ro
    environment:
      - TZ=${TZ:-America/Indiana/Indianapolis}
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - TELEGRAM_BOT_TOKEN=${TELEGRAM_BOT_TOKEN:-${TELEGRAM_TOKEN}}
      - TELEGRAM_CHAT_ID=${TELEGRAM_CHAT_ID}
      - HOME=/workspace
      - CARGO_HOME=/workspace/.cargo
    restart: always
```

Note: The `--listen` CLI flag does not exist yet — it will be added when the full async poll loop
is wired into `main.rs` (planned for a future session).

---

## Gaps (What Doesn't Exist Yet)

| Gap                                | Status    | Planned |
|------------------------------------|-----------|---------|
| `async fn run_listener()`          | Missing   | Next session |
| `--listen` CLI flag in `main.rs`   | Missing   | When run_listener is ready |
| `bin/listener.rs` binary entry     | Missing   | Optional (could use main.rs flag) |
| Rate limiting for burst /ask       | Missing   | Future |
| Graceful shutdown (SIGTERM)        | Missing   | When run_listener is ready |
| Context injection in session start | Missing   | After run_listener exists |
| Multi-user support                 | Not planned | N/A (single-operator model) |

---

## Design Decisions

**Why a file, not a socket?**  The workspace is already a shared Docker volume.  A JSON file
requires zero infrastructure, is human-readable, diffable in git (if operator wants to check it),
and survives restarts of either process without data loss.

**Why a rolling window?**  Unbounded conversation logs grow forever and eventually become noise.
100 turns (~50 exchanges) gives roughly a day of moderate use without bloat.

**Why separate from `memory.json`?**  Key-value facts (`nuc.ip`, `operator.tz`) are structured,
stable, and operator-curated.  Conversation turns are ephemeral, ordered, and high-volume.  Mixing
them would make both stores harder to use.

**Why not async in listener.rs yet?**  The listener module is designed to be wired into main.rs
with a `--listen` flag.  Adding `async fn run_listener()` without the CLI entry point would be
dead code.  The config, stats, and system prompt builder are enough to unblock the architecture
design and tests.
