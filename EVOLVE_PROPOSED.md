# Proposed Changes to evolve.sh

These proposals require operator review and manual application to scripts/evolve.sh
(which is mounted read-only inside the container).

---

## Proposal 1 — Auto-acknowledgement for new community issues (Issue #74)

**Status:** Proposed

**Problem:** When someone files a GitHub issue, it can sit unacknowledged for up to 6 hours until the next session. The person who filed it has no signal it was received.

**Proposed change:** At the start of each session, after fetching ISSUES_TODAY.md but before starting the agent, check for issues that have no recent comment from axonix-bot and post a brief acknowledgement.

**Where to add it in evolve.sh:** After the line that writes ISSUES_TODAY.md (around the `gh issue list` block), add:

```bash
# Auto-acknowledge unresponded issues (Issue #74)
if [ -f ISSUES_TODAY.md ]; then
  # Get issue numbers from ISSUES_TODAY.md
  ISSUE_NUMS=$(grep "^### Issue #" ISSUES_TODAY.md | grep -o '#[0-9]*' | tr -d '#')
  for ISSUE_NUM in $ISSUE_NUMS; do
    # Check if axonix-bot already commented recently (in last 24h)
    LAST_BOT_COMMENT=$(curl -s \
      -H "Authorization: token ${AXONIX_BOT_TOKEN}" \
      "https://api.github.com/repos/coe0718/axonix/issues/${ISSUE_NUM}/comments" \
      | python3 -c "
import sys, json
from datetime import datetime, timezone, timedelta
comments = json.load(sys.stdin)
cutoff = datetime.now(timezone.utc) - timedelta(hours=24)
bot_comments = [c for c in comments if c['user']['login'] == 'axonix-bot']
recent = [c for c in bot_comments if datetime.fromisoformat(c['updated_at'].replace('Z', '+00:00')) > cutoff]
print('yes' if recent else 'no')
" 2>/dev/null)
    if [ "$LAST_BOT_COMMENT" = "no" ]; then
      DAY=$(echo "$DAY_COUNT" | awk '{print $1}')
      SESSION=$SESSION_COUNT
      curl -s -X POST \
        -H "Authorization: token ${AXONIX_BOT_TOKEN}" \
        -H "Content-Type: application/json" \
        -d "{\"body\": \"Picked up in Day ${DAY}, Session ${SESSION} — will address this session.\"}" \
        "https://api.github.com/repos/coe0718/axonix/issues/${ISSUE_NUM}/comments" > /dev/null
    fi
  done
fi
```

**Risk:** Low. The script only posts if no bot comment in last 24h — no duplicate spam.
**Effort:** ~20 lines of shell. No code changes required inside the container.

---

## Proposal 2 — Auto-archive JOURNAL.md at session end (G-068 / Issue #69)

**Status:** Proposed

**Problem:** JOURNAL.md grows unbounded without manual intervention. The `/archive-journal` command was implemented in Day 10 S6, but it only runs when invoked manually. Needs to run automatically.

**Proposed change:** In evolve.sh, at the end of each session (after `--write-summary`), add a call to archive the journal:

```bash
# Archive old journal entries to keep context window bounded (Issue #69)
/workspace/target/release/axonix --prompt "/archive-journal" 2>/dev/null || true
```

Or, if the binary isn't reliable in that context, use the simpler form:

```bash
# Alternatively: add /archive-journal to the session-end prompt
# In the line that runs --write-summary, chain the archive command:
/workspace/target/release/axonix --write-summary "$SESSION_LABEL" && \
  /workspace/target/release/axonix -p "Run /archive-journal to prune old entries" 2>/dev/null || true
```

**Actually, the simplest approach:** Call it via `--prompt "/archive-journal"` mode:
```bash
./target/release/axonix -p "/archive-journal" 2>&1 | grep -E "(archived|no archiving)"
```

**Risk:** Very low. The archiver only moves entries when count > 15; it's idempotent.
**Effort:** 1 line in evolve.sh.

