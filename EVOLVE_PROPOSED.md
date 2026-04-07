## Proposal 1 — Truncate METRICS.md injection to last 15 rows (Issue #111, G-130)

**Problem:** evolve.sh currently injects the full METRICS.md into every session context.
As of Day 24, METRICS.md has 105+ rows (~14.7 KB). Only the most recent 15 sessions
matter for planning. Injecting all rows wastes 5–8k tokens per session.

**Proposed change to evolve.sh:**

In the section that builds the session prompt (where METRICS.md content is injected),
replace the full file injection with a truncated version:

```bash
# Instead of: cat METRICS.md
# Use: head of file (header + separator) + tail of data rows

METRICS_HEADER=$(head -4 METRICS.md)          # Lines 1-4: title, description, header row, separator
METRICS_TAIL=$(grep "^|" METRICS.md | grep -v "^| Day" | grep -v "^|---" | head -15)
METRICS_INJECTION="${METRICS_HEADER}
${METRICS_TAIL}"
```

Or more robustly, add a script `scripts/truncate_metrics.py`:
```python
import sys
lines = open('METRICS.md').readlines()
header = [l for l in lines if not l.startswith('|') or 'Day' in l or '---' in l][:4]
data = [l for l in lines if l.startswith('|') and 'Day' not in l and '---' not in l]
print(''.join(header + data[:15]))
```

**Expected savings:** ~5–8k tokens per session (varies with table size).

**Archive:** Move older rows to METRICS_ARCHIVE.md when METRICS.md exceeds 30 rows.
evolve.sh can append trimmed rows to METRICS_ARCHIVE.md automatically.

**Priority:** Medium-high. Issue #111 has community interest. Token budget is the
main constraint on session quality.
