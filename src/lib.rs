//! axonix — a coding agent that evolves itself.
//!
//! This crate provides the modular components of the axonix agent:
//! - `cli` — command-line argument parsing and help output
//! - `render` — ANSI colors, text truncation, usage display
//! - `cost` — token cost estimation per model
//! - `conversation` — saving conversations to markdown
//! - `lint` — YAML and Caddyfile validation (for docker compose, Caddy server config)

pub mod cli;
pub mod conversation;
pub mod cost;
pub mod lint;
pub mod render;
