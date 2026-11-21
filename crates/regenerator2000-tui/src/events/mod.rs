//! TUI event loop, global input fallback, VICE client forwarding, and MCP server bridge.

mod app_event;
pub mod input;
mod mcp;
mod runner;
mod vice;

pub use app_event::AppEvent;
pub use mcp::{spawn_mcp_server_thread, spawn_mcp_server_thread_on_port};
pub use runner::{handle_unpack_complete, process_key_event, run_app};
pub use vice::connect_vice_client;
