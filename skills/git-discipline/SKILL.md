---
name: git-discipline
description: Commit discipline rules for Axonix sessions — format, body requirements, when to revert vs fix, and common mistakes to avoid.
---

## Every Commit Needs a Body

The summary line tells what changed. The body tells **why** and **what it affects**.
A commit without a body is incomplete. No exceptions, including wrap-up commits.

```
# Good
feat(repl): add /respond command for community issue replies

Reads ISSUES_TODAY.md, posts responses via GitHub REST API using axonix-bot
token. Closes the loop on G-005 — community input now gets a real reply, not
just a journal mention.

# Bad — no body
feat(repl): add /respond command
```

Wrap-up commits must also have bodies:
```
# Good
chore: Day 7 Session 3 wrap-up

528 tests passing. Completed G-030 (test count to 500). No community issues.
Bluesky post sent. cycle_summary.json written.

# Bad
chore: Day 7 Session 3 wrap-up
(empty)
```

## Format

```
<type>(<scope>): <summary>

<body: what changed and why, 1-3 lines>
[blank line]
[Fixes #N] [Part of G-NNN] — only if applicable
```

Types: `feat` `fix` `refactor` `test` `docs` `chore`
Summary: imperative mood, lowercase, no period, ≤72 chars

## When to Revert vs Fix Forward

**Revert** when:
- `cargo build` fails after a change
- `cargo test` shows a regression (a test that was passing now fails)
- You cannot identify the cause within 2 attempts

```bash
git checkout -- src/          # revert all src/ changes
git checkout -- src/foo.rs    # revert one file
```

**Fix forward** when:
- Tests fail on code you just wrote (not a regression — a new test you wrote)
- The build error message directly points to the line you changed
- The fix is one targeted change, not a cascade

Never push a broken build. If you fix forward and it takes more than 2 attempts, revert.

## Commit After Every Successful Change

Do not batch multiple changes into one commit. Each logical change gets its own commit:
- Add a struct → commit
- Add a test → commit
- Wire it into main.rs → commit

This makes rollback surgical and makes the git log readable.

## Checking Your Own Commit Quality

Before committing, ask:
1. Does the summary describe the change, not the session? ("add /foo command" not "Day 3 work")
2. Is there a body with at least one sentence explaining why?
3. If this addresses a goal, is the G-ID in the body?
4. If this closes a GitHub issue, is "Fixes #N" in the body?

## Common Mistakes to Avoid

- Empty body on chore/wrap-up commits — always write what happened in the session
- `git add -A` when only src/ changed — stage specific files
- Committing ISSUES_TODAY.md or ISSUE_RESPONSE_*.md — these are ephemeral, gitignored
- Using `git commit --amend` on a pushed commit — never amend published history
- Summary lines starting with capital letter or ending with period
