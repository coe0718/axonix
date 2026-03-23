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

# Warn if a pending operator proposal exists
if [ -f EVOLVE_PROPOSED.md ]; then
    echo "  ⚠️  EVOLVE_PROPOSED.md exists — operator action required before this is applied."
    tg_notify "⚠️ *Axonix* — EVOLVE\_PROPOSED.md has pending changes waiting for operator review\. Day $DAY, Session $SESSION starting anyway\."
fi

tg_notify "🤖 *Axonix* — Day $DAY, Session $SESSION starting"

# ── Morning brief (first session of each day only) ──
if [ "$SESSION" = "1" ] && [ -n "${TELEGRAM_TOKEN:-}" ] && [ -n "${TELEGRAM_CHAT_ID:-}" ]; then
    echo "→ Sending morning brief to Telegram..."
    cargo run --bin axonix --quiet -- --brief-telegram || true
    echo "  Morning brief sent."
fi

echo "Model: $MODEL"
echo ""

# Snapshot METRICS.md row count before the session so we can detect if the
# agent forgot to write its row during wrap-up.
METRICS_ROWS_BEFORE=$(grep -cE "^\| [0-9]" METRICS.md 2>/dev/null || echo "0")

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

    # Append recent discussions (last 5, with their comments) so Axonix can read and reply
    echo "" >> "$ISSUES_FILE"
    echo "## Recent Discussions" >> "$ISSUES_FILE"
    gh api graphql -f query='
    query($owner: String!, $name: String!, $limit: Int!) {
      repository(owner: $owner, name: $name) {
        discussions(first: $limit, orderBy: {field: UPDATED_AT, direction: DESC}) {
          nodes {
            number title body url author { login }
            comments(first: 5) { nodes { body author { login } } }
          }
        }
      }
    }' -f owner="$(echo $REPO | cut -d/ -f1)" \
       -f name="$(echo $REPO | cut -d/ -f2)" \
       -F limit=5 2>/dev/null | python3 -c "
import json, sys
data = json.load(sys.stdin)
nodes = data.get('data', {}).get('repository', {}).get('discussions', {}).get('nodes', [])
if not nodes:
    print('No recent discussions.')
else:
    for d in nodes:
        print(f'### Discussion #{d[\"number\"]}: {d[\"title\"]}')
        print(f'URL: {d[\"url\"]}')
        print(f'Author: {d[\"author\"][\"login\"]}')
        print()
        print(d['body'][:500])
        comments = d.get('comments', {}).get('nodes', [])
        if comments:
            print()
            print('**Comments:**')
            for c in comments:
                print(f'- **{c[\"author\"][\"login\"]}**: {c[\"body\"][:200]}')
        print()
" >> "$ISSUES_FILE" 2>/dev/null || echo "No discussions fetched." >> "$ISSUES_FILE"
    DISC_COUNT=$(grep -c '^### Discussion' "$ISSUES_FILE" 2>/dev/null || echo 0)
    echo "  ${DISC_COUNT} discussions loaded."
else
    echo "  gh CLI not available. Skipping issue fetch."
    echo "No issues available (gh CLI not installed)." > "$ISSUES_FILE"
fi
echo ""

# ── Step 3: Prepare context injections ──
# Token budget: Sonnet has 200K tokens. Pre-loading all src/ (~97K tokens) and
# docs/index.html (~13K tokens) every session left only ~90K for actual work.
# Instead: inject summaries inline and let Axonix read specific files as needed.

# Journal: last 3 entries only
RECENT_JOURNAL=$(python3 -c "
import re, sys
text = open('JOURNAL.md').read()
entries = re.split(r'(?=^## Day )', text, flags=re.MULTILINE)
entries = [e for e in entries if e.strip().startswith('## Day')]
recent = entries[:3]
print('# Journal (last 3 entries)\n')
print('\n'.join(recent))
" 2>/dev/null || head -60 JOURNAL.md 2>/dev/null || echo "No journal yet.")

# Metrics: last 5 rows only (full table is ~40 rows, most is old history)
RECENT_METRICS=$(grep "^|" METRICS.md 2>/dev/null | grep -v "^| Day\|^|---" | tail -5 || echo "No metrics yet.")

# Snapshot HEAD before session so we can diff commits afterward
SESSION_START_SHA=$(git rev-parse HEAD 2>/dev/null || echo "")

# ── Step 3b: Write METRICS.md stub before agent runs (crash safety) ──
# If the session crashes before Phase 4c, this row ensures a partial record exists.
# Step 5a will replace "in progress" with real stats at wrap-up.
if ! grep -qE "^\| $DAY \| S$SESSION \|" METRICS.md 2>/dev/null; then
    TEST_COUNT_PRE=$(cargo test --quiet 2>/dev/null | grep "test result" | grep -oE "[0-9]+ passed" | grep -oE "[0-9]+" | head -1 || echo "?")
    echo "| $DAY | S$SESSION | $DATE | ~?k | ${TEST_COUNT_PRE:-?} | ? | ? | ? | ? | ? | Day $DAY S$SESSION — in progress |" >> METRICS.md
    echo "  METRICS.md stub row written."
fi

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
6. COMMIT_CONVENTIONS.md — your rules for commit messages (follow these every session)
7. src/lib.rs and src/main.rs — your architecture overview only.
   Read other src/ files only when directly relevant to your current task.
   Do NOT read all .rs files upfront — the codebase is ~400KB and will exhaust your context.
8. ISSUES_TODAY.md — community requests and recent discussions (with comments)
9. docs/index.html — ONLY if you are making dashboard layout changes this session.
   Otherwise skip it entirely.

Your recent journal and metrics are injected below — no need to read those files.

=== RECENT JOURNAL (last 3 entries) ===
$RECENT_JOURNAL
=== END JOURNAL ===

=== RECENT METRICS (last 5 sessions) ===
| Day | Session | Date | Tokens | Tests | Failed | Files | +Lines | -Lines | Committed | Notes |
$RECENT_METRICS
=== END METRICS ===

=== PHASE 1: Self-Assessment ===

Read src/lib.rs and src/main.rs for architectural overview.
Read other src/ files only if you need to check or modify them.
Check for:
- Crash bugs or panics (especially on edge-case input)
- Missing error handling or silent failures
- Any capability in CAPABILITIES.md you haven't used yet

Check docker-compose.yml carefully:
- Every env var in CAPABILITIES.md must be in the axonix environment: section
- If anything is missing, add it NOW before doing anything else

Check GOALS.md carefully:
- Every [x] goal: verify the feature actually exists in the code, not just in the journal
- Every active goal: is it still relevant? Update or close it if not
- If the Active section is empty, promote at least one item from Backlog

Run: cargo build && cargo test 2>&1 | grep -E "(^test result|FAILED|^error\[)"
Report the exact test count from the summary line. Do not list individual passing tests.

=== PHASE 2: Review Community Issues ===

Read ISSUES_TODAY.md. It contains two sections:
1. GitHub Issues labeled "agent-input" — real people asking you to improve.
   Issues with more 👍 reactions should be prioritized higher.
2. Recent GitHub Discussions — community conversation about Axonix.
   Read each discussion and its comments. If someone asked a question or left
   feedback in a discussion, reply to it using gh api graphql with the
   addDiscussionComment mutation, or use the reply_to_discussion() method
   in GitHubClient (src/github.rs). Discussion node IDs are in the URL as
   the numeric ID — fetch them via the GraphQL API if needed.
   Acknowledge every unanswered community question before moving on.

=== PHASE 3: Decide ===

Choose what to work on this session. Prioritize:
1. Self-discovered crash or data loss bug
2. Community issue with most 👍 (if actionable today)
3. Active goal from GOALS.md
4. Self-discovered UX friction or missing error handling
5. Whatever you think will make you most useful to the person running you

=== PHASE 4: Journal + State Commit (DO THIS BEFORE ANY CODE) ===

This entire phase must be completed and committed before touching src/.
If the session ends early, these files must already be saved.

Step 4a — Write journal entry at the TOP of JOURNAL.md:
## Day $DAY, Session $SESSION — [title]
[2-4 sentences: what you plan to do, why you chose it]

Step 4b — Update GOALS.md right now:
- If Active section is empty, promote one item from Backlog
- For each goal you plan to complete this session, leave it [ ] but confirm it is in Active
- For any goal you already verified is done (in code, not just journal), mark [x] now
- Do not wait until Phase 7 — if the session ends early, GOALS.md must already reflect reality

Step 4c — Write a METRICS.md stub row right now:
Run: cargo test 2>&1 | grep "^test result" | head -1
Write this row (fill in actual test count, leave ? for stats filled in later):
  | $DAY | S$SESSION | $DATE | ~?k | <tests passed> | 0 | ? | ? | ? | yes | Day $DAY S$SESSION — in progress |

Step 4d — Commit all three together:
  git add JOURNAL.md GOALS.md METRICS.md && git commit -m "docs(journal): Day $DAY Session $SESSION — [title]"

=== PHASE 5: Issue Response ===

For each community issue you are addressing, write a separate response file:
ISSUE_RESPONSE_<N>.md (e.g. ISSUE_RESPONSE_5.md for issue #5)

Format:
issue_number: [N]
status: fixed|partial|wontfix
comment: [your response — what you did, why, what changed]

Write all response files before starting implementation.

=== PHASE 6: Implement ===

Use the implementer tool to do all coding work. Do NOT write code yourself.

Call the implementer tool with a detailed plan:
- What files to read (only the ones needed)
- What change to make and why
- What tests to write or run
- What goal ID or issue number this addresses

The implementer runs in its own fresh context window with 25 turns.
It will read files, write code, run tests, and commit — then return a summary.

If the implementer reports a failure, you may call it again with a revised plan.
Do not attempt to implement changes in this context.

=== PHASE 7: Wrap Up ===

GOALS.md and METRICS.md stub were already written in Phase 4.
This phase finalises them with real numbers.

- Update the METRICS.md stub row you wrote in Phase 4:
    Find the "in progress" row and replace it with actual values:
    | $DAY | $DATE | ~Xk | <tests passed> | 0 | <files changed> | <lines added> | <lines removed> | yes | <one line summary> |
    Run cargo test for exact count. Use git diff --stat HEAD~N HEAD to get file/line counts.
- Cross-check GOALS.md: for every goal mentioned as done in your journal, verify it is [x].
- If you added a new environment variable, add it to docker-compose.yml, .env.example, CAPABILITIES.md.
- Verify: cargo build && cargo test

== EVOLVE_PROPOSED.md RULES ==

If you need to propose a change to evolve.sh or any operator-only file:
- If EVOLVE_PROPOSED.md does NOT exist: create it with your proposal as "## Proposal 1"
- If EVOLVE_PROPOSED.md ALREADY EXISTS: append your proposal as the next numbered section
- NEVER overwrite or delete existing proposals — the operator may not have applied them yet

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
            -e 's/GH_TOKEN=[^ ]*/GH_TOKEN=[REDACTED]/g' \
            -e 's/AXONIX_BOT_TOKEN=[^ ]*/AXONIX_BOT_TOKEN=[REDACTED]/g' \
            -e 's/TELEGRAM_BOT_TOKEN=[^ ]*/TELEGRAM_BOT_TOKEN=[REDACTED]/g' \
            -e 's/TELEGRAM_TOKEN=[^ ]*/TELEGRAM_TOKEN=[REDACTED]/g' \
            -e 's/TELEGRAM_CHAT_ID=[^ ]*/TELEGRAM_CHAT_ID=[REDACTED]/g' \
            -e 's/BLUESKY_IDENTIFIER=[^ ]*/BLUESKY_IDENTIFIER=[REDACTED]/g' \
            -e 's/BLUESKY_APP_PASSWORD=[^ ]*/BLUESKY_APP_PASSWORD=[REDACTED]/g')
        curl -sf -X POST "$STREAM_URL" --data-binary "$line" 2>/dev/null || true
    done || true

rm -f "$PROMPT_FILE"

echo ""
echo "→ Session complete. Checking results..."

# ── Step 5: Verify build and handle leftovers ──
if cargo build --quiet 2>/dev/null && cargo test --quiet 2>/dev/null; then
    echo "  Build: PASS"
else
    echo "  Build: FAIL — reverting src/ to pre-session state"
    if [ -n "$SESSION_START_SHA" ]; then
        git checkout "$SESSION_START_SHA" -- src/
        git add src/
        git commit -m "revert(src): Day $DAY Session $SESSION — build/test failure, restored pre-session src/"
    else
        git checkout -- src/
    fi
fi

# DAY_COUNT already updated at session start

# ── Step 5a: Metrics fallback — fill in real stats ──
# If agent wrote a stub row ("in progress"), replace it with real numbers.
# If agent wrote nothing, append a new row.
TEST_COUNT=$(cargo test --quiet 2>/dev/null | grep "test result" | grep -oE "[0-9]+ passed" | grep -oE "[0-9]+" | head -1 || echo "?")
DIFF_STAT=$(git diff --stat "${SESSION_START_SHA}..HEAD" 2>/dev/null | tail -1 || echo "")
FILES_CHANGED=$(echo "$DIFF_STAT" | grep -oE "[0-9]+ file" | grep -oE "[0-9]+" || echo "?")
LINES_ADDED=$(echo "$DIFF_STAT" | grep -oE "[0-9]+ insertion" | grep -oE "[0-9]+" || echo "0")
LINES_REMOVED=$(echo "$DIFF_STAT" | grep -oE "[0-9]+ deletion" | grep -oE "[0-9]+" || echo "0")
METRICS_ROWS_AFTER=$(grep -cE "^\| [0-9]" METRICS.md 2>/dev/null || echo "0")

if grep -q "in progress" METRICS.md 2>/dev/null; then
    # Replace the stub row with real values
    sed -i "s|.* in progress .*|| " METRICS.md 2>/dev/null || true
    sed -i '/^$/d' METRICS.md 2>/dev/null || true
    echo "| $DAY | S$SESSION | $DATE | ~?k | ${TEST_COUNT:-?} | 0 | ${FILES_CHANGED:-?} | ${LINES_ADDED:-0} | ${LINES_REMOVED:-0} | yes | Day $DAY S$SESSION |" >> METRICS.md
    echo "  Metrics stub row replaced with real stats."
elif [ "$METRICS_ROWS_AFTER" -le "$METRICS_ROWS_BEFORE" ]; then
    echo "  WARNING: METRICS.md not updated this session — appending fallback row"
    echo "| $DAY | S$SESSION | $DATE | ~?k | ${TEST_COUNT:-?} | 0 | ${FILES_CHANGED:-?} | ${LINES_ADDED:-0} | ${LINES_REMOVED:-0} | yes | Day $DAY S$SESSION — auto-generated (agent missed wrap-up) |" >> METRICS.md
    echo "  Fallback metrics row appended."
fi

# ── Step 5b: Auto-mark completed goals from session commit messages ──
# If Axonix referenced a G-ID in a commit message, it's done — mark it.
if [ -n "$SESSION_START_SHA" ]; then
    SESSION_COMMITS=$(git log --format="%s%n%b" "${SESSION_START_SHA}..HEAD" 2>/dev/null || echo "")
    if [ -n "$SESSION_COMMITS" ]; then
        COMPLETED_GOALS=$(echo "$SESSION_COMMITS" | grep -oE '\bG-[0-9]+\b' | sort -u)
        for GOAL_ID in $COMPLETED_GOALS; do
            if grep -qE "^\- \[ \] \[${GOAL_ID}\]" GOALS.md 2>/dev/null; then
                sed -i "s/^- \[ \] \[${GOAL_ID}\]/- [x] [${GOAL_ID}]/" GOALS.md
                echo "  Auto-marked ${GOAL_ID} done in GOALS.md (referenced in session commits)"
            fi
        done
    fi

    # If Active section now has no unchecked goals, promote one from Backlog
    ACTIVE_OPEN=$(awk '/^## Active/{f=1} /^## Backlog/{f=0} f && /^\- \[ \]/' GOALS.md | wc -l)
    if [ "$ACTIVE_OPEN" -eq 0 ]; then
        # Find first unchecked backlog item and move it to active section
        BACKLOG_GOAL=$(awk '/^## Backlog/{f=1} f && /^\- \[ \]/{print; exit}' GOALS.md)
        if [ -n "$BACKLOG_GOAL" ]; then
            # Remove from backlog, add to active
            GOAL_TEXT=$(echo "$BACKLOG_GOAL" | sed 's/\[/\\[/g; s/\]/\\]/g')
            # Insert after "## Active" line
            sed -i "/^## Active$/a\\${BACKLOG_GOAL}" GOALS.md
            # Remove from backlog (first occurrence)
            ESCAPED=$(echo "$BACKLOG_GOAL" | sed 's/[[\.*^$()+?{|]/\\&/g')
            sed -i "0,/^## Backlog/! { /^${ESCAPED}$/{ s/.*/GOAL_PLACEHOLDER_DELETE/; } }" GOALS.md 2>/dev/null || true
            sed -i '/^GOAL_PLACEHOLDER_DELETE$/d' GOALS.md
            echo "  Promoted backlog goal to Active (Active was empty after session)"
        fi
    fi
fi

# ── Step 5a-ii: Write cycle_summary.json via --write-summary CLI flag (G-033/G-035) ──
# Uses real git log, GOALS.md state, and test counts — no fragile shell extraction.
cargo run --bin axonix --quiet -- --write-summary "Day ${DAY}, Session ${SESSION}" 2>/dev/null \
    && echo "  Cycle summary written to .axonix/cycle_summary.json" \
    || echo "  Cycle summary write failed (non-fatal)"

# ── Step 5b-ii: Trim completed goal detail lines to keep GOALS.md lean ──
# Strip indented continuation lines under [x] goals; collapse double blank lines.
python3 - <<'PYEOF'
import re, itertools

with open('GOALS.md') as f:
    lines = f.readlines()

out = []
in_done = False
for line in lines:
    if re.match(r'^- \[x\] ', line):
        in_done = True
        out.append(line)
    elif in_done and line.startswith('  '):
        pass  # drop continuation lines under completed goals
    else:
        in_done = False
        out.append(line)

# Collapse consecutive blank lines to one
collapsed = []
for is_blank, group in itertools.groupby(out, key=lambda l: l.strip() == ''):
    if is_blank:
        collapsed.append('\n')
    else:
        collapsed.extend(group)

with open('GOALS.md', 'w') as f:
    f.writelines(collapsed)
PYEOF

# Rebuild website
echo "→ Rebuilding website..."
python3 scripts/build_site.py
echo "  Site rebuilt."

# ── Step 5c: Post session update to Bluesky ──
if [ -n "${BLUESKY_IDENTIFIER:-}" ] && [ -n "${BLUESKY_APP_PASSWORD:-}" ]; then
    echo "→ Posting session update to Bluesky..."
    JOURNAL_TITLE=$(grep "^## Day $DAY, Session $SESSION" JOURNAL.md | head -1 | sed 's/^## Day [0-9]*, Session [0-9]* — //')
    if [ -n "$JOURNAL_TITLE" ]; then
        POST_TEXT="axonix Day $DAY, Session $SESSION: $JOURNAL_TITLE — axonix.live"
        POST_TEXT=$(echo "$POST_TEXT" | cut -c1-300)
        cargo run --bin axonix --quiet -- --bluesky-post "$POST_TEXT" && echo "  Bluesky post sent." || echo "  Bluesky post failed (non-fatal)"
    else
        echo "  No journal title found — skipping Bluesky post."
    fi
fi

# ── Step 5d: Post session journal entry as GitHub Discussion (deduplicated) ──
if [ -n "${AXONIX_BOT_TOKEN:-}${GH_TOKEN:-}" ]; then
    echo "→ Posting session discussion..."
    mkdir -p .axonix
    DISCUSS_FLAG=".axonix/discussed_day_${DAY}_session_${SESSION}"
    JOURNAL_CHECK=$(grep "^## Day $DAY, Session $SESSION" JOURNAL.md | head -1)
    if [ -f "$DISCUSS_FLAG" ]; then
        echo "  Discussion already posted for Day $DAY Session $SESSION — skipping."
    elif [ -n "$JOURNAL_CHECK" ]; then
        if cargo run --bin axonix --quiet -- --discuss; then
            touch "$DISCUSS_FLAG"
            echo "  Discussion posted."
        else
            echo "  Discussion post failed (non-fatal)"
        fi
    else
        echo "  No journal entry for Day $DAY Session $SESSION — skipping discussion."
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

# ── Step 7: Enforce non-empty commit body before push ──
LAST_COMMIT_BODY=$(git log -1 --format="%b" | tr -d '[:space:]')
if [ -z "$LAST_COMMIT_BODY" ]; then
    echo "  WARNING: last commit has no body — amending before push"
    LAST_SUBJECT=$(git log -1 --format="%s")
    COMMIT_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    git commit --amend --no-edit -m "${LAST_SUBJECT}

Auto-generated body: session wrap-up at ${COMMIT_DATE}.
No manual body was provided; amended by evolve.sh to satisfy git-discipline skill."
fi

# ── Step 8: Push ──
echo ""
echo "→ Pushing..."
git push || echo "  Push failed (maybe no remote or auth issue)"

echo ""
echo "=== Day $DAY, Session $SESSION complete ==="
JOURNAL_ENTRY=$(awk '/^## Day '"$DAY"', Session '"$SESSION"'/{found=1; next} found && /^## Day/{exit} found{print}' JOURNAL.md | head -5 | tr '\n' ' ')
tg_notify "✅ *Axonix* — Day $DAY, Session $SESSION complete
${JOURNAL_ENTRY:-No journal entry written.}"
