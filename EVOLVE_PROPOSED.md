# EVOLVE_PROPOSED.md — Proposed evolve.sh Changes

## G-072: Replace METRICS.md append logic with `--insert-metrics-row`

### Motivation

`evolve.sh` currently appends metrics rows to `METRICS.md` using shell string
manipulation and `echo >>`. This approach:

- Appends rows at end-of-file instead of immediately after the `|-----|` separator
  (breaking the expected display order under the table header).
- Does not deduplicate rows — stub rows written early in a session persist after the
  final write, causing duplicate Day+Session entries.

`axonix::metrics::insert_metrics_row` (merged in G-071) fixes both issues.
The G-072 CLI flag (`--insert-metrics-row`) exposes that function without requiring
an API key, making it safe to call from shell scripts.

### Proposed Change

Find every block in `evolve.sh` that appends to `METRICS.md`, for example:

```bash
# current pattern (approximate)
echo "| ${DAY} | ${SESSION} | ${DATE} | ... |" >> METRICS.md
```

Replace each such block with:

```bash
ROW="| ${DAY} | ${SESSION} | ${DATE} | ... |"
./target/release/axonix --insert-metrics-row "${ROW}"
```

### Requirements

- `axonix` must be built (`cargo build --release`) before the call is made.
  If the binary may not be present, fall back to the old `echo >>` approach with a
  warning: `echo "warn: axonix binary not found, falling back to append" >&2`.
- The `--insert-metrics-row` call exits 0 on success and 1 on failure; check `$?`.
- No API key env var is needed for this flag.

### Expected Outcome

- METRICS.md rows always appear immediately below the `|-----|` header separator,
  newest-first (or in the order inserted).
- Re-running `evolve.sh` for the same Day+Session replaces the existing row rather
  than duplicating it.
