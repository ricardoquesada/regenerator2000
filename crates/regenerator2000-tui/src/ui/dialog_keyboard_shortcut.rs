//! Modal keyboard shortcuts reference dialog (`Help -> Keyboard Shortcuts` / `?`).
//!
//! Defines Layer 3 of the 4-layer keyboard shortcut architecture alongside live key routing,
//! [`crate::ui::menu::MenuState::new`], and `docs/keyboard_shortcuts.md`.

use crate::ui_state::UIState;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear, List, ListItem, ListState};

use crate::ui::widget::{Widget, WidgetResult};

const SHORTCUTS: &[(&str, &str)] = &[
    ("General", ""),
    ("F10", "Activate Main Menu"),
    ("Alt+f", "File Menu"),
    ("Alt+t", "Edit Menu"),
    ("Alt+j", "Jump Menu"),
    ("Alt+r", "Search Menu"),
    ("Alt+v", "View Menu"),
    ("Alt+u", "Debugger Menu"),
    ("Alt+h", "Help Menu"),
    ("Ctrl+q", "Quit"),
    ("Ctrl+o", "Open File"),
    ("Alt+o (Ctrl+Shift+o)", "Open Recent Projects"),
    ("Ctrl+s", "Save Project"),
    ("Alt+s (Ctrl+Shift+s)", "Save Project As"),
    ("Ctrl+e", "Export to .asm"),
    ("Alt+e (Ctrl+Shift+e)", "Export to .asm as..."),
    ("Alt+d (Ctrl+Shift+d)", "Document Settings"),
    ("Alt+p (Ctrl+,)", "Settings"),
    ("u", "Undo"),
    ("Ctrl+r", "Redo"),
    (
        "Tab",
        "Switch Pane (Disasm/Hex Dump/Sprites/Charset/Bitmap/Blocks/Debugger)",
    ),
    ("Alt+1 (Ctrl+1)", "Toggle Blocks View"),
    ("Alt+2 (Ctrl+2)", "Cycle Hex Dump View (16/8/Off)"),
    ("Alt+3 (Ctrl+3)", "Cycle Sprites View (2-Col/1-Col/Off)"),
    ("Alt+4 (Ctrl+4)", "Cycle Charset View (8-Col/4-Col/Off)"),
    ("Alt+5 (Ctrl+5)", "Toggle Bitmap View"),
    ("Alt+6 (Ctrl+6)", "Toggle Debugger Panel"),
    ("", ""),
    ("Navigation", ""),
    ("Up/Down/j/k", "Move Cursor"),
    ("PageUp/PageDown", "Page Up/Down"),
    ("Home/End", "Start/End of File"),
    ("Ctrl+u / Ctrl+d", "Up/Down 10 Lines"),
    ("Alt+g (Ctrl+g)", "Jump to Address"),
    ("Alt+Shift+g (Ctrl+Shift+g)", "Jump to Line"),
    ("[Number] Shift+g", "Jump to Line / End"),
    ("Enter", "Jump to Operand / Jump to Disasm"),
    ("Delete", "Remove Scope / Exclude External Address"),
    ("Backspace", "Navigate Back"),
    ("Ctrl+n", "Next Unexplored Block"),
    ("Ctrl+Shift+n", "Prev Unexplored Block"),
    ("", ""),
    ("Search", ""),
    ("/", "Vim Search"),
    ("n / N", "Next / Prev Match"),
    ("Ctrl+f", "Search Dialog"),
    ("F3 / Shift+F3", "Find Next / Previous"),
    ("Ctrl+p", "Go to symbol"),
    ("Ctrl+x", "Find Cross References"),
    ("", ""),
    ("Editing (Disassembly)", ""),
    ("Shift+v", "Toggle Visual Selection Mode"),
    ("Shift+Arrows", "Select Text"),
    ("d", "Disassemble Address (flow analysis)"),
    ("c", "Code"),
    ("r", "Add Scope"),
    ("Alt+Up / Alt+Down", "Nudge Scope Boundary"),
    ("b", "Byte"),
    ("w", "Word"),
    ("a", "Address"),
    ("p", "Petscii Text"),
    ("s", "Screencode Text"),
    ("Shift+e", "External File"),
    ("?", "Undefined"),
    ("i / I", "Next/Prev Immediate Mode Format"),
    ("e", "Apply Enum"),
    ("[", "Pack Lo/Hi Address (Immediate Mode)"),
    ("]", "Pack Hi/Lo Address (Immediate Mode)"),
    ("<", "Lo/Hi Address"),
    (">", "Hi/Lo Address"),
    (",", "Lo/Hi Word"),
    (".", "Hi/Lo Word"),
    (";", "Side Comment"),
    (":", "Line Comment"),
    ("l", "Label"),
    ("o", "Set Symbolic Offset"),
    ("+ / -", "Increment/Decrement Symbolic Offset"),
    ("Ctrl+b", "Toggle Bookmark"),
    ("Alt+b (Ctrl+Shift+b)", "List Bookmarks"),
    ("|", "Toggle Splitter"),
    ("Ctrl+a", "Analyze"),
    ("Ctrl+k", "Toggle Collapsed Block"),
    ("", ""),
    ("Debugger", ""),
    ("F2", "Toggle Breakpoint"),
    ("Shift+F2", "Toggle Breakpoint..."),
    ("F6", "Watchpoint"),
    ("m", "Memory Dump..."),
    ("F4", "Run to Cursor"),
    ("F7", "Step Instruction"),
    ("F8", "Step Over"),
    ("Shift+F8", "Step Out"),
    ("F9", "Run / Pause (Continue)"),
    ("", ""),
    ("Editing (Hex Dump, Sprites, Charset, Bitmap)", ""),
    ("Shift+v", "Toggle Visual Selection Mode"),
    ("Shift+Arrows", "Select Text"),
    ("b", "Byte"),
    ("m", "Toggle Multicolor (Sprites/Charset,Bitmap)"),
    ("m / M", "Next / Prev Text mode (Hex Dump)"),
    ("s / S", "Next / Prev Screen RAM (Bitmap)"),
    ("x", "Screen RAM after Bitmap (Bitmap)"),
];

/// Modal help dialog that shows all application keyboard shortcuts (Layer 3).
pub struct ShortcutsDialog {
    /// Zero-based vertical scroll offset into the rendered shortcut rows.
    pub scroll_offset: usize,
}

impl Default for ShortcutsDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutsDialog {
    /// Creates a new [`ShortcutsDialog`] scrolled to the top.
    #[must_use]
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    /// Returns the static slice of shortcut categories and `(key, description)` pairs for the
    /// Help -> Keyboard Shortcuts dialog.
    ///
    /// This table defines Layer 3 of the 4-layer keyboard shortcut architecture. The Pillar 3
    /// contract test checks it against [`crate::ui::menu::MenuState::new`] and
    /// `docs/keyboard_shortcuts.md`.
    #[must_use]
    pub const fn get_shortcuts() -> &'static [(&'static str, &'static str)] {
        SHORTCUTS
    }

    pub fn scroll_down(&mut self) {
        let max_offset = Self::get_shortcuts().len().saturating_sub(1);
        if self.scroll_offset < max_offset {
            self.scroll_offset += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
}

impl Widget for ShortcutsDialog {
    fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        _app_state: &crate::state::AppState,
        ui_state: &mut UIState,
    ) {
        let theme = &ui_state.theme;
        let area = crate::utils::centered_rect_adaptive(60, 60, 60, 20, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area); // Clear background

        let block = crate::ui::widget::create_dialog_block(" Keyboard Shortcuts ", theme);

        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let items: Vec<ListItem> = Self::get_shortcuts()
            .iter()
            .copied()
            .map(|(key, desc)| {
                if key.is_empty() && desc.is_empty() {
                    ListItem::new("").style(Style::default())
                } else if desc.is_empty() {
                    // Header
                    ListItem::new(Span::styled(
                        key,
                        Style::default()
                            .fg(theme.highlight_fg)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ))
                } else {
                    let content = format!("{key:<25} {desc}");
                    ListItem::new(content).style(Style::default().fg(theme.dialog_fg))
                }
            })
            .collect();

        let list = List::new(items).block(Block::default()).highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );

        let mut state = ListState::default();
        state.select(Some(self.scroll_offset));

        f.render_stateful_widget(list, inner, &mut state);
    }

    fn handle_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        _app_state: &mut crate::state::AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let max_offset = Self::get_shortcuts().len().saturating_sub(1);
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                self.scroll_down();
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                self.scroll_up();
                WidgetResult::Handled
            }
            KeyCode::PageDown => {
                self.scroll_offset = (self.scroll_offset + 10).min(max_offset);
                WidgetResult::Handled
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                WidgetResult::Handled
            }
            KeyCode::Char('d')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.scroll_offset = (self.scroll_offset + 10).min(max_offset);
                WidgetResult::Handled
            }
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                WidgetResult::Handled
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                WidgetResult::Handled
            }
            KeyCode::End => {
                self.scroll_offset = max_offset;
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::theme::Theme;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_shortcuts_dialog_scrolling_and_render() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = ShortcutsDialog::new();
        let max_offset = ShortcutsDialog::get_shortcuts().len() - 1;

        dialog.handle_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.scroll_offset, 1);

        dialog.handle_input(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.scroll_offset, 11);

        dialog.handle_input(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.scroll_offset, max_offset);

        // Down at max_offset stays clamped.
        dialog.scroll_down();
        assert_eq!(dialog.scroll_offset, max_offset);

        dialog.handle_input(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.scroll_offset, 0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                dialog.render(f, f.area(), &app_state, &mut ui_state);
            })
            .unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("Activate Main Menu"));

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Close
        );
    }
}
