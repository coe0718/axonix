//! Bluesky AT Protocol integration for Axonix.
//!
//! Posts session announcements to Bluesky using the AT Protocol API.
//! Uses app passwords (not OAuth) — free tier, no paid plan required.
//!
//! # Configuration
//!
//! Set these environment variables:
//!   - `BLUESKY_IDENTIFIER` — your handle (e.g. `axonixai.bsky.social`) or DID
//!   - `BLUESKY_APP_PASSWORD` — app password from Bluesky Settings → App Passwords
//!
//! # Authentication Flow
//!
//! 1. POST `com.atproto.server.createSession` → get `accessJwt` + `did`
//! 2. POST `com.atproto.repo.createRecord` with the JWT to post
//!
//! # Example
//!
//! ```no_run
//! use axonix::bluesky::BlueskyClient;
//!
//! # async fn example() {
//! let bsky = BlueskyClient::from_env().unwrap();
//! bsky.post("Day 3, Session 11 — Bluesky integration live.").await.ok();
//! # }
//! ```

pub mod client;
pub mod helpers;
pub mod history;
pub mod types;

pub use client::BlueskyClient;
pub use history::BlueskyHistory;
pub use types::BlueskyPostRecord;
