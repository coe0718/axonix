# Evolve.sh Proposed Changes

## Proposal 1 — Token efficiency improvements (Issue #111)

**Filed:** Day 23 S3 (2026-04-06)
**Priority:** High — currently burning 40-71k tokens/session; target is ≤35k for single-goal sessions

### Background

Day 23 S2 used ~71k tokens to commit 5 lines. The main sources of context overhead are:
1. The session prompt itself (fixed size — can't reduce)
2. Files read during Phase 1 (GOALS.md 4KB, LEARNINGS.md 10KB, CAPABILITIES.md, etc.)
3. File reads during implementation (varies by task)

METRICS.md was already archived in Day 23 S2 (now ~1.5KB, was 14.7KB). evolve.sh
already only injects 5 rows via `tail -5`, so that was never the problem.

### Root Cause

The actual 71k token cost in Day 23 S2 was the file splitting work (reading 3000-line
files to split them). That's unavoidable for large refactor sessions. However, there
are still structural improvements that save tokens every session:

### Proposed Change 1: Limit RECENT_JOURNAL injection to last 2 entries (not 3)

In `evolve.sh`, find this line (around line 139):
```bash
RECENT_JOURNAL=$(python3 -c "
import re
text = open('JOURNAL.md').read()
...
" 2>/dev/null || head -60 JOURNAL.md 2>/dev/null || echo "No journal yet.")
```

The Python script currently extracts the last 3 journal entries. Change `3` to `2`
in the regex/slice. Journal entries average 200-400 tokens each. Saving 1 entry = ~300 tokens/session.

To find the exact line count to change: search for the number that limits entries in the python3 -c block around line 147.

### Proposed Change 2: Skip LEARNINGS.md Phase 1 read if hash matches

Currently the session prompt instructs Axonix to read LEARNINGS.md every session
(~10KB). This is wasted context when the file hasn't changed.

Add LEARNINGS.md to doc_hashes.json tracking. The session startup already checks
doc_hashes.json for IDENTITY.md, USER.md, etc. Adding LEARNINGS.md to this check
means it's only read in full when it actually changes.

In `evolve.sh`, after running the session, update the hash for LEARNINGS.md:
```bash
python3 -c "
import json, hashlib
with open('.axonix/doc_hashes.json') as f:
    d = json.load(f)
for fname in ['LEARNINGS.md', 'CAPABILITIES.md', 'GOALS.md']:
    try:
        d[fname] = hashlib.sha256(open(fname,'rb').read()).hexdigest()[:16]
    except: pass
with open('.axonix/doc_hashes.json','w') as f:
    json.dump(d, f, indent=2)
"
```

This would save ~10KB (LEARNINGS.md) + ~4KB (GOALS.md if tracked) per session when
those files don't change — which is most sessions.

### Proposed Change 3: Add GOALS.md and CAPABILITIES.md to doc_hashes.json

Same as above — track these files in doc_hashes.json so Axonix skips reading them
when they haven't changed. CAPABILITIES.md rarely changes (save ~3KB). GOALS.md
changes most sessions but could still skip in sessions where only code changes occur.

### Expected Total Saving

- Proposal 1 (journal): ~300 tokens/session
- Proposal 2 (LEARNINGS skip): ~4k tokens when no changes (most sessions)
- Proposal 3 (GOALS/CAPS skip): ~2k tokens in code-only sessions

Combined expected savings: 3-6k tokens for typical sessions after apply.

### Notes

- METRICS.md archiving already done in Day 23 S2 — no change needed there
- evolve.sh's `tail -5` on METRICS.md is already correct — Issue #111's claim about
  full METRICS.md injection was inaccurate (the file was large but injection was already truncated)
- The main token cost is unavoidable for large refactor sessions (G-121 will also be expensive)
- The 35k target is achievable for single-goal sessions that don't read large files
