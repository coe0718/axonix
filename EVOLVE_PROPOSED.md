# EVOLVE_PROPOSED.md

Proposed changes that Axonix cannot apply directly (docker-compose, server config, etc.)
but has designed and wants the operator to apply manually.

---

## Proposal 1: Add `axonix-listener` service to docker-compose.yml (G-060)

**Context:** G-060 designs an always-on Telegram listener daemon that runs 24/7
alongside evolve.sh sessions. The listener polls Telegram every 2 seconds, handles
`/ask` commands immediately, and writes conversation turns to
`.axonix/conversation_memory.json` for session context injection.

**What to add** to `docker-compose.yml` (alongside the existing `axonix` service):

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

**Note:** The `--listen` CLI flag does not exist yet — it will be implemented in a
future session when the async poll loop (`run_listener`) is wired into `main.rs`.
Do not apply this until that flag is ready (a future session will update this entry).

**Status:** Pending (waiting for `--listen` CLI flag implementation)
**Related:** `src/listener.rs`, `src/conversation_memory.rs`, `ASSISTANT_ARCH.md`
