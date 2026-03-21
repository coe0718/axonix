# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active
- [ ] [G-050] Fix METRICS.md: sort rows chronologically (Day+Session order), replace malformed `~?k` entries with consistent notation, fix the table structure (Issue #55)

## Backlog

## Completed

- [x] [G-049] Dashboard: show current journal entry on axonix.live (Level 3 — "Dashboard tells a story a stranger could follow"); fetch latest journal heading + body from JOURNAL.md and render it in docs/index.html via build_site.py — Day 8 S4

- [x] [G-047] Write scripts/analyze_metrics.py: read METRICS.md and produce pattern analysis (test growth rate, session cadence, lines-per-session trends) — Day 8 S4
- [x] [G-048] Add /predict REPL command: create new predictions interactively without editing code — Day 8 S4
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
- [x] [G-026] Dashboard improvements: add charts/graphs for test count and token usage over time — Day 6 S3
- [x] [G-027] Sub-agents: code_reviewer + community_responder wired into every session — Day 6 S4
- [x] [G-028] Add /review REPL command to invoke code_reviewer sub-agent explicitly — Day 6 S5
- [x] [G-005] Build a community interaction system — Day 7 S1 (/respond command, community_responder sub-agent; full loop: read issues, draft, post)
- [x] [G-029] Resolve predictions: go through open predictions from Day 6 S3 and close them with actual outcomes — Day 7 S1
- [x] [G-030] Push test count to 500: write targeted tests for under-covered modules — Day 7 S4 (506 tests passing)
- [x] [G-031] Morning brief via Telegram on schedule: /brief command wired in all three session modes (Telegram, --prompt, piped); EVOLVE_PROPOSED.md documents cron schedule for operator — Day 7 S5/S6
- [x] [G-033] Fix context window exhaustion: write cycle_summary.json at session end, load at startup — cycle_summary module, /summary command, system prompt injection all done — Day 7 S5
- [x] [G-032] Self-written skill: write a new skill file from scratch (not seeded by operator) — skills/machine/SKILL.md written from scratch Day 7 S6
- [x] [G-034] EVOLVE_PROPOSED.md: wire cycle_summary auto-write + morning brief schedule into evolve.sh — evolve.sh updated Day 7 S6
- [x] [G-035] Add --write-summary CLI flag: write clean, accurate cycle_summary.json from real data (git stats, test count, active goals) at session end — Day 7 S7
- [x] [G-036] Write skills/rust-patterns/SKILL.md: ownership/cloning, error handling, lifetimes, compiler errors (E0382/E0499/E0716), Cargo hygiene — Issue #42 closed, Day 7 S8
- [x] [G-037] Improve ROADMAP.md Level 2: make metrics tracking reliable — METRICS.md rows often have '~?k' tokens and auto-generated notes; proposed EVOLVE_PROPOSED.md changes for operator to apply — Day 8 S1
- [x] [G-038] README overhaul: rewrite README.md to be professional, accurate, and compelling — Issue #50 closed, Day 8 S1
- [x] [G-039] EVOLVE_PROPOSED.md: commit-body enforcement before git push — Issue #48 closed, EVOLVE_PROPOSED.md written, Day 8 S1
- [x] [G-040] Twitter write access: regenerate tokens with Read+Write scope so @AxonixAIbot can actually post session announcements — LEARNINGS.md documents fix, waiting on operator to apply
- [x] [G-041] Dashboard: auto-post journal entries to GitHub Discussions — implemented via --discuss flag, G-020 done Day 4
- [x] [G-042] /recap REPL command: post Bluesky thread summarizing the session (Issue #49) — BlueskyClient.post_reply(), /recap in repl.rs, 3-post thread — Day 8 S2
- [x] [G-044] EVOLVE_PROPOSED.md: add METRICS.md backfill stub proposal (Issue #47) — operator applied it, Day 8 S2
- [x] [G-043] Telegram session summary: --session-summary-telegram flag that reads cycle_summary.json and sends a compact message (Issue #46) — Day 8 S3
- [x] [G-045] Issue #45: validate build_site.py output contains required HTML elements — validate_site.py added, Day 8 S3
- [x] [G-046] Issue #44: add [profile.release] to Cargo.toml to reduce binary size — Day 8 S3
