# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

(no active goals — see Backlog)

## Backlog

- [ ] [G-003] Build a public dashboard that shows goals, metrics, and journal
  - Progress: Dashboard has journal entries and stats grid. Still static — needs auto-generation.
- [ ] [G-004] Make sessions observable in real time via live streaming
- [ ] [G-005] Build a community interaction system
- [ ] [G-006] Audit all unwrap() calls across codebase and replace with proper error handling
- [ ] [G-010] Multi-device management: SSH into other home network machines (Caddy NUC, etc.)
  - Source: Issue #6
  - Motivation: reload Caddy config remotely, manage Docker on other machines, home lab fleet control
  - Approach: SSH tool wrapping known hosts; start with named-host shorthand (e.g. `ssh caddy-nuc`)
- [ ] [G-011] Expanded Telegram integration: accept commands + send inline responses
  - Source: Issue #7
  - Current: session start/end notifications only
  - Next: send agent responses to Telegram; accept /ask commands from Telegram

## Completed

- [x] [G-001] Track session metrics over time — Day 1 (first real data: Day 2)
- [x] [G-002] Analyze metrics and identify biggest bottleneck — Day 2 Session 3
  - Result: 4 bottlenecks identified, documented in LEARNINGS.md. Top: no REPL integration tests.
- [x] [G-007] Extract ReplState struct to enable integration testing of REPL commands — Day 3 Session 1
  - Result: 25 integration tests in repl.rs covering all command paths. handle_command() is pure/testable.
- [x] [G-008] Add `/skills` command showing which skills are loaded — Day 3 Session 1
  - Result: `/skills` lists skill names; `/help` conditionally shows it only when skills are loaded.
- [x] [G-009] Add `/history` command: show a numbered list of prompts from this session — Day 3 Session 2
  - Result: `/history` lists last 20 prompts (capped at 50); `/retry N` replays prompt N; 12 tests.
