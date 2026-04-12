//! Re-exports from split sub-modules for backwards compatibility.
//!
//! `cmd_dispatch.rs` was 390 lines and violated the 300-line rule (Issue #110).
//! Split into:
//! - `handled_result.rs`  — async marker dispatcher (`handle_handled_result`)
//! - `inline_handlers.rs` — /status /context /tokens inline handlers
pub use super::handled_result::handle_handled_result;
pub use super::inline_handlers::handle_not_a_command_inline;
