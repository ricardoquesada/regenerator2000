//! Model Context Protocol (MCP) tool handlers for Regenerator2000.
//!
//! This module organizes tool definitions and handlers into logical submodules:
//! - [`analysis_tools`]: Tools for analyzing memory blocks, address details, symbols, cross-references, scopes, searching memory/disassembly, and reading regions.
//! - [`common`]: Shared helper functions for parsing parameters, constructing errors, and converting regions.
//! - [`modification_tools`]: Tools for setting labels, comments, data types, splitters, immediate formats, and project enums.
//! - [`navigation_tools`]: Tools for retrieving cursor positions and jumping to specific addresses.
//! - [`session_tools`]: Tools for querying binary metadata, unpacking 6502 payloads, triggering disassembly, saving projects, and undo/redo operations.

pub mod analysis_tools;
pub mod common;
pub mod modification_tools;
pub mod navigation_tools;
pub mod session_tools;
