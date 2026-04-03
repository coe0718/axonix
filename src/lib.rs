//! axonix — a coding agent that evolves itself.
//!
//! This crate provides the modular components of the axonix agent:
//! - `brief` — morning brief: surface what matters before the day starts (G-022)
//! - `db`    — SQLite structured memory (kv/sessions/goals) (G-075, Issue #91)
//! - `embeddings` — local Ollama embeddings for semantic memory search (Issues #103, #109)
//! - `cli` — command-line argument parsing and help output
//! - `conversation_memory` — persistent turn-by-turn conversation log (.axonix/conversation_memory.json)
//! - `cycle_summary` — compact session summary persisted across restarts (Issue #38)
//! - `failure_patterns` — cross-session failure pattern tracking (.axonix/failure_patterns.json) (G-066)
//! - `render` — ANSI colors, text truncation, usage display
//! - `cost` — token cost estimation per model
//! - `conversation` — saving conversations to markdown
//! - `github` — GitHub API integration (issue comments as axonix-bot or owner)
//! - `journal_archive` — journal archiving to prevent context window bloat (Issue #69, G-068)
//! - `health` — system health metrics (CPU, memory, disk, uptime)
//! - `lint` — YAML and Caddyfile validation (for docker compose, Caddy server config)
//! - `listener` — always-on Telegram listener daemon (config, stats, system prompt)
//! - `memory` — persistent key-value memory store (.axonix/memory.json)
//! - `meta_health` — meta-system health check: predictions, cycle summary, METRICS.md (G-070, Issue #83)
//! - `metrics` — ordered, deduplicated METRICS.md row writes (Issue #67, G-071)
//! - `pogo` — Pokémon GO events and promo codes from leekduck.com (via ScrapedDuck API)
//! - `predictions` — prediction tracking and self-calibration (.axonix/predictions.json)
//! - `ssh` — multi-device management via SSH
//! - `telegram` — Telegram bot integration (notifications + inbound /ask commands)
//! - `bluesky` — Bluesky AT Protocol integration (session announcements, free-tier)
//! - `watch` — health watch: periodic threshold checks + Telegram alerts (G-025)

pub mod brief;
pub mod bluesky;
pub mod db;
pub mod embeddings;
pub mod journal_archive;
pub mod conversation_memory;
pub mod listener;
pub mod cycle_summary;
pub mod cli;
pub mod conversation;
pub mod cost;
pub mod failure_patterns;
pub mod git_summary;
pub mod github;
pub mod health;
pub mod lint;
pub mod memory;
pub mod meta_health;
pub mod metrics;
pub mod pogo;
pub mod predictions;
pub mod render;
pub mod repl;
pub mod ssh;
pub mod telegram;
pub mod watch;
