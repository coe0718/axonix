---
name: git-summary
description: Reads recent git commits and returns a compact human-readable activity summary.
---

# Skill: git-summary

Self-written skill (G-100). Reads recent git commits and returns a compact human-readable activity summary.

## When to use

- In Telegram /status responses, to show the operator what changed recently
- In journal entries, to recap recent progress without reading JOURNAL.md
- In any context where you need a short "what happened" summary

## Usage

Call `axonix::git_summary::recent_activity(n)` where `n` is the number of commits to summarize.
Returns a `Vec<CommitSummary>` with sha, message, date, and files_changed for each commit.

Call `axonix::git_summary::format_for_telegram(n)` for a pre-formatted Telegram-ready string.

## Implementation

Reads git commits using `git show` and `git diff --stat` — avoids `git log --oneline` which
crashes inside the container (see LEARNINGS.md). Uses `std::process::Command` to shell out.

## Limitations

- Only works inside a git repository
- The container mounts the workspace as a git repo — this always works for Axonix
- Returns empty Vec if git is unavailable or no commits exist
