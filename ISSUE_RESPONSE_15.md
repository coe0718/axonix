issue_number: 15
status: partial
comment: Audit complete. Here is the plan — I have NOT implemented anything yet, as requested.

## Token Usage Audit

**Current baseline (last 9 sessions):** 18k–50k tokens/session. Median ~25k. High-water: ~50k (Day 1 S1 when I read the entire codebase cold).

## Where tokens go

1. **Session initialization read** — Every session re-reads IDENTITY.md, CAPABILITIES.md, ROADMAP.md, GOALS.md, LEARNINGS.md, METRICS.md, COMMIT_CONVENTIONS.md, all .rs source files (~1,300 lines total), JOURNAL.md, docs/index.html, ISSUES_TODAY.md. This is roughly 3,000–5,000 tokens of prompt context per session before I've done anything.

2. **Full source file reads** — When I read `src/repl.rs` (1,078 lines) or `src/telegram.rs` (535 lines) every session even when those files aren't being touched, I'm spending ~2,000–4,000 tokens on context I don't need.

3. **Redundant cargo test output** — I run `cargo test` 2–4 times per session (build check, after each change, final verify). Each run produces ~100 lines of output I read and parse.

4. **Multi-turn self-correction** — When a change fails, I read the error, form a plan, read the file again, edit, build again. This can cost 5,000–8,000 tokens per failed attempt.

5. **Journal/METRICS appending** — Small but repeatable: reading the full JOURNAL.md (69 lines) and METRICS.md (16 lines) every session even when I'm only appending one line.

## Proposed compression plan (pending your approval)

**A) Lazy source reads (saves ~2,000–4,000 tokens/session)**
Only read source files that are relevant to the session's chosen work. Instead of reading all 13 .rs files upfront, read only the ones I plan to modify. Heuristic: read file list first, then read specific files based on the issue/goal being addressed.

**B) Summarized JOURNAL.md context (saves ~500–1,000 tokens/session)**
The session prompt could include only the last 3 journal entries instead of the full file. Past entries beyond 3 are rarely relevant to current decisions.

**C) Build output filtering (saves ~300–600 tokens/session)**
Parse `cargo test` output to extract only: pass count, fail count, any FAILED lines. Discard the individual "test X ... ok" lines.

**D) Incremental source reads (saves ~1,000–3,000 tokens on unchanged sessions)**
Track a `FILES_READ_CACHE` — a list of file hashes. If a file hasn't changed since last session, skip reading it and use the cached summary.

**E) Structured METRICS.md (saves ~200 tokens/session)**
METRICS.md is a markdown table read in full. Moving to a compact format (CSV or JSON) would reduce the read cost by ~40%.

## Expected savings if all implemented: 4,000–9,000 tokens/session (16–36% reduction)

## Risk assessment
- A/D carry risk: if I read the wrong file set, I might miss context and make a bad change. Mitigated by: always reading the file that defines the current goal.
- C is safe: it's just parsing, no behavioral change.
- B is safe: recent journal entries are what matter for decisions.
- E is safe: the format change doesn't affect content.

**My recommendation:** Start with B and C (lowest risk, meaningful savings). Hold A and D until the codebase stabilizes — premature context trimming could cause me to repeat mistakes. Decide: do you want me to implement B and C this session, or hold everything?
