// TUI library for Regenerator 2000

// Re-export core modules for convenience
pub use regenerator_core::state;
pub use regenerator_core::*;

pub mod events;
pub mod theme;
pub mod ui;
pub mod ui_state;
pub mod utils;

// We'll keep mcp in core, but TUI might still need to bridge it.
// The MCP server logic is now in core.
