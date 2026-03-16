issue_number: 15
status: partial
comment: Here is my honest token audit. Per your instructions, this is analysis only — no changes made.

**Where tokens go (based on session logs: 18–50k per session):**

1. **File reads at session start — biggest cost.** Every session reads IDENTITY.md, CAPABILITIES.md, ROADMAP.md, GOALS.md, LEARNINGS.md, METRICS.md, COMMIT_CONVENTIONS.md, all .rs files (13 files, ~4,000+ lines of Rust), JOURNAL.md (~80 lines), docs/index.html, and ISSUES_TODAY.md. This is the dominant cost — we're likely sending 6,000–10,000 tokens of context before any work begins.

2. **Source code is the largest payload.** The 13 .rs files total ~4,800 lines. Sending all of them every session costs roughly 15,000–20,000 tokens. Most sessions only modify 1–3 files, but we read all 13.

3. **Long JOURNAL.md grows each session.** Currently 80 lines, adds ~10 lines per session. In 30 more sessions this is 380 lines of context sent every session.

4. **Test output echoes large blocks.** When tests pass and I echo all 208 test names, that's ~4,000 chars of output going into context.

**Proposed compression strategies (for your review):**

| Strategy | Estimated savings | Risk | Notes |
|----------|------------------|------|-------|
| Only read src files that changed since last session | 50–70% of src cost | Medium — need reliable change detection | Use `git diff HEAD~1 --name-only` |
| Summarize JOURNAL.md to last 5 entries instead of full file | ~60% of journal cost | Low | Last 5 entries = last 1–2 days, enough for continuity |
| Compress METRICS.md reads — only send last 5 rows | ~40% of metrics cost | Low | Full history rarely needed for decisions |
| Skip reading unchanged config files (IDENTITY, ROADMAP, etc.) | 20–30% of context cost | Low | These change rarely — read on first session of day only |
| Use `cargo test --quiet` and only print summary line | ~2,000 tokens saved | Very low | Already partially done |

**My recommendation (when you're ready):**
Start with summarizing JOURNAL.md (low risk, guaranteed savings each session) and reading only changed .rs files. These two changes alone would cut session costs by ~30–40% without sacrificing any capability or self-verification. The file change detection needs one new line in the session prompt.

**Caveat:** I can't know exactly how many tokens the session prompt itself costs without instrumentation. If you want precision data, I can add a token counter that logs pre-prompt vs post-prompt token counts to METRICS.md — that would let us measure actual savings from any change.

DO NOT CLOSE — awaiting your direction on which strategy to implement.
