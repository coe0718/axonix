# EVOLVE_PROPOSED.md — Operator Action Required

This file contains proposed changes to `scripts/evolve.sh`, which is READ-ONLY
inside the agent container. The operator must apply these manually.

---

## Proposal 1: Fix METRICS.md stub row format (Issue #57)

**Problem:** Three places in `evolve.sh` write METRICS.md rows that omit the
`Session` column, producing 10-column rows instead of the expected 11-column
rows. The METRICS.md table header has had a `Session` column since Day 8 S5,
but the shell write points were never updated.

**Files to edit:** `scripts/evolve.sh`

### Fix 1a — Pre-session stub (line ~149)

CURRENT:
```bash
    echo "| $DAY | $DATE | ~?k | ${TEST_COUNT_PRE:-?} | ? | ? | ? | ? | ? | Day $DAY S$SESSION — in progress |" >> METRICS.md
```

REPLACE WITH:
```bash
    echo "| $DAY | S$SESSION | $DATE | ~?k | ${TEST_COUNT_PRE:-?} | ? | ? | ? | ? | ? | Day $DAY S$SESSION — in progress |" >> METRICS.md
```

### Fix 1b — Post-session stub replacement (line ~354)

CURRENT:
```bash
    echo "| $DAY | $DATE | ~?k | ${TEST_COUNT:-?} | 0 | ${FILES_CHANGED:-?} | ${LINES_ADDED:-0} | ${LINES_REMOVED:-0} | yes | Day $DAY S$SESSION |" >> METRICS.md
```

REPLACE WITH:
```bash
    echo "| $DAY | S$SESSION | $DATE | ~?k | ${TEST_COUNT:-?} | 0 | ${FILES_CHANGED:-?} | ${LINES_ADDED:-0} | ${LINES_REMOVED:-0} | yes | Day $DAY S$SESSION |" >> METRICS.md
```

### Fix 1c — Fallback row (line ~358)

CURRENT:
```bash
    echo "| $DAY | $DATE | ~?k | ${TEST_COUNT:-?} | 0 | ${FILES_CHANGED:-?} | ${LINES_ADDED:-0} | ${LINES_REMOVED:-0} | yes | Day $DAY S$SESSION — auto-generated (agent missed wrap-up) |" >> METRICS.md
```

REPLACE WITH:
```bash
    echo "| $DAY | S$SESSION | $DATE | ~?k | ${TEST_COUNT:-?} | 0 | ${FILES_CHANGED:-?} | ${LINES_ADDED:-0} | ${LINES_REMOVED:-0} | yes | Day $DAY S$SESSION — auto-generated (agent missed wrap-up) |" >> METRICS.md
```

### Fix 1d — Phase 4c prompt template (line ~247)

CURRENT:
```
  | $DAY | $DATE | ~?k | <tests passed> | 0 | ? | ? | ? | yes | Day $DAY S$SESSION — in progress |
```

REPLACE WITH:
```
  | $DAY | S$SESSION | $DATE | ~?k | <tests passed> | 0 | ? | ? | ? | yes | Day $DAY S$SESSION — in progress |
```

**Verification after applying:** Run the next session and confirm the new stub
row in METRICS.md has 11 columns with `S<N>` as column 2. Also run:
```bash
python3 scripts/analyze_metrics.py
```
No `WARNING: METRICS.md line … appears malformed` lines should appear on stderr.

---

## Proposal 2: Add morning brief to evolve.sh (Issue #59)

**Problem:** The `--brief-telegram` flag was implemented in G-031 (Day 7 S5)
and works when called directly, but it was never wired into `evolve.sh`. As a
result, no morning brief is sent to Telegram at the start of each day.

**Files to edit:** `scripts/evolve.sh`

### Fix 2a — Add morning brief block after the session-start Telegram notify

Find this line (line ~49):
```bash
tg_notify "🤖 *Axonix* — Day $DAY, Session $SESSION starting"
```

Add the following block IMMEDIATELY AFTER that line:

```bash
# ── Morning brief (first session of each day only) ──
if [ "$SESSION" = "1" ] && [ -n "${TELEGRAM_BOT_TOKEN:-}" ] && [ -n "${TELEGRAM_CHAT_ID:-}" ]; then
    echo "→ Sending morning brief to Telegram..."
    ./target/release/axonix --brief-telegram || true
    echo "  Morning brief sent."
fi
```

**Note:** The block is guarded by `SESSION = "1"` so it only fires at the first
session of each calendar day, not on subsequent sessions. It is also guarded by
the presence of both `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID` so it silently
skips if the environment is not configured.

**Verification after applying:** On the next Day N, Session 1 run, you should
receive a morning brief Telegram message before the session plan prompt is sent.

---

*Written by Axonix agent — Day 9 S1. Operator: please apply both proposals and
delete this file (or rename it to EVOLVE_APPLIED.md) once done.*
