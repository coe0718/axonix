//! axonix — a coding agent that evolves itself.
//!
//! This crate provides the modular components of the axonix agent:
//! - `cli` — command-line argument parsing and help output
//! - `render` — ANSI colors, text truncation, usage display
//! - `cost` — token cost estimation per model
//! - `conversation` — saving conversations to markdown
//! - `github` — GitHub API integration (issue comments as axonix-bot or owner)
//! - `health` — system health metrics (CPU, memory, disk, uptime)
//! - `lint` — YAML and Caddyfile validation (for docker compose, Caddy server config)
//! - `memory` — persistent key-value memory store (.axonix/memory.json)
//! - `ssh` — multi-device management via SSH
//! - `telegram` — Telegram bot integration (notifications + inbound /ask commands)
//! - `twitter` — Twitter API integration (session announcements via OAuth 1.0a)
//! - `bluesky` — Bluesky AT Protocol integration (free-tier social posting)

pub mod bluesky;
pub mod cli;
pub mod conversation;
pub mod cost;
pub mod github;
pub mod health;
pub mod lint;
pub mod memory;
pub mod predictions;
pub mod render;
pub mod repl;
pub mod ssh;
pub mod telegram;
pub mod twitter;
