//! Domain action handlers for the Regenerator 2000 core engine.

use crate::disassembler::{Disassembler, DisassemblyLine};
use crate::event::CoreEvent;
use crate::state::actions::AppAction;
use crate::state::{Addr, AppState};
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

/// Logical anchor for a single disassembly row before a state rebuild.
#[derive(Debug, Clone, Copy)]
struct LineAnchor {
    raw_index: usize,
    ext_addr: Option<Addr>,
    in_range_addr: Option<Addr>,
}

/// Captures and restores disassembly cursor and selection state across disassembly rebuilds.
#[derive(Debug, Clone)]
pub(crate) struct DisassemblyViewAnchor {
    cursor: LineAnchor,
    selection: Option<LineAnchor>,
    selected_ext_addrs: Vec<Addr>,
}

impl DisassemblyViewAnchor {
    /// Captures the current cursor and optional selection anchors from `state` and `view`.
    #[must_use]
    pub(crate) fn capture(state: &AppState, view: &CoreViewState) -> Self {
        let cursor = capture_line_anchor(state, view.cursor_index);
        let mut selected_ext_addrs = Vec::new();

        if view.is_visual_mode || view.selection_start.is_some() {
            let start_idx = view.selection_start.unwrap_or(view.cursor_index);
            let end_idx = view.cursor_index;
            let (min_idx, max_idx) = (start_idx.min(end_idx), start_idx.max(end_idx));
            if let Some(slice) = state.disassembly.get(min_idx..=max_idx) {
                for line in slice {
                    if let Some(ext) = line.external_label_address {
                        selected_ext_addrs.push(ext);
                    }
                }
            }
        }

        Self {
            cursor,
            selection: view
                .selection_start
                .map(|idx| capture_line_anchor(state, idx)),
            selected_ext_addrs,
        }
    }

    /// Re-anchors `view` against the rebuilt `state.disassembly`.
    pub(crate) fn restore(self, state: &AppState, view: &mut CoreViewState) {
        view.cursor_index = resolve_line_anchor(state, self.cursor);

        if let Some(line) = state.disassembly.get(view.cursor_index) {
            let formatter = Disassembler::formatter(state.settings.assembler);
            let total = line.visual_line_counts(state, formatter).total();
            if total == 0 {
                view.sub_cursor_index = 0;
            } else if view.sub_cursor_index >= total {
                view.sub_cursor_index = total - 1;
            }
        } else {
            view.sub_cursor_index = 0;
        }

        if state.disassembly.is_empty() {
            view.scroll_index = 0;
            view.scroll_sub_index = 0;
        } else if view.scroll_index >= state.disassembly.len() {
            view.scroll_index = state.disassembly.len() - 1;
            view.scroll_sub_index = 0;
        } else if let Some(scroll_line) = state.disassembly.get(view.scroll_index) {
            let formatter = Disassembler::formatter(state.settings.assembler);
            let total = scroll_line.visual_line_counts(state, formatter).total();
            if total == 0 {
                view.scroll_sub_index = 0;
            } else if view.scroll_sub_index >= total {
                view.scroll_sub_index = total - 1;
            }
        }

        // Preserve explicit clears performed inside the mutation closure while re-anchoring
        // or clearing any selection that remained active across the rebuild.
        if view.is_visual_mode || view.selection_start.is_some() {
            let any_ext_vanished = self.selected_ext_addrs.iter().any(|&ext| {
                !state
                    .disassembly
                    .iter()
                    .any(|l| l.external_label_address == Some(ext))
            });
            if any_ext_vanished {
                view.selection_start = None;
                view.is_visual_mode = false;
            } else if let Some(sel_anchor) = self.selection {
                view.selection_start = Some(resolve_line_anchor(state, sel_anchor));
            }
        }
    }
}

/// Captures a [`LineAnchor`] for row `idx` in `state.disassembly`.
#[must_use]
fn capture_line_anchor(state: &AppState, idx: usize) -> LineAnchor {
    let line = state.disassembly.get(idx);
    let ext_addr = line.and_then(|l| l.external_label_address);
    let in_range_addr = line
        .filter(|l| l.external_label_address.is_none() && !state.is_external(l.address))
        .map(|l| l.address);
    LineAnchor {
        raw_index: idx,
        ext_addr,
        in_range_addr,
    }
}

/// Snaps `idx` forward to the next navigable line, or backward when none exists forward.
#[must_use]
fn snap_to_navigable(disassembly: &[DisassemblyLine], idx: usize) -> usize {
    if disassembly.is_empty() {
        return 0;
    }
    let clamped = idx.min(disassembly.len() - 1);
    if disassembly[clamped].is_navigable() {
        return clamped;
    }
    if let Some(rel) = disassembly[clamped..]
        .iter()
        .position(DisassemblyLine::is_navigable)
    {
        return clamped + rel;
    }
    if let Some(prev) = disassembly[..clamped]
        .iter()
        .rposition(DisassemblyLine::is_navigable)
    {
        return prev;
    }
    clamped
}

/// Resolves `anchor` to a navigable row index in the rebuilt `state.disassembly`.
#[must_use]
fn resolve_line_anchor(state: &AppState, anchor: LineAnchor) -> usize {
    let max_idx = state.disassembly.len().saturating_sub(1);
    let candidate = if let Some(ext) = anchor.ext_addr {
        state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(ext))
            .or_else(|| {
                state
                    .disassembly
                    .iter()
                    .position(|l| l.external_label_address.is_some_and(|a| a > ext))
            })
            .or_else(|| {
                state
                    .disassembly
                    .iter()
                    .rposition(|l| l.external_label_address.is_some_and(|a| a < ext))
            })
            .or_else(|| {
                state
                    .disassembly
                    .iter()
                    .position(|l| !l.bytes.is_empty() || l.is_collapsed)
            })
            .unwrap_or_else(|| anchor.raw_index.min(max_idx))
    } else if let Some(addr) = anchor.in_range_addr {
        state
            .get_line_index_containing_address(addr)
            .or_else(|| state.get_line_index_for_address(addr))
            .unwrap_or_else(|| anchor.raw_index.min(max_idx))
    } else {
        anchor.raw_index.min(max_idx)
    };
    snap_to_navigable(&state.disassembly, candidate)
}

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
    /// Preserves the logical cursor position and active selection across state mutations in `f`.
    pub fn preserve_cursor<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Self),
    {
        let anchor = DisassemblyViewAnchor::capture(self.state, self.view);
        f(self);
        anchor.restore(self.state, self.view);
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
