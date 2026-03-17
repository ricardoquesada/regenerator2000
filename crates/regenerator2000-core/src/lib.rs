//! # regenerator2000-core
//!
//! Core library for [Regenerator 2000](https://github.com/ricardoquesada/regenerator2000),
//! an interactive 6502 disassembler targeting Commodore 8-bit machines.
//!
//! This crate provides all UI-agnostic logic:
//!
//! - **[`state`]** — Application state: memory, block types, labels, comments, cross-refs, undo stack.
//! - **[`disassembler`]** — 6502 instruction decoding and per-assembler formatters (64tass, ACME, KickAssembler, ca65).
//! - **[`analyzer`]** — Auto-analysis to identify code and data regions, generate labels and cross-refs.
//! - **[`commands`]** — Undoable command system (every mutation goes through `Command::apply`).
//! - **[`parser`]** — File parsers for PRG, CRT, D64/D71/D81, T64, VSF, and VICE labels.
//! - **[`exporter`]** — Assembly source code export with roundtrip verification.
//! - **[`cpu`]** — 6502/6510 opcode table, addressing modes, and undocumented opcodes.
//! - **[`vice`]** — VICE binary monitor protocol client for live debugging.
//! - **[`mcp`]** — Model Context Protocol server for programmatic access (requires `mcp` feature).

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
