# Goals

## North Star

Be more useful to the person running me than any off-the-shelf tool could be.

Every goal should move toward this. Every session should answer:
*did I become more useful today?*

## Active

### G-094 — Telegram task triggers: /run, /goal, /status commands (Issue #105)
**Why:** The operator can already ask questions via `/ask`. Adding `/run <task>`, `/goal <description>`, and `/status` would let them dispatch work and add backlog items directly from Telegram — removing friction from the human-in-the-loop workflow.
**Definition of done:** Three new `BotCommand` variants in `telegram.rs`, dispatch in `listener.rs`, tests for each.
**Status:** [ ]

## Backlog

### G-095 — Haiku routing for lightweight listener tasks (Issue #106)
**Why:** Once /run, /goal, /status are live (G-094), the listener can route simple commands (status, goal append) to claude-haiku-4-5 instead of Sonnet to reduce token costs. /run <task> stays on Sonnet for reasoning capability.
**Definition of done:** Model selection in listener.rs keyed on command type; `LISTENER_HAIKU_MODEL` env var; test coverage.

## Completed

Completed goals have been archived to GOALS_ARCHIVE.md to keep this file lean.
Do not move goals back here — append new completions to GOALS_ARCHIVE.md directly,
or keep a rolling window of the last 5 completed goals below for recent context.

<!-- Last 5 completed (newest first): -->
- [x] [G-094] Telegram task triggers: /run, /goal, /status — Day 18 S2
- [x] [G-093] Telegram /ask context awareness: active goal + memory injected — Day 18 S1
- [x] [G-092] Dashboard: observations list page with tag filtering — Day 17 S4
- [x] [G-091] Dashboard: wire memory-search results into session orient panel — Day 17 S3
- [x] [G-090] Morning brief: predictions due soon + memory context sections — Day 17 S2
