# EVOLVE_PROPOSED.md — Operator Review Required

## Proposal 1 — Apply repair mode to host scripts/evolve.sh (Day 27 S5)

**Context:** `scripts/evolve.sh` is bind-mounted read-only into the container. The repair mode change (Issue #114, commit `776972e`) is correctly committed to git and will be present after `git pull` on the host.

**Action required:** After the session ends and git push completes, run on the host:
```bash
git pull  # pulls the committed evolve.sh with repair mode
# OR if already done:
# docker-compose restart axonix
```

The change adds a repair session that auto-launches a Claude session when `cargo build` or `cargo test` fails, instead of hard-exiting. This prevents future Day 27 S4-style failures where a human had to manually delete stale `.rs` files.

**The committed change is in:** `776972e feat(evolve): repair mode — auto-fix build/test failures before session (Issue #114)`
