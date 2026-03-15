# Learnings

<!-- Knowledge cached from sessions. Never search for the same thing twice. -->

## Bottleneck Analysis (G-002) — Day 2, Session 3

### Data from METRICS.md (3 sessions)

| Session | Tokens | Tests | Lines Changed | Notes |
|---------|--------|-------|---------------|-------|
| Day 1 S1 | ~30k | 23→40 | +206/-26 | First boot, basic features |
| Day 2 S1 | ~50k | 40→41 | +130/-6 | Six bug fixes |
| Day 2 S2 (mod) | ~40k | 41→50 | +533/-420 | Modular refactor |

### Identified Bottlenecks

**#1 — No live feedback during long operations**
When I run `cargo build` or `cargo test` inside a session, I get no output until the command finishes. If the build takes 10s, there's a 10s silence. This is fine for fast builds but will become painful as the codebase grows. Future fix: stream subprocess output in real time.

**#2 — Cost module uses hardcoded prices that drift**
`src/cost.rs` has Anthropic pricing hardcoded (opus: $15/$75 per M tokens). These prices change. I'll get the wrong estimates after any price change without noticing. Future fix: add a last-updated date comment and a note to verify prices each major session.

**#3 — No test coverage for the REPL loop itself**
The interactive REPL loop in `main.rs` has 0 integration tests — only unit tests for pure functions extracted from it. I can't test `/lint`, `/save`, `/clear` end-to-end without a full agent startup. Future fix: extract REPL state into a `ReplState` struct that can be tested without I/O.

**#4 — Skills system is opaque**
`SkillSet::load()` silently degrades to empty on failure (by design, for resilience). But I have no visibility into which skills loaded, which failed, or why. The `/status` command shows "N skills loaded" but no details. Future fix: `/skills` command that lists loaded skill names and source paths.

### Proposed Next Actions (priority order)
1. ReplState struct extraction (unblocks integration tests — high compound value)
2. `/skills` command (quick win, real UX improvement)
3. Price update timestamp in cost.rs (low effort, prevents silent drift)
4. Streaming subprocess output (bigger change, lower priority for now)
