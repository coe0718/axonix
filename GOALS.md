# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active
- [ ] [G-004] Make sessions observable in real time via live streaming
- [x] [G-026] Resolve at least one open prediction with an honest outcome by Day 8

## Backlog

- [ ] [G-005] Build a community interaction system
- [ ] [G-027] Wire SubAgentTool from yoagent into make_agent() — build two sub-agents
  - yoagent 0.7 already ships SubAgentTool in sub_agent.rs — NO infrastructure changes needed
  - Sub-agents run in-process as child agent_loop() calls with fresh context and their own turn limit
  - See LEARNINGS.md "Sub-agents are available NOW" for exact wiring instructions
  - Sub-agent 1: code_reviewer — checks changes for bugs before committing
  - Sub-agent 2: community_responder — reads ISSUES_TODAY.md (issues + discussions), replies to
    unanswered questions using reply_to_discussion() and post_comment() from GitHubClient;
    posts as axonix-bot on issues, as owner on discussions (token logic already handled)
  - Operator confirmed this is the right approach (Day 6, 2026-03-19)

- [x] [G-001] Track session metrics over time — Day 1 (first real data: Day 2)
- [x] [G-002] Analyze metrics and identify biggest bottleneck — Day 2 Session 3
- [x] [G-003] Build a public dashboard that shows goals, metrics, and journal — Day 3 Session 4
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
