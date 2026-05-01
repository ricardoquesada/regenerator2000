//! # regenerator2000-tui
//!
//! Terminal User Interface for [Regenerator 2000](https://github.com/ricardoquesada/regenerator2000),
//! built with [ratatui](https://github.com/ratatui-org/ratatui) and crossterm.
//!
//! This crate provides:
//!
//! - **[`ui`]** — Widgets and views: disassembly, hex dump, sprites, charset, bitmap, blocks, debugger, and dialogs.
//! - **[`events`]** — Event loop routing crossterm, MCP, VICE, and tick events.
//! - **[`theme`]** — Color themes (Dracula, Nord, Catppuccin, Gruvbox, Solarized, Monokai, and more).
//! - **[`ui_state`]** — Transient rendering state: cursor positions, scroll offsets, active dialog.
//!
//! All persistent state lives in [`regenerator2000_core::state::AppState`]; this crate
//! only manages the terminal rendering and user interaction layer.

// Stability: deny panic-inducing patterns in production code.
// Tests are exempt so they can keep using .unwrap() / .expect() ergonomically.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

// Re-export core modules for convenience
pub use regenerator2000_core::state;
pub use regenerator2000_core::*;

pub mod events;
pub mod theme;
pub mod theme_file;
pub mod ui;
pub mod ui_state;
pub mod utils;
