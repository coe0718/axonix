# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

- [ ] [G-021] Prediction tracking: log predictions, compare against outcomes, build calibration data (Issue #24)
  - When I predict something will work a certain way, log it
  - After the session, compare predicted vs actual and store the delta
  - Over time, builds corpus of "where my model of my own codebase was wrong"
  - Compounds with memory (G-019) and aligns with honesty value

## Backlog

- [ ] [G-004] Make sessions observable in real time via live streaming
- [ ] [G-005] Build a community interaction system
- [x] [G-006] Audit all unwrap() calls across codebase and replace with proper error handling
  - Result: all unwrap() calls verified to be inside #[test] blocks only — production code is clean

- [x] [G-001] Track session metrics over time — Day 1 (first real data: Day 2)
- [x] [G-002] Analyze metrics and identify biggest bottleneck — Day 2 Session 3
  - Result: 4 bottlenecks identified, documented in LEARNINGS.md. Top: no REPL integration tests.
- [x] [G-003] Build a public dashboard that shows goals, metrics, and journal — Day 3 Session 4
  - Result: build_site.py auto-generates dashboard from JOURNAL.md + METRICS.md; live stats grid
    showing sessions, tokens, tests, lines written; runs automatically at end of every session.
- [x] [G-007] Extract ReplState struct to enable integration testing of REPL commands — Day 3 Session 1
  - Result: 25 integration tests in repl.rs covering all command paths. handle_command() is pure/testable.
- [x] [G-008] Add `/skills` command showing which skills are loaded — Day 3 Session 1
  - Result: `/skills` lists skill names; `/help` conditionally shows it only when skills are loaded.
- [x] [G-009] Add `/history` command: show a numbered list of prompts from this session — Day 3 Session 2
  - Result: `/history` lists last 20 prompts (capped at 50); `/retry N` replays prompt N; 12 tests.
- [x] [G-010] Multi-device management: SSH into other home network machines — Day 3 Session 5
  - Result: `/ssh list` shows registered hosts, `/ssh <host> <cmd>` runs commands on remote machines, hosts.toml config, 8 integration tests. Closes Issue #6.
- [x] [G-012] Post GitHub comments and commits as axonix-bot, not under owner's account — Day 2 Session 10
  - Result: `GitHubClient` wired to `/comment <n> <text>` REPL command; `configure_git_identity()` called at startup; GitHub identity shown in banner; `AXONIX_BOT_TOKEN` added to docker-compose.yml (Issue #13 fix). 7 new tests. Closes Issues #11 and #13.
- [x] [G-011] Expanded Telegram integration: accept commands + send inline responses — Day 3 Session 6
  - Result: background poll loop accepts `/ask <prompt>` from Telegram; agent responses forwarded back;
    Unicode-safe message chunking (fixed Day 3 Session 7); prompt injection protection (wrong chat_id rejected).
    14 tests in telegram.rs. Closes Issue #7.
- [x] [G-014] Token compression B and C — Day 3 Session 5
  - Result: evolve.sh updated to inject only last 3 journal entries (B) and filter test output to summary line (C).
    Saves ~800-1,600 tokens/session. Closes Issue #18.
- [x] [G-015] Telegram /status command: report session health from Telegram — Day 3 Sessions 8–10
  - Result: `/status` and `/help` commands work in all three modes (interactive, --prompt, piped)
    via background Telegram poll tasks. format_status_reply() shows model, mode, uptime, tokens.
    Closes Issue #21 (Telegram two-way). Fully implemented in main.rs, 14+ tests in telegram.rs.
- [x] [G-016] Backfill missing sessions in METRICS.md and verify session tracking is reliable
  - Result: Sessions 8, 9, 10, 11 all backfilled. METRICS.md now has complete history Day 1–Day 3 S11.
  - Session tracking now reliable (evolve.sh appends rows via build_site.py at end of each session).
- [x] [G-017] Bluesky integration: free-tier social posting alternative to Twitter — Day 3 Session 11
  - Result: `BlueskyClient` in bluesky.rs posts to Bluesky AT Protocol free API; `--bluesky-post` CLI flag;
    `format_session_post()` for session announcements; 13 tests. Env vars fixed in docker-compose.yml (Session 12).
    Closes Issue #22 (free alternative to blocked Twitter write API).
- [x] [G-018] Extend Telegram capabilities: /health command — Day 3 Session 11
  - Result: `/health` Telegram command reports CPU/memory/disk/uptime via HealthSnapshot::collect().
    BotCommand::Health dispatched in all three modes (interactive, --prompt, piped).
    Closes the /health expansion from Issue #7.
- [x] [G-019] Structured persistent memory: key-value store across sessions — Day 3 Session 13
  - Result: `MemoryStore` in memory.rs backed by `.axonix/memory.json`; `/memory` REPL command
    (set/get/del/list); timestamps on entries; 16 tests. Operator facts persist across sessions.
- [x] [G-020] Journal auto-post to GitHub Discussions — Day 4 Sessions 1–2
  - Result: `post_discussion` in github.rs via GraphQL API; `parse_latest_journal` + `format_discussion_body`
    helpers; `--discuss` CLI flag wired in main.rs; 5 tests in github.rs, 3 tests in cli.rs.
    Closes ROADMAP Level 3 "Journal entries posted to GitHub Discussions automatically".
