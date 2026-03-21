# Proposed Changes to evolve.sh

## Proposal 1: Enforce non-empty commit body before push (Issue #48, G-039)

### Why This Is Needed

The `git-discipline` skill (`skills/git-discipline/SKILL.md`) explicitly requires every commit to have a
non-empty body — including wrap-up commits. The rule is: *"A commit without a body is incomplete. No
exceptions."*

However, `evolve.sh` does not check whether the wrap-up commit (or any other commit made during the
session) has a body before running `git push`. This means a session can silently push a commit that
violates the discipline rule. The constraint exists in the skill, but there is no enforcement at the push
boundary.

This proposal closes that gap.

---

### Where to Add It

In `evolve.sh`, locate the wrap-up commit block. It will look something like:

```bash
git add GOALS.md METRICS.md JOURNAL.md cycle_summary.json 2>/dev/null || true
git commit -m "chore: Day X Session Y wrap-up" || true
git push origin main
```

Insert the snippet below **after the wrap-up commit and before `git push`**.

---

### The Bash Snippet

```bash
# --- Enforce non-empty commit body before push (git-discipline skill) ---
LAST_COMMIT_BODY=$(git log -1 --format="%b" | tr -d '[:space:]')
if [ -z "$LAST_COMMIT_BODY" ]; then
  echo "[evolve] WARNING: last commit has no body — auto-amending with minimal body"
  LAST_SUBJECT=$(git log -1 --format="%s")
  COMMIT_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  git commit --amend --no-edit -m "${LAST_SUBJECT}

Auto-generated body: session wrap-up at ${COMMIT_DATE}.
No manual body was provided; amended by evolve.sh to satisfy git-discipline skill."
fi
# --- End enforcement ---
git push origin main
```

---

### How It Works

1. After the wrap-up commit, `git log -1 --format="%b"` extracts the commit body (everything after the
   blank line following the subject).
2. The body is stripped of whitespace. If it is empty, the commit is amended in-place using
   `git commit --amend --no-edit` with a generated body that includes the timestamp.
3. If the body is already present (the normal case), the check passes silently and `git push` runs
   immediately.
4. The amend happens **before** `git push`, so no published history is ever rewritten.

---

### Notes for the Operator

- `evolve.sh` is mounted read-only in the Axonix Docker container, so Axonix cannot apply this change
  itself. The operator must edit the host copy and redeploy.
- This only amends the **last** commit. If an earlier commit in the session has an empty body, that is
  not caught here. A stricter version could loop over all commits since the last push
  (`git log origin/main..HEAD --format="%H %b"`) and amend each one — but that adds complexity for a
  rare case.
- The `--no-edit` flag on `git commit --amend` preserves the subject line unchanged; only the body is
  replaced.
