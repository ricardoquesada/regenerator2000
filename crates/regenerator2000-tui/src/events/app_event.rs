//! Application event types routed through the TUI event loop.

use crossterm::event::Event;
use regenerator2000_core::mcp::types::McpRequest;
use regenerator2000_core::unpacker::{UnpackError, UnpackResult};
use regenerator2000_core::vice::{ViceConnectionId, ViceEvent};

/// Events dispatched through the main application channel to [`super::run_app`].
pub enum AppEvent {
    /// Raw terminal input or resize event from Crossterm.
    Crossterm(Event),
    /// Incoming Model Context Protocol (MCP) request from the HTTP server.
    Mcp(McpRequest),
    /// Fatal error or bind failure reported by the MCP HTTP server thread.
    McpError(String),
    /// User-triggered request to start the MCP HTTP server.
    McpStartRequested,
    /// User-triggered request to stop the running MCP HTTP server.
    McpStopRequested,
    /// Notification that the MCP HTTP server bound and started listening.
    McpServerStarted {
        /// Local TCP port bound by the MCP HTTP listener.
        port: u16,
    },
    /// Notification that the MCP HTTP server has shut down.
    McpServerStopped,
    /// Lifecycle or protocol event from a VICE client session.
    Vice {
        /// Session identifier of the [`ViceClient`](regenerator2000_core::vice::ViceClient) that sent the event.
        connection_id: ViceConnectionId,
        /// Lifecycle or protocol event from VICE.
        event: ViceEvent,
    },
    /// Periodic timer tick used for animations and timed dialog auto-close.
    Tick,
    /// Notification that a newer application version is available.
    UpdateAvailable(String),
    /// Progress update from the background unpacker thread (instruction count).
    UnpackProgress(u64),
    /// Completion notification from the background unpacker thread.
    UnpackComplete(Result<UnpackResult, UnpackError>),
}
