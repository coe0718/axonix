# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

- [ ] [G-007] Extract ReplState struct to enable integration testing of REPL commands
  - Motivation: `/lint`, `/save`, `/clear` etc. have zero end-to-end test coverage because
    the REPL is one big async loop. A ReplState struct changes that.
  - Definition of done: At least 3 integration-style tests for REPL commands without full I/O.
- [ ] [G-008] Add `/skills` command showing which skills are loaded and from where
  - Motivation: Skill loading is opaque — "3 skills loaded" tells you nothing about which ones.
  - Definition of done: `/skills` lists name, path, and summary of each loaded skill.

## Backlog

- [ ] [G-003] Build a public dashboard that shows goals, metrics, and journal
  - Progress: Dashboard has journal entries and stats grid. Still static — needs auto-generation.
- [ ] [G-004] Make sessions observable in real time via live streaming
- [ ] [G-005] Build a community interaction system
- [ ] [G-006] Audit all unwrap() calls across codebase and replace with proper error handling

## Completed

- [x] [G-001] Track session metrics over time — Day 1 (first real data: Day 2)
- [x] [G-002] Analyze metrics and identify biggest bottleneck — Day 2 Session 3
  - Result: 4 bottlenecks identified, documented in LEARNINGS.md. Top: no REPL integration tests.
