#!/bin/bash
# scripts/evolve.sh — One evolution cycle. Run daily via GitHub Actions or manually.
#
# Usage:
#   ANTHROPIC_API_KEY=sk-... ./scripts/evolve.sh
#
# Environment:
#   ANTHROPIC_API_KEY  — required
#   REPO               — GitHub repo (default: coe0718/axonix)
#   MODEL              — LLM model (default: claude-opus-4-6)
#   TIMEOUT            — Max session time in seconds (default: 600)

set -euo pipefail

tg_notify() {
    if [ -n "${TELEGRAM_TOKEN:-}" ] && [ -n "${TELEGRAM_CHAT_ID:-}" ]; then
        curl -s -X POST "https://api.telegram.org/bot$TELEGRAM_TOKEN/sendMessage" \
            -d "chat_id=$TELEGRAM_CHAT_ID&text=$1&parse_mode=Markdown" > /dev/null || true
    fi
}

REPO="${REPO:-coe0718/axonix}"
MODEL="${MODEL:-claude-opus-4-6}"
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
echo "Timeout: ${TIMEOUT}s"
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
        --limit 10 \
        --json number,title,body,labels,reactionGroups \
        > /tmp/issues_raw.json 2>/dev/null || true

    python3 scripts/format_issues.py /tmp/issues_raw.json > "$ISSUES_FILE" 2>/dev/null || echo "No issues found." > "$ISSUES_FILE"
    echo "  $(grep -c '^### Issue' "$ISSUES_FILE" 2>/dev/null || echo 0) issues loaded."
else
    echo "  gh CLI not available. Skipping issue fetch."
    echo "No issues available (gh CLI not installed)." > "$ISSUES_FILE"
fi
echo ""

# ── Step 3: Prepare journal tail (last 10 entries for context) ──
RECENT_JOURNAL=$(head -200 JOURNAL.md 2>/dev/null || echo "No journal yet.")

# ── Step 4: Run evolution session ──
echo "→ Starting evolution session..."
echo ""

PROMPT_FILE=$(mktemp)
cat > "$PROMPT_FILE" <<PROMPT
Today is Day $DAY, Session $SESSION ($DATE).

Read these files in this order:
1. IDENTITY.md (who you are and your rules)
2. src/main.rs (your current source code — this is YOU)
3. JOURNAL.md (your recent history — last 10 entries)
4. ISSUES_TODAY.md (community requests)

=== PHASE 1: Self-Assessment ===

Read your own source code carefully. Then try a small task to test
yourself — for example, read a file, edit something, run a command.
Note any friction, bugs, crashes, or missing capabilities.

=== PHASE 2: Review Community Issues ===

Read ISSUES_TODAY.md. These are real people asking you to improve.
Issues with more 👍 reactions should be prioritized higher.

=== PHASE 3: Decide ===

Choose what to work on this session. Prioritize:
1. Self-discovered crash or data loss bug
2. Community issue with most 👍 (if actionable today)
3. Self-discovered UX friction or missing error handling
4. Whatever you think will make you most useful to real developers

=== PHASE 4: Journal ===

Before writing any code, write today's entry at the TOP of JOURNAL.md. Format:
## Day $DAY, Session $SESSION — [title]
[2-4 sentences: what you plan to do, why you chose it]

Then commit it: git add JOURNAL.md && git commit -m "Day $DAY Session $SESSION: journal"

=== PHASE 5: Issue Response ===

If you are working on a community GitHub issue, write to ISSUE_RESPONSE.md now:
issue_number: [N]
status: fixed|partial|wontfix
comment: [your 2-3 sentence response to the person]

=== PHASE 6: Implement ===

For each improvement, follow the evolve skill rules:
- Write a test first if possible
- Use edit_file for surgical changes
- Run cargo build && cargo test after changes
- If build fails, try to fix it. If you can't, revert with: bash git checkout -- src/
- After each successful change, commit: git add -A && git commit -m "Day $DAY Session $SESSION: <short description>"
- Then move on to the next improvement

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

# Commit any remaining uncommitted changes (journal, roadmap, day counter, site, etc.)
git add -A
if ! git diff --cached --quiet; then
    git commit -m "Day $DAY Session $SESSION: session wrap-up"
    echo "  Committed session wrap-up."
else
    echo "  No uncommitted changes remaining."
fi

# ── Step 6: Handle issue response ──
if [ -f ISSUE_RESPONSE.md ]; then
    echo ""
    echo "→ Posting issue response..."
    
    ISSUE_NUM=$(grep "^issue_number:" ISSUE_RESPONSE.md | awk '{print $2}' || true)
    STATUS=$(grep "^status:" ISSUE_RESPONSE.md | awk '{print $2}' || true)
    COMMENT=$(sed -n '/^comment:/,$ p' ISSUE_RESPONSE.md | sed '1s/^comment: //' || true)
    
    if [ -n "$ISSUE_NUM" ] && command -v gh &>/dev/null; then
        gh issue comment "$ISSUE_NUM" \
            --repo "$REPO" \
            --body "🤖 **Day $DAY**

$COMMENT

Commit: $(git rev-parse --short HEAD)" || true

        if [ "$STATUS" = "fixed" ]; then
            gh issue close "$ISSUE_NUM" --repo "$REPO" || true
            echo "  Closed issue #$ISSUE_NUM"
        else
            echo "  Commented on issue #$ISSUE_NUM (status: $STATUS)"
        fi
    fi
    
    rm -f ISSUE_RESPONSE.md
fi

# ── Step 7: Push ──
echo ""
echo "→ Pushing..."
git push || echo "  Push failed (maybe no remote or auth issue)"

echo ""
echo "=== Day $DAY complete ==="
JOURNAL_ENTRY=$(awk '/^## Day '"$DAY"'/{found=1; next} found && /^## Day/{exit} found{print}' JOURNAL.md | head -5 | tr '\n' ' ')
tg_notify "✅ *Axonix* — Day $DAY, Session $SESSION complete
${JOURNAL_ENTRY:-No journal entry written.}"
