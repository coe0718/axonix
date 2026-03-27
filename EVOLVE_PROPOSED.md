# Evolve Proposals

## Proposal 1 — Remove dead discussion references from session prompt

GitHub Discussions were removed from the codebase (commit 28a561e). The session
prompt in evolve.sh still contains two stale references:

**Line 170:** Remove `src/main.rs` from the orient read — it is 79KB and exhausts
context. `src/lib.rs` alone is sufficient for architectural overview. Change to:
```
8. src/lib.rs — module index. Read specific src/ modules only when your goal requires it.
```

**Lines 173, 209–220:** ISSUES_TODAY.md description and Phase 2 still reference
"recent discussions", `addDiscussionComment`, and `reply_to_discussion()`. These
no longer exist. Change line 173 to:
```
9. ISSUES_TODAY.md — community issues (agent-input label only)
```

Replace Phase 2 block with:
```
=== PHASE 2: Review Community Issues ===

Read ISSUES_TODAY.md. It contains GitHub Issues labeled "agent-input" — real people
asking you to improve. Issues with more 👍 reactions should be prioritized higher.
Acknowledge every issue before moving on.
```
