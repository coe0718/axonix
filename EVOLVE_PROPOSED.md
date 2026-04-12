# EVOLVE_PROPOSED.md

Proposals for changes to evolve.sh or operator-owned config files.
Do NOT apply these yourself — the operator reviews and applies them.

---

## Proposal 1 — Trim verbose phase instructions (Issue #116)

**Requested by:** Issue #116 (community, Day 29 S2)
**Estimated savings:** ~100–120 tokens/session, ~200–240 tokens/day

### Specific changes to evolve.sh prompt section:

**1. Collapse Phase 7 "DO NOT do these manually" block (~50 tokens saved)**

Replace the current 12-line block (8 bullet points listing archive-journal, cycle_summary, token count, Bluesky, site rebuild, fallback metrics, wrap-up commit) with a single comment line:

```
# evolve.sh handles automatically after session: journal-archive, cycle_summary, token count, Bluesky post, site rebuild, fallback metrics, wrap-up commit. Do not do these manually.
```

**2. Collapse Phase 1 self-assessment preamble (~30 tokens saved)**

The paragraph "Check for: Crash bugs or panics / Missing error handling / Any capability in CAPABILITIES.md you haven't used yet" can be reduced to:
```
Check for: crash bugs, silent failures, unused CAPABILITIES.md entries.
```

**3. Remove Phase 8 prediction examples (~20 tokens saved)**

The 3 example prediction lines under Phase 8 are no longer needed after 29 sessions. Remove:
```
Good predictions name a specific day, metric, or behaviour. Examples:
  "By Day 32, the personal assistant architecture will be designed and a goal will be open for it."
  "The test count will reach N by Day 31."
  "The most common failure mode in the next 5 sessions will be X."
```

**Keep intact:** Phase 3 priority list, Phase 6 batching rule, Phase 4 commit-order instructions — these still prevent real mistakes.

**Total estimated token reduction per session:** ~100–120 tokens (~5% of typical session overhead from the evolve.sh prompt injection).
