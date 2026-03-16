# Proposed Changes to scripts/evolve.sh

Approved by operator in Issue #18 comments: "go ahead with B and C Axonix. Thanks. Close this when your done."

## Change B: Compress JOURNAL.md context to last 3 entries

**Current (line 72-73):**
```bash
# ── Step 3: Prepare journal tail (last 10 entries for context) ──
RECENT_JOURNAL=$(head -200 JOURNAL.md 2>/dev/null || echo "No journal yet.")
```

**Replace with:**
```bash
# ── Step 3: Prepare journal tail (last 3 entries for context) ──
# Token compression B: only include the 3 most recent journal entries instead of
# the full file. Saves ~500-1,000 tokens/session (grows as journal grows).
RECENT_JOURNAL=$(python3 -c "
import re, sys
text = open('JOURNAL.md').read()
entries = re.split(r'(?=^## Day )', text, flags=re.MULTILINE)
entries = [e for e in entries if e.strip().startswith('## Day')]
recent = entries[:3]
print('# Journal (last 3 entries)\n')
print('\n'.join(recent))
" 2>/dev/null || head -60 JOURNAL.md 2>/dev/null || echo "No journal yet.")
```

**Why:** The journal grows by ~10 lines per session. Reading the full 89-line journal sends
context that is rarely relevant to the current session. Last 3 entries = last 1-2 days of
work, which is all that's needed for continuity. Full journal still readable via read_file.

---

## Change C: Filter cargo test output in session prompt

**Current (line 108-109 in the PROMPT heredoc):**
```
Run: cargo build && cargo test
Report the exact test count and any failures.
```

**Replace with:**
```
Run: cargo build && cargo test 2>&1 | grep -E "(^test result|FAILED|^error\[)"
Report the exact test count and any failures shown. The summary line "test result: ok. N passed"
is sufficient — do not include individual "test X ... ok" lines in your report.
```

**Why:** Running `cargo test` currently echoes 208 lines of "test X ... ok" into context.
Only the summary line "test result: ok. 208 passed; 0 failed" matters. This saves
300-600 tokens per test run, multiplied by 2-4 runs per session = up to 2,400 tokens saved.

---

## Combined estimated savings: 800-3,600 tokens/session

Apply these changes manually to scripts/evolve.sh on the host. The file is :ro
inside the container so I cannot modify it directly.
