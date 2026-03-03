// Stability: deny panic-inducing patterns in production code.
// Tests are exempt so they can keep using .unwrap() / .expect() ergonomically.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

pub mod analyzer;
pub mod assets;
pub mod parser;

pub mod commands;
pub mod cpu;
pub mod disassembler;
pub mod events;
pub mod exporter;
pub mod state;
pub mod ui;
pub mod ui_state;

pub mod config;
pub mod mcp;
pub mod theme;
pub mod utils;
pub mod vice;
