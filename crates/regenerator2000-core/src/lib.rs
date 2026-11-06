//! # regenerator2000-core
//!
//! Core library for [Regenerator 2000](https://github.com/ricardoquesada/regenerator2000),
//! an interactive 6502 disassembler for Commodore 8-bit machines.
//!
//! This crate provides all UI-agnostic logic:
//!
//! - **[`state`]** — Application state: memory, block types, labels, comments, cross-references, and undo stack.
//! - **[`action_handlers`]** — Domain action handlers that process user and MCP actions.
//! - **[`disassembler`]** — 6502 instruction decoders and assembler formatters (64tass, ACME, KickAssembler, ca65).
//! - **[`analyzer`]** — Static analyzer that identifies code and data regions and generates labels and cross-references.
//! - **[`commands`]** — Undoable command system (`Command::apply` and `Command::undo`).
//! - **[`parser`]** — File parsers for PRG, CRT, D64/D71/D81, T64, VSF, `.dis65`, and VICE labels.
//! - **[`exporter`]** — Assembly and HTML exporters with roundtrip verification.
//! - **[`cpu`]** — 6502/6510 opcode table, addressing modes, and undocumented opcodes.
//! - **[`vice`]** — VICE binary monitor client for live debugging.
//! - **[`mcp`]** — Model Context Protocol server for programmatic access (requires the `mcp` feature).

// Deny panic-inducing patterns in production code.
// Tests remain exempt so assertions can use .unwrap() and .expect().
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod action_handlers;
pub mod analyzer;
pub mod assets;
pub mod commands;
pub mod config;
pub mod core;
pub mod cpu;
pub mod error;

pub use core::Core;
pub use error::*;

/// Specialized [`Result`](std::result::Result) type for core operations that return [`CoreError`].
pub type Result<T> = std::result::Result<T, CoreError>;
pub mod disassembler;
pub use disassembler::{
    CODE_COLUMN_WIDTH, DEFINITION_COLUMN_WIDTH, Disassembler, DisassemblyLine, DisassemblyLineKind,
    INSTRUCTION_COLUMN_WIDTH, LABEL_COLUMN_WIDTH, LabelPlacement, MidInstructionLabel,
    SPLITTER_MNEMONIC, VisualLineCounts,
};
pub mod event;
pub mod exporter;
pub mod navigation;
pub mod packers;
pub mod parser;
pub mod state;
pub use state::{XRef, XRefKind};
pub mod unpacker;
pub mod utils;
pub mod vice;
pub mod view_state;

#[cfg(feature = "mcp")]
pub mod mcp;
