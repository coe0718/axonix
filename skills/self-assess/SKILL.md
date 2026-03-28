---
name: self-assess
description: Axonix evaluates its own code, goals, and metrics to identify improvement opportunities.
---

# Self Assess

You are Axonix. This skill is called when you need to evaluate your current state.

## What To Assess

### Code Quality
Read `src/main.rs` and evaluate:
- Are there obvious bugs or edge cases not handled?
- Is there duplicated logic that could be simplified?
- Are error messages clear and useful?
- Are there missing tests for important behaviors?
- Is there anything that would cause a silent failure?

### Goal Quality
Read `GOALS.md` and evaluate:
- Are active goals still the right thing to pursue?
- Are any goals blocked in a way that needs a subgoal?
- Is the backlog ordered by impact?
- Are any backlog items now irrelevant given what has been learned?

### Metrics Trends
Read `METRICS.md` and evaluate:
- Is the commit rate trending up or down?
- Are test failures increasing or decreasing?
- Are sessions getting longer or shorter?
- Is there a pattern in what causes reverts?

### Skills
Read all files in `skills/` and evaluate:
- Are any skills outdated or no longer accurate?
- Are there capability gaps that a new skill would fill?
- Could any existing skill be made more effective?

## Output

Write a structured assessment with three sections:

**Strengths** — what is working well and should be preserved

**Gaps** — specific problems, weaknesses, or missing capabilities

**Recommendations** — ranked list of improvements, each with:
- What to do
- Why it matters
- How to know when it's done

This assessment feeds directly into goal formation. Be specific and honest.

## Test Count Discrepancy Check

At the start of every session, after running `cargo test`, compare the current test count
against the last recorded count in METRICS.md.

Steps:
1. Run: `cargo test 2>&1 | grep "^test result"`
2. Sum the passed counts across all test result lines.
3. Read METRICS.md and find the most recent non-stub row (not containing `in progress` or `~?k`).
   Parse the "Tests" column (column 5).
4. Compute delta = current_count - last_count.
5. If |delta| > 10: log a warning in the journal entry noting the discrepancy and the likely cause.
   Example: "⚠️ Test count dropped from 757 to 739 (-18) — no documented reason. Check for removed tests."

The threshold of ±10 allows for normal test churn (adding 1-5 tests per session) while catching
unexpected drops (deleted test files, commented-out tests) or suspicious spikes.

This check runs during Phase 1 self-assessment. Record findings in the journal even if
everything looks fine (e.g., "test count: 768, last recorded: 768 — no discrepancy").
