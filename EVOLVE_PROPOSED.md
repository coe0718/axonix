# EVOLVE_PROPOSED.md

Proposed changes to `scripts/evolve.sh` — requires operator review and manual application.
(evolve.sh is mounted read-only inside the container — Axonix cannot modify it directly.)

---

## Proposal 1: Auto-write cycle_summary.json at session end (Issue #38 / G-033)

**Problem:** `cycle_summary.json` is implemented and loaded at startup, but it's only
written when the agent manually calls `/summary`. For context window control to work,
the summary must be written automatically at the end of every cycle.

**Change needed in evolve.sh** — add after the `axonix --prompt "..."` call completes:

```bash
# Auto-write cycle summary at end of each cycle (Issue #38)
# Extracts session label from environment and writes a compact summary.
SESSION_LABEL="Day ${DAY} Session ${SESSION_COUNT}"
docker compose run --rm axonix axonix --summary "${SESSION_LABEL}" 2>/dev/null || true
```

Or, if axonix supports a `--write-summary` flag (it doesn't yet — see Proposal 1b below):

```bash
docker compose run --rm axonix axonix --write-summary "${SESSION_LABEL}" 2>/dev/null || true
```

**Proposal 1b:** Add `--write-summary <label>` CLI flag to axonix that writes a minimal
cycle_summary.json based on recent git diff and GOALS.md state — no agent session needed.
This is cleaner than running a full `/summary` command via --prompt.

---

## Proposal 2: Morning brief via Telegram at 7:00 AM (G-031)

**Problem:** The `/brief` Telegram command works but requires someone to send it.
The morning brief should be sent automatically each morning so the operator gets it
on their phone before the day starts.

**Change needed:** Add a cron job that runs at 7:00 AM (America/Indiana/Indianapolis):

```cron
0 7 * * * cd /path/to/axonix && docker compose run --rm axonix axonix --brief --telegram 2>/dev/null
```

Or, add a `--brief-telegram` mode to axonix that sends the brief to Telegram and exits:

```bash
docker compose run --rm axonix axonix --brief --send-telegram 2>/dev/null || true
```

**Current state:** `axonix --brief` prints the brief to stdout. The Telegram command
`/brief` sends it to Telegram when triggered inbound. Missing: outbound push at 7 AM.

**Implementation note:** The brief content is ready (Brief::collect() + format_telegram()).
What's needed is a `--brief --send-telegram` mode that calls tg.send_message() and exits.
I can implement that flag if the operator wants it — it's a 15-minute code change.

---

## Proposal 3: Auto-post discussion at end of each cycle

**Current state:** `axonix --discuss` posts the latest journal entry as a GitHub Discussion.
This is called in evolve.sh already (based on the .axonix/discussed_* marker files).
No change needed here — this is working.

---

*Written by Axonix — Day 7 Session 6 (2026-03-20)*
*Awaiting operator review. Apply manually to scripts/evolve.sh.*

---

**Note: --write-summary is now implemented (G-035).** Proposal 1b is complete.
Operators can update evolve.sh to call `axonix --write-summary "Day N Session M"` instead
of the Python block. The CLI flag reads real git log data, GOALS.md active items, and
changed files — producing accurate `.axonix/cycle_summary.json` with no fragile shell logic.
