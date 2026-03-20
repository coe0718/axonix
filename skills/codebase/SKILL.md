---
name: codebase
description: How this codebase is structured, where to find things, and how to make changes safely. Read this before touching any src/ file.
---

## Module Map

```
src/main.rs          Entry point, CLI dispatch, REPL event loop, build_tools(), make_agent()
src/lib.rs           Re-exports all modules — check here to see what exists
src/cli.rs           CliArgs struct, flag parsing, print_help()
src/repl.rs          handle_command() — all /slash commands live here
src/render.rs        ANSI color constants, truncate(), format helpers
src/cost.rs          Token cost estimation per model
src/conversation.rs  save_conversation() — export to markdown
src/github.rs        GitHubClient — issue comments, discussions, git identity
src/telegram.rs      TelegramClient — send_message(), poll loop, BotCommand dispatch
src/bluesky.rs       BlueskyClient — post() via AT Protocol
src/ssh.rs           SSH tool — remote host execution
src/lint.rs          YAML + Caddyfile validation
src/health.rs        CPU/mem/disk metrics, HealthReport
src/memory.rs        MemoryStore — .axonix/memory.json key-value store
src/predictions.rs   PredictionStore — .axonix/predictions.json
src/cycle_summary.rs CycleSummary — .axonix/cycle_summary.json, from_real_data()
src/brief.rs         Brief::collect(), format_terminal(), format_telegram()
src/watch.rs         WatchConfig, run_watch() — health threshold alerts
src/bin/stream_server.rs  SSE server — /pipe (POST) and /stream (GET SSE)
```

## Adding a New REPL Command

1. Add the match arm in `src/repl.rs` → `handle_command()` (the giant match block)
2. Return `CommandResult::Handled(vec![...])` for synchronous output
3. Return `CommandResult::Handled(vec!["__marker:data".to_string()])` for async work
4. Handle the `__marker:` in `src/main.rs` in the `CommandResult::Handled` block
5. Add the command to the `/help` output (search for `println!("    /`)
6. Add tests in `#[cfg(test)]` at the bottom of `repl.rs`

## Adding a New CLI Flag

1. Add field to `CliArgs` struct in `src/cli.rs`
2. Parse it in `CliArgs::parse()` (positional args use `.position()` pattern)
3. Add to the `Some(Self { ... })` constructor
4. Add help line in `print_help()`
5. Handle in `main()` in `src/main.rs` — flags that don't need an API key go BEFORE the key check
6. Add tests in `#[cfg(test)]` at bottom of `cli.rs`

## Adding a New Sub-Agent

In `build_tools()` in `src/main.rs`:
```rust
let my_agent = SubAgentTool::new("name", Arc::clone(&provider))
    .with_description("one sentence for the parent agent to know when to call this")
    .with_system_prompt("focused role description — no planning, no reading identity files")
    .with_model(model)
    .with_api_key(api_key)
    .with_tools(arc_tools.clone())  // clone() — arc_tools is used multiple times
    .with_max_turns(N);
tools.push(Box::new(my_agent));
```
Update test `test_build_tools_count` to expect the new total.

## yoagent Patterns

- `default_tools()` returns `Vec<Box<dyn AgentTool>>` — convert to Arc for sub-agents:
  ```rust
  let arc_tools: Vec<Arc<dyn AgentTool>> = default_tools()
      .into_iter().map(|b| Arc::from(b) as Arc<dyn AgentTool>).collect();
  ```
- `with_tools()` on SubAgentTool takes `Vec<Arc<dyn AgentTool>>`, not `Vec<Box<...>>`
- `arc_tools` must be `.clone()`d for each sub-agent — it doesn't implement Copy
- `ContextConfig` is set in `make_agent()` — do not change max_context_tokens without understanding the 200K window

## Test Patterns

Tests live in `#[cfg(test)] mod tests { ... }` at the bottom of each source file.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;  // for file-based tests

    fn tmp_path(name: &str) -> std::path::PathBuf {
        TempDir::new().unwrap().into_path().join(name)
    }

    #[test]
    fn test_something() { ... }
}
```

- Use `tempfile::TempDir` for any test that touches the filesystem
- Test the error path, not just the happy path
- `repl.rs` tests use a `state()` helper that returns a default `ReplState`
- Always assert the exact output format, not just "contains something"

## Key Gotchas

- `cargo test` runs the binary tests AND integration tests — check all result lines
- `src/twitter.rs` is deleted — do not re-add it or reference it
- `scripts/evolve.sh` is mounted `:ro` in the container — propose changes in `EVOLVE_PROPOSED.md`
- `git log` with `--oneline` or a pager can segfault inside the container — use `git show HEAD` instead
- `configure_git_identity()` only runs inside Docker (checked via `/.dockerenv`) — do not call it unconditionally
- `.axonix/` directory holds runtime state (memory.json, predictions.json, cycle_summary.json) — gitignored
