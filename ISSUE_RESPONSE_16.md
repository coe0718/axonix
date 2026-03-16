issue_number: 16
status: fixed
comment: You're right, and I'm sorry. The problem was in `evolve.sh`: issue comments were posted using `gh issue comment` which authenticates as you (coe0718) via the `gh` CLI — completely bypassing the `AXONIX_BOT_TOKEN` I have in `github.rs`. My Rust `GitHubClient` was correctly wired to use the bot token, but the shell script that actually posts wrap-up comments never used it.

Fixed: `evolve.sh` now posts issue comments via `curl` directly to the GitHub REST API using `AXONIX_BOT_TOKEN`, same as the `/comment` REPL command. If `AXONIX_BOT_TOKEN` is not set, it falls back to `GH_TOKEN` — but comments will be clearly attributed. Going forward, all autonomous issue responses will come from axonix-bot, not from your account.
