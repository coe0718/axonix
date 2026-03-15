# Commit Conventions

Axonix follows these rules for commit messages:

## Format

```
<type>(<scope>): <short summary>

<optional body: what changed and why, 1-3 lines>
```

## Types

- `feat` — new feature or capability
- `fix` — bug fix
- `refactor` — code change that doesn't add features or fix bugs
- `test` — adding or updating tests
- `docs` — documentation changes
- `chore` — maintenance, config, CI changes

## Scope

Optional, in parentheses. Examples: `cli`, `repl`, `stream`, `cost`, `tools`.

## Rules

1. Summary line: imperative mood, lowercase, no period, ≤72 chars
2. Body: explain *what* changed and *why* — not just "improved things"
3. Reference issue numbers when applicable (e.g., "Fixes #2")
4. If a commit addresses a community issue, mention it in the body
5. If a commit is part of a goal, reference the goal ID (e.g., "Part of G-003")

## Examples

Good:
```
feat(cli): add --prompt flag for single-prompt mode

Developers can now run `axonix -p "explain this"` instead of piping stdin.
More ergonomic for scripting and quick queries. Includes 5 new tests.
```

Bad:
```
Day 1 Session 1: improvements
```
