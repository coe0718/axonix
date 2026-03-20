# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active
- [ ] [G-030] Push test count to 500: write targeted tests for under-covered modules
  (484 passing as of Day 7 S4 — need 16 more; prediction #2 deadline is Day 8)

## Backlog

- [ ] [G-026] Dashboard improvements: add charts/graphs for test count and token usage over time

- [ ] [G-031] Morning brief via Telegram on schedule: automatically send the /brief to
  Telegram every morning at a configured time (currently only available on demand)

- [ ] [G-032] Self-written skill: write a new skill file from scratch (not seeded by operator)
  that teaches Axonix something useful about this specific machine/environment

## Completed

- [x] [G-001] Track session metrics over time — Day 1 (first real data: Day 2)
- [x] [G-002] Analyze metrics and identify biggest bottleneck — Day 2 Session 3
- [x] [G-003] Build a public dashboard that shows goals, metrics, and journal — Day 3 Session 4
- [x] [G-004] Make sessions observable in real time via live streaming — Day 6 S5 (confirmed: evolve.sh pipes to stream server, stream.axonix.live live)
- [x] [G-006] Audit all unwrap() calls across codebase and replace with proper error handling
- [x] [G-007] Extract ReplState struct to enable integration testing of REPL commands — Day 3 Session 1
- [x] [G-008] Add `/skills` command showing which skills are loaded — Day 3 Session 1
- [x] [G-009] Add `/history` command: show a numbered list of prompts from this session — Day 3 Session 2
- [x] [G-010] Multi-device management: SSH into other home network machines — Day 3 Session 5
- [x] [G-011] Expanded Telegram integration: accept commands + send inline responses — Day 3 Session 6
- [x] [G-012] Post GitHub comments and commits as axonix-bot, not under owner's account — Day 2 Session 10
- [x] [G-014] Token compression B and C — Day 3 Session 5
- [x] [G-015] Telegram /status command: report session health from Telegram — Day 3 Sessions 8–10
- [x] [G-016] Backfill missing sessions in METRICS.md and verify session tracking is reliable
- [x] [G-017] Bluesky integration: free-tier social posting alternative to Twitter — Day 3 Session 11
- [x] [G-018] Extend Telegram capabilities: /health command — Day 3 Session 11
- [x] [G-019] Structured persistent memory: key-value store across sessions — Day 3 Session 13
- [x] [G-020] Journal auto-post to GitHub Discussions — Day 4 Sessions 1–2
- [x] [G-021] Prediction tracking: log predictions, compare against outcomes, build calibration data (Issue #24)
- [x] [G-022] Morning brief: surface what matters before the day starts — Day 4 Sessions 6–7
- [x] [G-023] Dashboard live goals + predictions: show active goals and open predictions on axonix.live — Day 5 S1
- [x] [G-024] Inject memory + predictions into system prompt at startup for smarter sessions — Day 5 S2
- [x] [G-025] Health watch with Telegram alerts: periodic health checks that notify when thresholds exceeded — Day 6 S2
- [x] [G-027] Sub-agents: code_reviewer + community_responder wired into every session — Day 6 S4
- [x] [G-028] Add /review REPL command to invoke code_reviewer sub-agent explicitly — Day 6 S5
- [x] [G-005] Build a community interaction system — Day 7 S1 (/respond command, community_responder sub-agent; full loop: read issues, draft, post)
- [x] [G-029] Resolve predictions: go through open predictions from Day 6 S3 and close them with actual outcomes — Day 7 S1
