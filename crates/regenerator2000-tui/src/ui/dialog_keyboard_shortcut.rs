//! Modal keyboard shortcuts reference dialog (`Help -> Keyboard Shortcuts` / `?`).
//!
//! Defines Layer 3 of the 4-layer keyboard shortcut architecture alongside live key routing,
//! [`crate::ui::menu::MenuState::new`], and `docs/keyboard_shortcuts.md`.

use crate::state::AppState;
use crate::ui::widget::{self, Widget, WidgetResult};
use crate::ui_state::UIState;
use crate::utils;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear, List, ListItem, ListState};

/// Static entry displayed in [`ShortcutsDialog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutEntry {
    /// Key combination label or section header title.
    pub key: &'static str,
    /// Action description, or an empty string for section headers and blank spacers.
    pub description: &'static str,
}

const fn entry(key: &'static str, description: &'static str) -> ShortcutEntry {
    ShortcutEntry { key, description }
}

const SHORTCUTS: &[ShortcutEntry] = &[
    entry("General", ""),
    entry("F10", "Activate Main Menu"),
    entry("Alt+f", "File Menu"),
    entry("Alt+t", "Edit Menu"),
    entry("Alt+j", "Jump Menu"),
    entry("Alt+r", "Search Menu"),
    entry("Alt+v", "View Menu"),
    entry("Alt+u", "Debugger Menu"),
    entry("Alt+h", "Help Menu"),
    entry("Ctrl+q", "Quit"),
    entry("Ctrl+o", "Open File"),
    entry("Alt+o (Ctrl+Shift+o)", "Open Recent Projects"),
    entry("Ctrl+s", "Save Project"),
    entry("Alt+s (Ctrl+Shift+s)", "Save Project As"),
    entry("Ctrl+e", "Export to .asm"),
    entry("Alt+e (Ctrl+Shift+e)", "Export to .asm as..."),
    entry("Alt+d (Ctrl+Shift+d)", "Document Settings"),
    entry("Alt+p (Ctrl+,)", "Settings"),
    entry("u", "Undo"),
    entry("Ctrl+r", "Redo"),
    entry(
        "Tab",
        "Switch Pane (Disasm/Hex Dump/Sprites/Charset/Bitmap/Blocks/Debugger)",
    ),
    entry("Alt+1 (Ctrl+1)", "Toggle Blocks View"),
    entry("Alt+2 (Ctrl+2)", "Cycle Hex Dump View (16/8/Off)"),
    entry("Alt+3 (Ctrl+3)", "Cycle Sprites View (2-Col/1-Col/Off)"),
    entry("Alt+4 (Ctrl+4)", "Cycle Charset View (8-Col/4-Col/Off)"),
    entry("Alt+5 (Ctrl+5)", "Toggle Bitmap View"),
    entry("Alt+6 (Ctrl+6)", "Toggle Debugger Panel"),
    entry("", ""),
    entry("Navigation", ""),
    entry("Up/Down/j/k", "Move Cursor"),
    entry("PageUp/PageDown", "Page Up/Down"),
    entry("Home/End", "Start/End of File"),
    entry("Ctrl+u / Ctrl+d", "Up/Down 10 Lines"),
    entry("Alt+g (Ctrl+g)", "Jump to Address"),
    entry("Alt+Shift+g (Ctrl+Shift+g)", "Jump to Line"),
    entry("[Number] Shift+g", "Jump to Line / End"),
    entry("Enter", "Jump to Operand / Jump to Disasm"),
    entry("Delete", "Remove Scope / Exclude External Address"),
    entry("Backspace", "Navigate Back"),
    entry("Ctrl+n", "Next Unexplored Block"),
    entry("Ctrl+Shift+n", "Prev Unexplored Block"),
    entry("", ""),
    entry("Search", ""),
    entry("/", "Vim Search"),
    entry("n / N", "Next / Prev Match"),
    entry("Ctrl+f", "Search Dialog"),
    entry("F3 / Shift+F3", "Find Next / Previous"),
    entry("Ctrl+p", "Go to symbol"),
    entry("Ctrl+x", "Find Cross References"),
    entry("", ""),
    entry("Editing (Disassembly)", ""),
    entry("Shift+v", "Toggle Visual Selection Mode"),
    entry("Shift+Arrows", "Select Text"),
    entry("d", "Disassemble Address (flow analysis)"),
    entry("c", "Code"),
    entry("r", "Add Scope"),
    entry("Alt+Up / Alt+Down", "Nudge Scope Boundary"),
    entry("b", "Byte"),
    entry("w", "Word"),
    entry("a", "Address"),
    entry("p", "Petscii Text"),
    entry("s", "Screencode Text"),
    entry("Shift+e", "External File"),
    entry("?", "Undefined"),
    entry("i / I", "Next/Prev Immediate Mode Format"),
    entry("e", "Apply Enum"),
    entry("[", "Pack Lo/Hi Address (Immediate Mode)"),
    entry("]", "Pack Hi/Lo Address (Immediate Mode)"),
    entry("<", "Lo/Hi Address"),
    entry(">", "Hi/Lo Address"),
    entry(",", "Lo/Hi Word"),
    entry(".", "Hi/Lo Word"),
    entry(";", "Side Comment"),
    entry(":", "Line Comment"),
    entry("l", "Label"),
    entry("o", "Set Symbolic Offset"),
    entry("+ / -", "Increment/Decrement Symbolic Offset"),
    entry("Ctrl+b", "Toggle Bookmark"),
    entry("Alt+b (Ctrl+Shift+b)", "List Bookmarks"),
    entry("|", "Toggle Splitter"),
    entry("Ctrl+a", "Analyze"),
    entry("Ctrl+k", "Toggle Collapsed Block"),
    entry("", ""),
    entry("Debugger", ""),
    entry("F2", "Toggle Breakpoint"),
    entry("Shift+F2", "Toggle Breakpoint..."),
    entry("F6", "Watchpoint"),
    entry("m", "Memory Dump..."),
    entry("F4", "Run to Cursor"),
    entry("F7", "Step Instruction"),
    entry("F8", "Step Over"),
    entry("Shift+F8", "Step Out"),
    entry("F9", "Run / Pause (Continue)"),
    entry("", ""),
    entry("Editing (Hex Dump, Sprites, Charset, Bitmap)", ""),
    entry("Shift+v", "Toggle Visual Selection Mode"),
    entry("Shift+Arrows", "Select Text"),
    entry("b", "Byte"),
    entry("m", "Toggle Multicolor (Sprites/Charset,Bitmap)"),
    entry("m / M", "Next / Prev Text mode (Hex Dump)"),
    entry("s / S", "Next / Prev Screen RAM (Bitmap)"),
    entry("x", "Screen RAM after Bitmap (Bitmap)"),
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

    /// Returns the static slice of [`ShortcutEntry`] rows for the Help -> Keyboard Shortcuts dialog.
    ///
    /// This table defines Layer 3 of the 4-layer keyboard shortcut architecture. The Pillar 3
    /// contract test checks it against [`crate::ui::menu::MenuState::new`] and
    /// `docs/keyboard_shortcuts.md`.
    #[must_use]
    pub const fn shortcuts() -> &'static [ShortcutEntry] {
        SHORTCUTS
    }

    /// Scrolls the shortcut list down by one entry, clamped to the last row.
    pub fn scroll_down(&mut self) {
        let max_offset = Self::shortcuts().len().saturating_sub(1);
        if self.scroll_offset < max_offset {
            self.scroll_offset += 1;
        }
    }

    /// Scrolls the shortcut list up by one entry, clamped to zero.
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
}

impl Widget for ShortcutsDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let area = utils::centered_rect_adaptive(60, 60, 60, 20, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area); // Clear background

        let block = widget::create_dialog_block(" Keyboard Shortcuts ", theme);

        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let items: Vec<ListItem> = Self::shortcuts()
            .iter()
            .copied()
            .map(|row| {
                if row.key.is_empty() && row.description.is_empty() {
                    ListItem::new("").style(Style::default())
                } else if row.description.is_empty() {
                    // Header
                    ListItem::new(Span::styled(
                        row.key,
                        Style::default()
                            .fg(theme.highlight_fg)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ))
                } else {
                    let content = format!("{:<25} {}", row.key, row.description);
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
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let max_offset = Self::shortcuts().len().saturating_sub(1);
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
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_offset = (self.scroll_offset + 10).min(max_offset);
                WidgetResult::Handled
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
    use crate::theme::Theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Cell;

    #[test]
    fn test_shortcuts_dialog_scrolling_and_render() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = ShortcutsDialog::new();
        let max_offset = ShortcutsDialog::shortcuts().len() - 1;

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
        let mut terminal = Terminal::new(backend).expect("creating test terminal should succeed");
        terminal
            .draw(|f| {
                dialog.render(f, f.area(), &app_state, &mut ui_state);
            })
            .expect("drawing ShortcutsDialog should succeed");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(Cell::symbol)
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
