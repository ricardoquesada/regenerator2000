// Stability: deny panic-inducing patterns in production code.
// Tests are exempt so they can keep using .unwrap() / .expect() ergonomically.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

// Re-export all core modules so existing `regenerator2000::state::*` paths continue to work.
pub use regenerator_core::*;

// TUI-only modules
pub mod events;
pub mod mcp;
pub mod navigation;
pub mod theme;
pub mod ui;
pub mod ui_state;
pub mod utils;
