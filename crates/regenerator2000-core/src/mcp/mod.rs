//! Model Context Protocol (MCP) server implementation.
//!
//! This module provides JSON-RPC request dispatch, HTTP and stdio transports,
//! and tool handlers for programmatic disassembly analysis and modification.

pub mod handler;
pub mod http;
pub mod stdio;
pub mod tools;
pub mod types;
