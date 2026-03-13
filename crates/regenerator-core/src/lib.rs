// Stability: deny panic-inducing patterns in production code.
// Tests are exempt so they can keep using .unwrap() / .expect() ergonomically.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod analyzer;
pub mod assets;
pub mod commands;
pub mod config;
pub mod core;
pub mod cpu;

pub use core::Core;
pub mod disassembler;
pub mod event;
pub mod exporter;
pub mod navigation;
pub mod parser;
pub mod state;
pub mod utils;
pub mod vice;
pub mod view_state;

#[cfg(feature = "mcp")]
pub mod mcp;
