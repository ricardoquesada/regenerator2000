//! Modular domain action handlers for Regenerator 2000 core engine.

use crate::event::CoreEvent;
use crate::state::AppState;
use crate::state::actions::AppAction;
use crate::view_state::CoreViewState;

pub mod debug_handler;
pub mod disassembly_handler;
pub mod file_handler;
pub mod navigation_handler;

pub use debug_handler::DebugActionHandler;
pub use disassembly_handler::DisassemblyActionHandler;
pub use file_handler::FileActionHandler;
pub use navigation_handler::NavigationActionHandler;

pub use crate::error::CoreError;

/// Execution context provided to action handlers during action processing.
pub struct ActionContext<'a> {
    /// Mutable reference to application persistent state [`AppState`].
    pub state: &'a mut AppState,
    /// Mutable reference to application view state [`CoreViewState`].
    pub view: &'a mut CoreViewState,
    /// Vector of emitted core events [`CoreEvent`].
    pub events: &'a mut Vec<CoreEvent>,
}

impl<'a> ActionContext<'a> {
    /// Helper to preserve the cursor's logical position (address) across state changes.
    pub fn preserve_cursor<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Self),
    {
        let cursor_line = self.state.disassembly.get(self.view.cursor_index);
        let current_addr = cursor_line.map(|l| l.address);
        let saved_cursor_index = self.view.cursor_index;

        f(self);

        if let Some(addr) = current_addr {
            if let Some(idx) = self.state.get_line_index_containing_address(addr) {
                self.view.cursor_index = idx;
            } else if let Some(idx) = self.state.get_line_index_for_address(addr) {
                self.view.cursor_index = idx;
            } else {
                let max_idx = self.state.disassembly.len().saturating_sub(1);
                self.view.cursor_index = saved_cursor_index.min(max_idx);
            }
        }
    }
}

/// Trait implemented by domain-specific action handlers.
pub trait DomainActionHandler {
    /// Attempts to handle the given action.
    ///
    /// Returns `Ok(true)` if the action was handled by this domain handler,
    /// `Ok(false)` if it was unhandled, or `Err(CoreError)` if an error occurred.
    ///
    /// # Errors
    ///
    /// Returns a [`CoreError`] if processing the action fails.
    fn handle_action(
        &self,
        action: &AppAction,
        ctx: &mut ActionContext<'_>,
    ) -> Result<bool, CoreError>;
}
