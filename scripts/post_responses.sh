#!/bin/bash
# scripts/post_responses.sh — Post issue responses as axonix-bot.
#
# This script replaces the inline `gh issue comment` block in evolve.sh.
# It uses AXONIX_BOT_TOKEN (or GH_TOKEN as fallback) to post comments via the
# GitHub REST API directly, so comments appear as axonix-bot — not as the repo owner.
#
# Usage (called by evolve.sh):
#   DAY=3 SESSION=1 REPO=coe0718/axonix ./scripts/post_responses.sh
#
# Reads: ISSUE_RESPONSE*.md files in the working directory.
# Deletes each response file after processing.

set -euo pipefail

DAY="${DAY:-?}"
SESSION="${SESSION:-?}"
REPO="${REPO:-coe0718/axonix}"

# Use AXONIX_BOT_TOKEN first (posts as axonix-bot), fall back to GH_TOKEN (posts as owner).
COMMENT_TOKEN="${AXONIX_BOT_TOKEN:-${GH_TOKEN:-}}"

if [ -z "$COMMENT_TOKEN" ]; then
    echo "  Warning: no AXONIX_BOT_TOKEN or GH_TOKEN set — cannot post issue responses."
    exit 0
fi

for RESPONSE_FILE in ISSUE_RESPONSE*.md; do
    [ -f "$RESPONSE_FILE" ] || continue
    echo ""
    echo "→ Posting issue response from $RESPONSE_FILE..."

    ISSUE_NUM=$(grep "^issue_number:" "$RESPONSE_FILE" | awk '{print $2}' || true)
    STATUS=$(grep "^status:" "$RESPONSE_FILE" | awk '{print $2}' || true)
    # Extract everything after "comment:" (may be multiline)
    COMMENT=$(awk '/^comment:/{found=1; sub(/^comment: ?/, ""); print; next} found{print}' "$RESPONSE_FILE" || true)

    if [ -z "$ISSUE_NUM" ]; then
        echo "  Warning: no issue_number in $RESPONSE_FILE, skipping."
        rm -f "$RESPONSE_FILE"
        continue
    fi

    COMMENT_BODY="🤖 **Day ${DAY}, Session ${SESSION}**

${COMMENT}

Commit: $(git rev-parse --short HEAD)"

    # Post comment via GitHub REST API as axonix-bot
    BODY_JSON=$(python3 -c "import json,sys; print(json.dumps({'body': sys.stdin.read()}))" <<< "$COMMENT_BODY")
    HTTP_STATUS=$(curl -sf -o /tmp/gh_comment_response.json -w "%{http_code}" \
        -X POST \
        -H "Authorization: Bearer $COMMENT_TOKEN" \
        -H "Accept: application/vnd.github+json" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        -H "User-Agent: axonix-bot/1.0" \
        -d "$BODY_JSON" \
        "https://api.github.com/repos/${REPO}/issues/${ISSUE_NUM}/comments" || echo "000")

    if [ "$HTTP_STATUS" -ge 200 ] && [ "$HTTP_STATUS" -lt 300 ] 2>/dev/null; then
        COMMENT_URL=$(python3 -c "import json; d=json.load(open('/tmp/gh_comment_response.json')); print(d.get('html_url','(unknown)'))" 2>/dev/null || echo "(unknown)")
        echo "  ✓ Posted comment on #${ISSUE_NUM}: $COMMENT_URL"
    else
        echo "  ✗ Comment post returned HTTP $HTTP_STATUS — falling back to gh CLI"
        if command -v gh &>/dev/null; then
            gh issue comment "$ISSUE_NUM" --repo "$REPO" --body "$COMMENT_BODY" || true
        fi
    fi

    # Close issue if status is "fixed"
    if [ "$STATUS" = "fixed" ]; then
        CLOSE_STATUS=$(curl -sf -o /dev/null -w "%{http_code}" \
            -X PATCH \
            -H "Authorization: Bearer $COMMENT_TOKEN" \
            -H "Accept: application/vnd.github+json" \
            -H "X-GitHub-Api-Version: 2022-11-28" \
            -H "User-Agent: axonix-bot/1.0" \
            -d '{"state":"closed"}' \
            "https://api.github.com/repos/${REPO}/issues/${ISSUE_NUM}" || echo "000")
        if [ "$CLOSE_STATUS" -ge 200 ] && [ "$CLOSE_STATUS" -lt 300 ] 2>/dev/null; then
            echo "  ✓ Closed issue #${ISSUE_NUM}"
        else
            echo "  ✗ Close returned HTTP $CLOSE_STATUS"
            if command -v gh &>/dev/null; then
                gh issue close "$ISSUE_NUM" --repo "$REPO" || true
            fi
        fi
    else
        echo "  Commented on issue #${ISSUE_NUM} (status: ${STATUS})"
    fi

    rm -f "$RESPONSE_FILE"
done
