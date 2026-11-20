//! Domain action handlers for the Regenerator 2000 core engine.

mod context;
pub mod debug_handler;
pub mod disassembly_handler;
pub mod file_handler;
pub mod navigation_handler;

pub use crate::error::CoreError;
pub(crate) use context::DisassemblyViewAnchor;
pub use context::{ActionContext, DomainActionHandler};
pub use debug_handler::DebugActionHandler;
pub use disassembly_handler::DisassemblyActionHandler;
pub use file_handler::FileActionHandler;
pub use navigation_handler::NavigationActionHandler;
