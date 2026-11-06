//! Domain action handlers for the Regenerator 2000 core engine.

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

/// Execution context that domain action handlers use to process an [`AppAction`].
pub struct ActionContext<'a> {
    /// Mutable reference to the persistent application state [`AppState`].
    pub state: &'a mut AppState,
    /// Mutable reference to the transient view state [`CoreViewState`].
    pub view: &'a mut CoreViewState,
    /// Output buffer for emitted [`CoreEvent`] notifications.
    pub events: &'a mut Vec<CoreEvent>,
}

impl<'a> ActionContext<'a> {
    /// Preserves the logical cursor position across state mutations in `f`.
    pub fn preserve_cursor<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Self),
    {
        let cursor_line = self.state.disassembly.get(self.view.cursor_index);
        let ext_addr = cursor_line.and_then(|l| l.external_label_address);
        let current_addr = cursor_line
            .filter(|l| l.external_label_address.is_none() && !self.state.is_external(l.address))
            .map(|l| l.address);
        let saved_cursor_index = self.view.cursor_index;

        f(self);

        let max_idx = self.state.disassembly.len().saturating_sub(1);
        if let Some(ext) = ext_addr {
            if let Some(idx) = self
                .state
                .disassembly
                .iter()
                .position(|l| l.external_label_address == Some(ext))
            {
                self.view.cursor_index = idx;
            } else {
                self.view.cursor_index = saved_cursor_index.min(max_idx);
            }
        } else if let Some(addr) = current_addr {
            if let Some(idx) = self.state.get_line_index_containing_address(addr) {
                self.view.cursor_index = idx;
            } else if let Some(idx) = self.state.get_line_index_for_address(addr) {
                self.view.cursor_index = idx;
            } else {
                self.view.cursor_index = saved_cursor_index.min(max_idx);
            }
        } else {
            self.view.cursor_index = saved_cursor_index.min(max_idx);
        }
    }
}

/// Trait that each domain-specific action handler implements.
pub trait DomainActionHandler {
    /// Processes `action` within `ctx`.
    ///
    /// Returns `Ok(true)` when this handler processes `action`, or `Ok(false)` when
    /// `action` belongs to another domain handler.
    ///
    /// # Errors
    ///
    /// Returns a [`CoreError`] if action processing fails.
    fn handle_action(
        &self,
        action: &AppAction,
        ctx: &mut ActionContext<'_>,
    ) -> Result<bool, CoreError>;
}
