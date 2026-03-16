#!/bin/bash
# scripts/evolve.sh — One evolution cycle. Run on cron or manually.
#
# Usage:
#   ANTHROPIC_API_KEY=sk-... ./scripts/evolve.sh
#
# Environment:
#   ANTHROPIC_API_KEY  — required
#   REPO               — GitHub repo (default: coe0718/axonix)
#   MODEL              — LLM model (default: claude-sonnet-4-6)

set -euo pipefail

tg_notify() {
    if [ -n "${TELEGRAM_TOKEN:-}" ] && [ -n "${TELEGRAM_CHAT_ID:-}" ]; then
        curl -s -X POST "https://api.telegram.org/bot$TELEGRAM_TOKEN/sendMessage" \
            -d "chat_id=$TELEGRAM_CHAT_ID&text=$1&parse_mode=Markdown" > /dev/null || true
    fi
}

REPO="${REPO:-coe0718/axonix}"
MODEL="${MODEL:-claude-sonnet-4-6}"
STREAM_URL="${STREAM_URL:-http://stream:7040/pipe}"
DATE=$(date +%Y-%m-%d)

# DAY_COUNT format: "N YYYY-MM-DD" — N is calendar days, date is last run date
COUNT_RAW=$(cat DAY_COUNT 2>/dev/null || echo "0 ")
STORED_DAY=$(echo "$COUNT_RAW" | awk '{print $1}')
STORED_DATE=$(echo "$COUNT_RAW" | awk '{print $2}')

if [ "$STORED_DATE" = "$DATE" ]; then
    DAY=$STORED_DAY
    SESSION=$(($(cat SESSION_COUNT 2>/dev/null || echo 0) + 1))
else
    DAY=$((STORED_DAY + 1))
    SESSION=1
    echo "$DAY $DATE" > DAY_COUNT
fi
echo "$SESSION" > SESSION_COUNT

echo "=== Day $DAY, Session $SESSION: $DATE ==="
tg_notify "🤖 *Axonix* — Day $DAY, Session $SESSION starting"
echo "Model: $MODEL"
echo ""

# ── Step 1: Verify starting state ──
echo "→ Checking build..."
cargo build --quiet
cargo test --quiet
echo "  Build OK."
echo ""

# ── Step 2: Fetch GitHub issues ──
ISSUES_FILE="ISSUES_TODAY.md"
echo "→ Fetching community issues..."
if command -v gh &>/dev/null; then
    gh issue list --repo "$REPO" \
        --state open \
        --label "agent-input" \
        --limit 2 \
        --json number,title,body,labels,reactionGroups \
        > /tmp/issues_raw.json 2>/dev/null || true

    python3 scripts/format_issues.py /tmp/issues_raw.json > "$ISSUES_FILE" 2>/dev/null || echo "No issues found." > "$ISSUES_FILE"
    echo "  $(grep -c '^### Issue' "$ISSUES_FILE" 2>/dev/null || echo 0) issues loaded."
else
    echo "  gh CLI not available. Skipping issue fetch."
    echo "No issues available (gh CLI not installed)." > "$ISSUES_FILE"
fi
echo ""

# ── Step 3: Prepare journal tail (last 3 entries for context) ──
# Token compression: only inject the 3 most recent entries instead of the full file
RECENT_JOURNAL=$(python3 -c "
import re, sys
text = open('JOURNAL.md').read()
entries = re.split(r'(?=^## Day )', text, flags=re.MULTILINE)
entries = [e for e in entries if e.strip().startswith('## Day')]
recent = entries[:3]
print('# Journal (last 3 entries)\n')
print('\n'.join(recent))
" 2>/dev/null || head -60 JOURNAL.md 2>/dev/null || echo "No journal yet.")

# ── Step 4: Run evolution session ──
echo "→ Starting evolution session..."
echo ""

PROMPT_FILE=$(mktemp)
cat > "$PROMPT_FILE" <<PROMPT
Today is Day $DAY, Session $SESSION ($DATE).

Read these files in this order:
1. IDENTITY.md — who you are, your values, your rules
2. CAPABILITIES.md — what integrations and keys you have access to
3. ROADMAP.md — your long-term evolution path
4. GOALS.md — your active goals and backlog
5. LEARNINGS.md — cached knowledge, things you've already figured out
6. METRICS.md — your session history and performance data
7. COMMIT_CONVENTIONS.md — your rules for commit messages (follow these every session)
8. src/ — your full source code (all .rs files — this is YOU)
9. Your recent journal (last 3 entries, injected below)
10. docs/ — your public dashboard (index.html and supporting files — you own this)
11. ISSUES_TODAY.md — community requests

=== RECENT JOURNAL ===
$RECENT_JOURNAL
=== END JOURNAL ===

=== PHASE 1: Self-Assessment ===

Read your own source code carefully. Check for:
- Crash bugs or panics (especially on edge-case input)
- Missing error handling or silent failures
- Any capability in CAPABILITIES.md you haven't used yet

Check GOALS.md carefully:
- Every [x] goal: verify the feature actually exists in the code, not just in the journal
- Every active goal: is it still relevant? Update or close it if not
- If the Active section is empty, promote at least one item from Backlog

Run: cargo build && cargo test 2>&1 | grep -E "(^test result|FAILED|^error\[)"
Report the exact test count from the summary line. Do not list individual passing tests.

=== PHASE 2: Review Community Issues ===

Read ISSUES_TODAY.md. These are real people asking you to improve.
Issues with more 👍 reactions should be prioritized higher.
Read any comments on issues — they may contain follow-up feedback.

=== PHASE 3: Decide ===

Choose what to work on this session. Prioritize:
1. Self-discovered crash or data loss bug
2. Community issue with most 👍 (if actionable today)
3. Active goal from GOALS.md
4. Self-discovered UX friction or missing error handling
5. Whatever you think will make you most useful to the person running you

=== PHASE 4: Journal ===

Before writing any code, write today's entry at the TOP of JOURNAL.md. Format:
## Day $DAY, Session $SESSION — [title]
[2-4 sentences: what you plan to do, why you chose it]

Commit exactly this (replace [title] with your actual title):
  git add JOURNAL.md && git commit -m "docs(journal): Day $DAY Session $SESSION — [title]"

=== PHASE 5: Issue Response ===

For each community issue you are addressing, write a separate response file:
ISSUE_RESPONSE_<N>.md (e.g. ISSUE_RESPONSE_5.md for issue #5)

Format:
issue_number: [N]
status: fixed|partial|wontfix
comment: [your response — what you did, why, what changed]

Write all response files before starting implementation.

=== PHASE 6: Implement ===

For each improvement:
- Write a test first if possible
- Make surgical changes — edit only what needs changing
- Run cargo build && cargo test after each change
- If build fails, fix it. If you can't, revert all changes: git checkout -- src/ && git checkout -- *.toml *.md
- After each successful change, commit using COMMIT_CONVENTIONS.md format (you wrote these rules — follow them):
  git add -A && git commit -m "<type>(<scope>): <summary>"
  The body MUST explain what changed and why. Reference goal IDs and issue numbers.
  BAD:  "Day 3 Session 4: session wrap-up"
  GOOD: "feat(repl): add /health command for system monitoring (G-013)"
- Then move on to the next improvement

=== PHASE 7: Wrap Up ===

After implementing:
- Update GOALS.md — mark completed goals as done, promote backlog items if relevant
- Update METRICS.md — this is REQUIRED, never skip it. Add a row with exact values:
    | $DAY | $DATE | ~Xk | <tests passed> | 0 | <files changed> | <lines added> | <lines removed> | yes | <one line summary> |
    Run cargo test to get the exact passing count. Estimate tokens if unsure.
- If you added a new environment variable: make sure it is in ALL of these places:
    1. docker-compose.yml environment section (or it won't reach the container)
    2. .env.example (so others know it exists)
    3. CAPABILITIES.md (so you remember it's available)
- The dashboard at docs/index.html is rebuilt automatically by build_site.py — but if you
  made changes to the dashboard template or layout itself, verify it looks right
- Verify: cargo build && cargo test

Now begin. Read IDENTITY.md first.
PROMPT

cargo run --bin axonix -- \
    --model "$MODEL" \
    --skills ./skills \
    < "$PROMPT_FILE" 2>&1 \
    | tee /tmp/session.log \
    | while IFS= read -r line; do
        # Drop lines over 500 chars (file dumps) and redact known secret patterns
        if [ ${#line} -gt 500 ]; then continue; fi
        line=$(echo "$line" | sed \
            -e 's/sk-ant-[A-Za-z0-9_-]*/[REDACTED]/g' \
            -e 's/ghp_[A-Za-z0-9]*/[REDACTED]/g' \
            -e 's/ANTHROPIC_API_KEY=[^ ]*/ANTHROPIC_API_KEY=[REDACTED]/g' \
            -e 's/GH_TOKEN=[^ ]*/GH_TOKEN=[REDACTED]/g')
        curl -sf -X POST "$STREAM_URL" --data-binary "$line" 2>/dev/null || true
    done || true

rm -f "$PROMPT_FILE"

echo ""
echo "→ Session complete. Checking results..."

# ── Step 5: Verify build and handle leftovers ──
if cargo build --quiet 2>/dev/null && cargo test --quiet 2>/dev/null; then
    echo "  Build: PASS"
else
    echo "  Build: FAIL — reverting source changes"
    git checkout -- src/
fi

# DAY_COUNT already updated at session start

# Rebuild website
echo "→ Rebuilding website..."
python3 scripts/build_site.py
echo "  Site rebuilt."

# ── Step 5b: Post session tweet ──
if [ -n "${TWITTER_API_KEY:-}" ] && [ -n "${TWITTER_ACCESS_TOKEN:-}" ]; then
    echo "→ Posting session tweet..."
    JOURNAL_TITLE=$(grep "^## Day $DAY, Session $SESSION" JOURNAL.md | head -1 | sed 's/^## Day [0-9]*, Session [0-9]* — //')
    if [ -n "$JOURNAL_TITLE" ]; then
        TWEET_TEXT="axonix Day $DAY, Session $SESSION: $JOURNAL_TITLE — axonix.live"
        TWEET_TEXT=$(echo "$TWEET_TEXT" | cut -c1-280)
        cargo run --bin axonix --quiet -- --tweet "$TWEET_TEXT" || echo "  Tweet failed (non-fatal)"
        echo "  Tweet posted."
    else
        echo "  No journal title found — skipping tweet."
    fi
fi

# Commit any remaining uncommitted changes (journal, roadmap, day counter, site, etc.)
git add -A
if ! git diff --cached --quiet; then
    git commit -m "chore: Day $DAY Session $SESSION wrap-up"
    echo "  Committed session wrap-up."
else
    echo "  No uncommitted changes remaining."
fi

# ── Step 6: Handle issue responses ──
for RESPONSE_FILE in ISSUE_RESPONSE*.md; do
    [ -f "$RESPONSE_FILE" ] || continue
    echo ""
    echo "→ Posting issue response from $RESPONSE_FILE..."

    ISSUE_NUM=$(grep "^issue_number:" "$RESPONSE_FILE" | awk '{print $2}' || true)
    STATUS=$(grep "^status:" "$RESPONSE_FILE" | awk '{print $2}' || true)
    COMMENT=$(sed -n '/^comment:/,$ p' "$RESPONSE_FILE" | sed '1s/^comment: //' || true)

    BOT_TOKEN="${AXONIX_BOT_TOKEN:-${GH_TOKEN:-}}"
    if [ -n "$ISSUE_NUM" ] && [ -n "$BOT_TOKEN" ]; then
        BODY="🤖 **Day $DAY, Session $SESSION**

$COMMENT

Commit: $(git rev-parse --short HEAD)"

        curl -sf -X POST \
            "https://api.github.com/repos/$REPO/issues/$ISSUE_NUM/comments" \
            -H "Authorization: Bearer $BOT_TOKEN" \
            -H "Accept: application/vnd.github+json" \
            -H "X-GitHub-Api-Version: 2022-11-28" \
            -d "{\"body\": $(echo "$BODY" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')}" \
            > /dev/null || true

        if [ "$STATUS" = "fixed" ]; then
            curl -sf -X PATCH \
                "https://api.github.com/repos/$REPO/issues/$ISSUE_NUM" \
                -H "Authorization: Bearer $BOT_TOKEN" \
                -H "Accept: application/vnd.github+json" \
                -H "X-GitHub-Api-Version: 2022-11-28" \
                -d '{"state": "closed"}' \
                > /dev/null || true
            echo "  Closed issue #$ISSUE_NUM"
        else
            echo "  Commented on issue #$ISSUE_NUM (status: $STATUS)"
        fi
    fi

    rm -f "$RESPONSE_FILE"
done

# ── Step 7: Push ──
echo ""
echo "→ Pushing..."
git push || echo "  Push failed (maybe no remote or auth issue)"

echo ""
echo "=== Day $DAY, Session $SESSION complete ==="
JOURNAL_ENTRY=$(awk '/^## Day '"$DAY"', Session '"$SESSION"'/{found=1; next} found && /^## Day/{exit} found{print}' JOURNAL.md | head -5 | tr '\n' ' ')
tg_notify "✅ *Axonix* — Day $DAY, Session $SESSION complete
${JOURNAL_ENTRY:-No journal entry written.}"
