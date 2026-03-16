#!/bin/bash
# scripts/tweet_session.sh — Post a session announcement tweet as Axonix.
#
# Called by evolve.sh at session start/end, or manually.
# Uses the Rust axonix binary's built-in Twitter client (TWITTER_* env vars).
#
# Usage:
#   DAY=3 SESSION=1 TITLE="Fix bot identity" ./scripts/tweet_session.sh
#
# Required env vars (same as axonix binary):
#   TWITTER_API_KEY, TWITTER_API_SECRET, TWITTER_ACCESS_TOKEN, TWITTER_ACCESS_SECRET

set -euo pipefail

DAY="${DAY:-?}"
SESSION="${SESSION:-?}"
TITLE="${TITLE:-}"

if [ -z "$TITLE" ]; then
    # Try to extract from JOURNAL.md
    TITLE=$(grep -m1 "^## Day ${DAY}, Session ${SESSION}" JOURNAL.md 2>/dev/null | sed "s/^## Day ${DAY}, Session ${SESSION} — //" || echo "")
fi

if [ -z "$TITLE" ]; then
    echo "  No title found for Day $DAY Session $SESSION — skipping tweet."
    exit 0
fi

# Check for required Twitter credentials
if [ -z "${TWITTER_API_KEY:-}" ] || [ -z "${TWITTER_API_SECRET:-}" ] || \
   [ -z "${TWITTER_ACCESS_TOKEN:-}" ] || [ -z "${TWITTER_ACCESS_SECRET:-}" ]; then
    echo "  Twitter credentials not set — skipping tweet."
    exit 0
fi

echo "  Tweeting session announcement..."
python3 - <<PYEOF
import os, sys, time, base64, hashlib, hmac, json, urllib.request, urllib.parse

def percent_encode(s):
    return urllib.parse.quote(s, safe='')

def hmac_sha1(key, msg):
    return hmac.new(key.encode(), msg.encode(), digestmod='sha1').digest()

api_key = os.environ['TWITTER_API_KEY']
api_secret = os.environ['TWITTER_API_SECRET']
access_token = os.environ['TWITTER_ACCESS_TOKEN']
access_secret = os.environ['TWITTER_ACCESS_SECRET']

day = os.environ.get('DAY', '?')
session = os.environ.get('SESSION', '?')
title = os.environ.get('TITLE', '')

# Format tweet
prefix = f"axonix Day {day}, Session {session}: "
suffix = " — axonix.dev"
max_title_chars = 280 - len(prefix) - len(suffix)
if len(title) > max_title_chars:
    title = title[:max_title_chars-1] + "…"
tweet_text = prefix + title + suffix

# OAuth 1.0a
timestamp = str(int(time.time()))
nonce = base64.b64encode(os.urandom(16)).decode().strip('=+/')[:32]

url = "https://api.twitter.com/2/tweets"
oauth_params = [
    ("oauth_consumer_key", api_key),
    ("oauth_nonce", nonce),
    ("oauth_signature_method", "HMAC-SHA1"),
    ("oauth_timestamp", timestamp),
    ("oauth_token", access_token),
    ("oauth_version", "1.0"),
]

all_params = sorted([(percent_encode(k), percent_encode(v)) for k, v in oauth_params])
param_string = "&".join(f"{k}={v}" for k, v in all_params)
base_string = f"POST&{percent_encode(url)}&{percent_encode(param_string)}"
signing_key = f"{percent_encode(api_secret)}&{percent_encode(access_secret)}"

sig = base64.b64encode(hmac.new(signing_key.encode(), base_string.encode(), digestmod='sha1').digest()).decode()
oauth_params.append(("oauth_signature", sig))

auth_header = "OAuth " + ", ".join(
    f'{percent_encode(k)}="{percent_encode(v)}"' for k, v in oauth_params
)

body = json.dumps({"text": tweet_text}).encode()
req = urllib.request.Request(
    url,
    data=body,
    headers={
        "Authorization": auth_header,
        "Content-Type": "application/json",
        "User-Agent": "axonix-bot/1.0",
    },
    method="POST"
)
try:
    with urllib.request.urlopen(req) as resp:
        data = json.loads(resp.read())
        tweet_id = data.get("data", {}).get("id", "(unknown)")
        print(f"  ✓ Tweeted: {tweet_text[:60]}... (id: {tweet_id})")
except urllib.error.HTTPError as e:
    print(f"  ✗ Tweet failed: HTTP {e.code} — {e.read().decode()}")
PYEOF
