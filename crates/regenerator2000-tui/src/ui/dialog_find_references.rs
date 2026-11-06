//! Interactive TUI modal dialog for inspecting and navigating incoming typed cross-references.
//!
//! [`FindReferencesDialog`] (`Ctrl+X`) lists incoming [`XRef`] entries for a target [`Addr`] with
//! 5-character [`XRefKind::badge`](crate::state::XRefKind::badge) prefixes (`[R]  `, `[W]  `,
//! `[R/W]`, `[C]  `, `[J]  `, `[O]  `). Press `Enter` to jump to the selected source address.

use crate::state::actions::AppAction;
use crate::state::{Addr, AppState, XRef};
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState},
};

/// Modal dialog that displays incoming typed cross-references ([`XRef`]) for a target [`Addr`].
pub struct FindReferencesDialog {
    /// Target memory [`Addr`] for the cross-reference query.
    pub target_address: Addr,
    /// Incoming [`XRef`] entries sorted and deduplicated by `(from, kind)`.
    pub references: Vec<XRef>,
    /// Index of the selected entry in [`Self::references`].
    pub selected_index: usize,
    /// Formatted display rows (`"$xxxx: [B]   MNEMONIC OPERAND"`) for [`Self::references`].
    pub list_items: Vec<String>,
}

impl FindReferencesDialog {
    /// Creates a [`FindReferencesDialog`] for `target_address` with sorted and deduplicated
    /// [`XRef`] entries from [`AppState::cross_refs`].
    ///
    /// This constructor formats each entry with a 5-character left-aligned access badge
    /// (`format!("${ref_addr:04X}: {badge:<5}")`). Instruction mnemonics align in a single
    /// column across 3-character (`[R]`, `[W]`, `[C]`, `[J]`, `[O]`) and 5-character (`[R/W]`) badges.
    #[must_use]
    pub fn new(app_state: &AppState, target_address: Addr) -> Self {
        let mut refs = app_state
            .cross_refs
            .get(&target_address)
            .cloned()
            .unwrap_or_default();
        // Sort references for consistent display by (from, kind)
        refs.sort_unstable();
        refs.dedup();

        let mut list_items = Vec::new();

        if refs.is_empty() {
            list_items.push("No references found".to_string());
        } else {
            // Show typed `[R]`/`[W]`/`[R/W]`/`[C]`/`[J]`/`[O]` badges here and in MCP tools.
            // Inline side comments keep `"x-ref: $xxxx, $yyyy"` because the TUI and IR navigation
            // parsers split on `", "` and parse bare hex addresses.
            for xref in &refs {
                let ref_addr = xref.from;
                let badge = xref.kind.badge();
                let mut text = format!("${ref_addr:04X}: {badge:<5}");

                // Find line to get instruction details
                if let Some(idx) = app_state.get_line_index_for_address(ref_addr) {
                    if let Some(line) = app_state.disassembly.get(idx) {
                        text.push_str(&format!(" {} {}", line.mnemonic, line.operand));
                    }
                } else if let Some(idx) = app_state.get_line_index_containing_address(ref_addr) {
                    // Inside a block (e.g. data block)
                    if let Some(line) = app_state.disassembly.get(idx) {
                        text.push_str(&format!(
                            " {} ${:04X} (Inside Block)",
                            line.mnemonic, line.address
                        ));
                    }
                }
                list_items.push(text);
            }
        }

        Self {
            target_address,
            references: refs,
            selected_index: 0,
            list_items,
        }
    }

    /// Advances the selection to the next cross-reference entry, wrapping around to index `0`.
    pub fn next(&mut self) {
        if !self.references.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.references.len();
        }
    }

    /// Moves the selection to the previous cross-reference entry, wrapping around to the last entry.
    pub fn previous(&mut self) {
        if !self.references.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.references.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

impl Widget for FindReferencesDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = format!(" References to ${:04X} ", self.target_address);
        let block = crate::ui::widget::create_dialog_block(&title, theme);

        let area = crate::utils::centered_rect_adaptive(60, 50, 50, 14, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let items: Vec<ListItem> = self
            .list_items
            .iter()
            .map(|t| ListItem::new(t.clone()))
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(theme.menu_selected_bg)
                    .fg(theme.menu_selected_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        let mut state = ListState::default();
        state.select(Some(self.selected_index));

        f.render_stateful_widget(list, area, &mut state);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                self.next();
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                self.previous();
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if !self.references.is_empty() {
                    let addr = self.references[self.selected_index].from;
                    WidgetResult::Action(AppAction::NavigateToAddress(addr))
                } else {
                    WidgetResult::Close
                }
            }
            _ => WidgetResult::Handled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::XRefKind;
    use crate::theme::Theme;
    use crossterm::event::KeyModifiers;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_find_references_dialog_sorts_and_stores_typed_xrefs() {
        let mut app_state = AppState::new();
        let target = Addr(0xD020);
        app_state.cross_refs.insert(
            target,
            vec![
                XRef::new(Addr(0x1010), XRefKind::Write),
                XRef::new(Addr(0x1000), XRefKind::Write),
                XRef::new(Addr(0x1000), XRefKind::Read),
            ],
        );

        let mut dialog = FindReferencesDialog::new(&app_state, target);
        assert_eq!(
            dialog.references,
            vec![
                XRef::new(Addr(0x1000), XRefKind::Read),
                XRef::new(Addr(0x1000), XRefKind::Write),
                XRef::new(Addr(0x1010), XRefKind::Write),
            ]
        );

        let mut ui_state = UIState::new(Theme::default());
        let res = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(
            res,
            WidgetResult::Action(AppAction::NavigateToAddress(Addr(0x1000)))
        );

        dialog.next();
        dialog.next();
        let res2 = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(
            res2,
            WidgetResult::Action(AppAction::NavigateToAddress(Addr(0x1010)))
        );
    }

    #[test]
    fn test_find_references_dialog_renders_typed_badges() {
        let mut app_state = AppState::new();
        let target = Addr(0x2000);
        app_state.cross_refs.insert(
            target,
            vec![
                XRef::new(Addr(0x1000), XRefKind::Read),
                XRef::new(Addr(0x1003), XRefKind::Write),
                XRef::new(Addr(0x1006), XRefKind::ReadModifyWrite),
                XRef::new(Addr(0x1009), XRefKind::Call),
                XRef::new(Addr(0x100C), XRefKind::Jump),
                XRef::new(Addr(0x100F), XRefKind::Offset),
            ],
        );

        let dialog = FindReferencesDialog::new(&app_state, target);
        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut rendered = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                rendered.push_str(buffer[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        for expected_badge in ["[R]", "[W]", "[R/W]", "[C]", "[J]", "[O]"] {
            assert!(
                rendered.contains(expected_badge),
                "Expected rendered FindReferencesDialog to contain badge {expected_badge}, got:\n{rendered}"
            );
        }
    }
}
