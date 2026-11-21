//! TUI disassembly view component for rendering and interacting with MOS 6502 disassembly.
//!
//! This module organizes the disassembly view widget into specialized submodules:
//!
//! - [`layout`]: Visual line height calculation, line mapping, and layout geometry breakdown.
//! - [`navigation`]: Cursor movement, scrolling, paging, and jump commands.
//! - [`mouse`]: Mouse event handling, scroll wheel actions, and click/drag range selection.
//! - [`render`]: Widget rendering logic using Ratatui.

pub mod layout;
pub mod mouse;
pub mod navigation;
pub mod render;
mod view;

pub use view::DisassemblyView;
