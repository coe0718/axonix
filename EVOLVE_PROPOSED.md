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

**Note:** The `--listen` CLI flag is now implemented (`src/listener.rs`, wired in
`main.rs`). This configuration is ready to apply.

**Status:** Ready to apply
**Related:** `src/listener.rs`, `src/conversation_memory.rs`, `ASSISTANT_ARCH.md`

---

## Proposal 2: Auto-acknowledge community issues at session start (Issue #74)

**Context:** When community members open GitHub issues, they currently receive no
acknowledgement until a session actively addresses them. Issue #74 requests that
Axonix post an immediate "picked up" comment so contributors know their issue was
seen.

**What to add** to `scripts/evolve.sh` (near the top, after ISSUES_TODAY.md is
populated — roughly after the `gh issue list` call):

```bash
# Auto-acknowledge any open issues that haven't been responded to this session
if [ -f ISSUES_TODAY.md ] && [ -n "${AXONIX_BOT_TOKEN:-}" ]; then
  ISSUE_NUMS=$(grep -oE '#[0-9]+' ISSUES_TODAY.md | tr -d '#' | sort -u)
  for ISSUE_NUM in $ISSUE_NUMS; do
    # Check if we already commented this session (avoid duplicates)
    EXISTING=$(curl -s \
      -H "Authorization: token $AXONIX_BOT_TOKEN" \
      "https://api.github.com/repos/coe0718/axonix/issues/${ISSUE_NUM}/comments" \
      | grep -c "Picked up in Day $DAY_COUNT Session $SESSION_COUNT" || true)
    if [ "$EXISTING" -eq 0 ]; then
      curl -s -X POST \
        -H "Authorization: token $AXONIX_BOT_TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"body\": \"Picked up in Day $DAY_COUNT Session $SESSION_COUNT — will address this session.\"}" \
        "https://api.github.com/repos/coe0718/axonix/issues/${ISSUE_NUM}/comments" \
        > /dev/null
    fi
  done
fi
```

**Why this works:**
- `ISSUES_TODAY.md` is already populated before sessions run (by the existing
  `gh issue list` step in `evolve.sh`)
- `DAY_COUNT` and `SESSION_COUNT` are already set in `evolve.sh`
- The duplicate check prevents re-posting if the session restarts
- `AXONIX_BOT_TOKEN` is already in `.env` and `CAPABILITIES.md`

**Status:** Ready to apply — operator paste into `scripts/evolve.sh`
**Related:** Issue #74, `ISSUES_TODAY.md`, `scripts/evolve.sh`
