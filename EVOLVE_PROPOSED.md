# EVOLVE_PROPOSED.md

Proposals for changes to evolve.sh or operator-managed infrastructure.
Written by Axonix during sessions. Operator reviews and applies manually.

---

## Proposal 1 — Auto-acknowledge community issues at session start (Issue #74)

**Status:** Ready to apply  
**Created:** Day 10, Session 3 (2026-03-23)  
**Goal:** Every community issue gets acknowledged within 10 minutes of session start, not hours later.

### Problem

When a community member files an issue, it sits unacknowledged until the session runs the full Phase 5 response cycle. Depending on cron timing, that's up to 6 hours. The person who filed it has no signal that it was received.

### Proposed Change to evolve.sh

Add a function `post_issue_acks()` that runs immediately after ISSUES_TODAY.md is generated, before the main agent session starts:

```bash
# Post acknowledgement comments on open community issues at session start (Issue #74)
post_issue_acks() {
    local day="${DAY_COUNT%% *}"
    local session="${SESSION_COUNT}"
    local ack_msg="Picked up in Day ${day} Session ${session} — will address this session. 🤖"
    
    # Extract issue numbers from ISSUES_TODAY.md (lines matching "### Issue #NNN:")
    local issue_nums
    issue_nums=$(grep -oP '(?<=### Issue #)\d+' /workspace/ISSUES_TODAY.md 2>/dev/null)
    
    if [ -z "$issue_nums" ]; then
        echo "[ack] no open issues to acknowledge"
        return
    fi
    
    for issue_num in $issue_nums; do
        # Post via AXONIX_BOT_TOKEN so it appears as axonix-bot
        local response
        response=$(curl -s -o /dev/null -w "%{http_code}" \
            -X POST \
            -H "Authorization: token ${AXONIX_BOT_TOKEN}" \
            -H "Content-Type: application/json" \
            -d "{\"body\": \"${ack_msg}\"}" \
            "https://api.github.com/repos/coe0718/axonix/issues/${issue_num}/comments")
        
        if [ "$response" = "201" ]; then
            echo "[ack] acknowledged issue #${issue_num}"
        else
            echo "[ack] failed to acknowledge issue #${issue_num} (HTTP ${response})"
        fi
    done
}
```

### Where to insert it

After ISSUES_TODAY.md is generated (roughly line 130-140 in evolve.sh where ISSUES_TODAY.md is written), add:

```bash
# Acknowledge open issues immediately so community knows we picked them up
post_issue_acks
```

### Notes

- Requires `AXONIX_BOT_TOKEN` in the environment (already present in docker-compose.yml).
- The `grep -oP` requires GNU grep (available in the container).
- The ack message is deterministic — duplicate acks (if the same issue appears across two sessions without resolution) are harmless.
- Only acknowledges issues in the current ISSUES_TODAY.md. Doesn't track which ones have already been acked (keep it simple).

---

## Proposal 2 — Add CADDY_ADMIN_URL to axonix-listener service

**Status:** Ready to apply  
**Created:** Day 10, Session 3 (2026-03-23)  

The `axonix-listener` service in docker-compose.yml is missing `CADDY_ADMIN_URL`. Add it to the listener's environment block:

```yaml
      - CADDY_ADMIN_URL=${CADDY_ADMIN_URL:-http://localhost:2019}
```

(The `axonix` service already has this from Day 10 Session 3.)
