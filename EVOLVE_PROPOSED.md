# EVOLVE_PROPOSED.md

Proposed changes to evolve.sh or session structure that require operator review.
Axonix cannot modify evolve.sh directly (mounted :ro). These are queued for manual apply.

---

## Proposal 1 — Batch implementer calls (G-107)

**Problem:** The session prompt says "call the implementer tool with a detailed plan" once per goal, which leads to multiple sequential implementer invocations for work that could be combined. Each invocation has cold-start overhead (build, read files, orient). This session used two implementer calls for two changes that shared the same files and test run.

**Proposed change:** Add this rule to Phase 6 of the session prompt:

```
If you have multiple code changes this session, combine them into a SINGLE implementer
call unless they are genuinely risky to combine (e.g., one change modifies a file the
other depends on in a way that could mask failures). Separate calls are appropriate for
large independent modules, not for small same-file changes. The implementer's 40-turn
budget is sufficient for 3-5 related tasks.
```

**Expected savings:** 1-2 fewer implementer round-trips per session on sessions with 2+ small goals. Estimate 5-10 minutes of wall-clock time saved per session.

**Risk:** Low. The implementer already handles multi-task plans well (see Day 18 S4 — two changes in one call worked cleanly).

---

## Proposal 2 — Stable-file hash cache (G-108)

**Problem:** Every session reads IDENTITY.md, USER.md, CAPABILITIES.md, ROADMAP.md in full. These files haven't changed in days. That's ~15KB (~4-5K tokens) of context burned before any planning happens.

**Proposed change:** After each session, evolve.sh writes SHA256 hashes of stable docs to `.axonix/doc_hashes.json`:

```bash
# After session completes, hash stable docs
python3 -c "
import json, hashlib, pathlib
files = ['IDENTITY.md', 'USER.md', 'CAPABILITIES.md', 'ROADMAP.md', 'COMMIT_CONVENTIONS.md']
hashes = {}
for f in files:
    p = pathlib.Path(f)
    if p.exists():
        hashes[f] = hashlib.sha256(p.read_bytes()).hexdigest()[:16]
with open('.axonix/doc_hashes.json', 'w') as fh:
    json.dump(hashes, fh, indent=2)
" || true
```

The session prompt preamble becomes:

```
Before reading stable docs, check .axonix/doc_hashes.json. For each file listed,
compute its current SHA256 prefix and compare to the stored hash. If they match,
skip reading the file and instead note: "[IDENTITY.md: unchanged since last session]".
Only read the file if the hash differs or doc_hashes.json doesn't exist.
```

**Expected savings:** 4-6K tokens per session once the cache is warm (after first run). Over 4 sessions/day that's 16-24K tokens/day.

**Risk:** Low. First session after operator applies this will read all files normally (no cache). Subsequent sessions skip unchanged ones.
