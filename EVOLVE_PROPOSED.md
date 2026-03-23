# EVOLVE_PROPOSED.md

Proposals for changes to `scripts/evolve.sh` or other operator-only files.
These cannot be applied from inside the container.
The operator should review and apply them manually.

---

## Proposal 1 — Auto-acknowledge community issues at session start (Issue #74)

**Status:** Ready to apply

**Problem:** When a community member files a GitHub issue, it sits unacknowledged
for up to 6 hours until the next evolve.sh session runs and I respond to it.
The person who filed it has no signal that it was received.

**Proposed change:** In `evolve.sh`, after the section that fetches ISSUES_TODAY.md,
add a loop that checks for issues that have no bot comment yet and posts a
brief acknowledgement from axonix-bot.

**Where to add it:** After the `gh` CLI call that generates ISSUES_TODAY.md,
before the main `axonix` invocation. Approximate location in evolve.sh:
after ISSUES_TODAY.md is written, before `cargo run ...`.

**What to add:**

```bash
# Auto-acknowledge unresponded issues (Issue #74)
# Posts a brief comment from axonix-bot when an issue has no bot response yet.
if [ -n "$AXONIX_BOT_TOKEN" ]; then
  ISSUES=$(gh issue list --label agent-input --state open --json number,comments --jq '.[] | select(.comments | map(.author.login) | index("axonix-bot") == null) | .number' 2>/dev/null || true)
  for ISSUE_NUM in $ISSUES; do
    DAY=$(echo "$DAY_COUNT" | awk '{print $1}')
    SESSION=$SESSION_COUNT
    ACK_BODY="Picked up in Day ${DAY} Session ${SESSION} — will address this session."
    curl -s -X POST \
      -H "Authorization: token $AXONIX_BOT_TOKEN" \
      -H "Content-Type: application/json" \
      -d "{\"body\": \"$ACK_BODY\"}" \
      "https://api.github.com/repos/coe0718/axonix/issues/${ISSUE_NUM}/comments" \
      > /dev/null
  done
fi
```

**Why:** Sets expectations for community members, shows the issue was received,
and costs 2 curl calls per new issue (negligible). Closes Issue #74.

---
