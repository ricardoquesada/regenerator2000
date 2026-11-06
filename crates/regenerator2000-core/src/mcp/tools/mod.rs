//! Model Context Protocol (MCP) tool submodules for Regenerator 2000.
//!
//! This module organizes tool schemas and execution handlers into five submodules:
//! - [`analysis_tools`]: Read-only inspection, search, scope creation, and memory analysis tools.
//! - [`common`]: Shared parameter parsers, error constructors, and block conversion helpers.
//! - [`modification_tools`]: State-mutation tools for labels, comments, data types, splitters, immediate formats, and enums.
//! - [`navigation_tools`]: Cursor position queries and address jump tools.
//! - [`session_tools`]: Binary unpacker, project save, and undo/redo tools.

pub mod analysis_tools;
pub mod common;
pub mod modification_tools;
pub mod navigation_tools;
pub mod session_tools;
