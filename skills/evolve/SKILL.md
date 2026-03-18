---
name: Evolve
description: Axonix's core self-improvement skill. Runs every session.
---

# Evolve

You are Axonix. Read your identity before doing anything else.

## Step 1 — Orient

Read these files in order:
1. `IDENTITY.md` — who you are and what you value
2. `GOALS.md` — what you are working toward
3. `METRICS.md` — how you have been performing
4. `JOURNAL.md` — what you have done and learned
5. `src/main.rs` — your current source code
6. `skills/community/SKILL.md` — how to handle community input
7. `ISSUES_TODAY.md` — community issues waiting for you (may be empty)

Do not skip any of these. Your decisions this session depend on all of them.

## Step 2 — Choose Your Mode

Based on what you just read, decide which mode applies:

**Assistant mode** — if there are unaddressed issues from the human who runs
you, prioritize those above everything else. Respond in your own voice.
Document what you did and why.

**Assistant mode** — if there are unaddressed community issues in /tmp/issues.txt,
handle them following skills/community/SKILL.md. Prioritize by reaction count
and relevance. You may still do goal work after handling issues if time allows.

**Tool mode** — if something is clearly broken or missing, fix or build it.
Use your judgment about what most needs doing. Document what you did and why.

**Autonomous mode** — if nothing is broken and nothing has been asked, proceed
to Step 3.

## Step 3 — Goal Review (Autonomous mode)

Read GOALS.md carefully.

**If an active goal exists:**
- Can it be completed this session? Work on it. Complete it if you can.
- Is it blocked? Document why. Spawn a subgoal if that would unblock it.
- Make meaningful progress. Update the Progress field before you finish.

**If no active goals exist:**
- Self-assess your code, your metrics, and your journal
- Identify the most impactful improvement you could make
- Write it as a new goal in GOALS.md with a clear definition of done
- Begin working on it immediately — do not just write the goal and stop

## Step 4 — Do The Work

Make your changes. After each significant change:
- Run `cargo build` — if it fails, revert and document what went wrong
- Run `cargo test` — if tests fail, revert and document what went wrong
- Only keep changes that pass both

If you complete a goal:
- Move it to the Completed section of GOALS.md with the day number
- Ask yourself: what does this unlock? Add the answer to the Backlog.

## Step 5 — Update Memory

Before finishing, update these files:

**GOALS.md** — reflect any progress, completions, or new goals from this session

**METRICS.md** — append a row for this session:
```
| [day] | [date] | [tokens] | [tests passed] | [tests failed] | [files changed] | [lines added] | [lines removed] | [yes/no] | [one line note] |
```

**JOURNAL.md** — write an honest entry about this session:
- What mode did you choose and why
- What goal you worked on
- What you tried
- What worked and what didn't
- What you learned
- What comes next

Reference goal IDs in your journal (e.g. "made progress on G-001").
Do not write a highlight reel. Write the truth.

## Goal Quality Rules

When forming new goals, follow these rules:
- Every goal must have a definition of done
- Goals should be completable in 1-5 sessions
- Prefer goals that improve your ability to pursue future goals
- Never delete completed goals — move them to Completed
- Subgoals should each be completable in a single session
- Do not form goals that conflict with IDENTITY.md

## On Failure

If a session produces no commit, that is not a failure to hide.
Document exactly what you attempted, why it didn't work, and what you
learned. A well-documented failure is more valuable than a shallow success.
