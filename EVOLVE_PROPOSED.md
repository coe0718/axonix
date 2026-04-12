# EVOLVE_PROPOSED.md

Proposals for changes to evolve.sh or operator-owned config files.
Do NOT apply these yourself — the operator reviews and applies them.

---

## Proposal 1 — Truncate METRICS.md injection to last 20 rows (Issue #111)

**Problem:** evolve.sh currently reads and injects the full METRICS.md file. As of Day 29, METRICS.md has 100+ rows (~14.7 KB). The injected RECENT_METRICS block shows "last 5 sessions" but the raw file is still fully read. This wastes 5–8k tokens per session.

**Proposed change to evolve.sh:**

Instead of injecting the full file, inject only the header row plus the 20 most recent data rows. Archive older rows to `METRICS_ARCHIVE.md`.

Suggested shell snippet (replace the current METRICS.md injection):

```bash
# Inject last 20 METRICS rows only (archive the rest)
METRICS_HEADER=$(head -2 METRICS.md)  # header + separator line
METRICS_TAIL=$(grep "^| [0-9]" METRICS.md | tail -20)
METRICS_INJECTION="${METRICS_HEADER}
${METRICS_TAIL}"

# Archive everything older than last 20 rows to METRICS_ARCHIVE.md
METRICS_OLD=$(grep "^| [0-9]" METRICS.md | head -n -20)
if [ -n "$METRICS_OLD" ]; then
    echo "$METRICS_OLD" >> METRICS_ARCHIVE.md
    # Rewrite METRICS.md with only last 20 rows
    echo "$METRICS_HEADER" > METRICS.md
    echo "$METRICS_TAIL" >> METRICS.md
fi
```

**Expected saving:** 5–8k tokens per session.

**Proposed by:** Axonix, Day 29 S1 (2026-04-12)
**Related issue:** #111
