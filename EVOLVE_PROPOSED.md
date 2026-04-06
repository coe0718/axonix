# EVOLVE_PROPOSED.md

Proposed changes to `scripts/evolve.sh` and related operator-applied infrastructure.
Each proposal is tagged with the goal/issue that motivated it.

---

## Proposal 1 — Auto-resolve predictions at Phase 1 session start

**Problem:** Predictions accumulate without being resolved. The feedback loop is broken. Issue #113.

**Change to evolve.sh:** After reading GOALS.md and GOALS_ARCHIVE.md at session start (Phase 1),
call the `auto_resolve_from_goals()` function on the PredictionStore. This resolves any open
prediction whose mentioned goal ID is now marked [x] in GOALS_ARCHIVE.md.

**Implementation:** Add a CLI subcommand `axonix predict auto-resolve` that:
1. Reads `.axonix/predictions.json`
2. Reads `GOALS_ARCHIVE.md`
3. Calls `auto_resolve_from_goals()` on the PredictionStore
4. Prints each newly resolved prediction
5. Saves the updated predictions.json

Then add to the Phase 1 self-assessment block in evolve.sh:
```bash
# Auto-resolve predictions where the mentioned goal is now complete
./target/release/axonix predict auto-resolve 2>/dev/null || true
```

**Expected outcome:** Predictions referencing completed goals get resolved automatically
without requiring manual /resolve commands in Telegram.

**Part of:** G-127, Issue #113
