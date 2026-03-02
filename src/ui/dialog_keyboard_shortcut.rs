use crate::ui_state::UIState;
// use crate::utils::centered_rect;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear, List, ListItem, ListState};

use crate::ui::widget::{Widget, WidgetResult};

pub struct ShortcutsDialog {
    pub scroll_offset: usize,
}

impl Default for ShortcutsDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutsDialog {
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    fn get_shortcuts() -> Vec<(&'static str, &'static str)> {
        vec![
            ("General", ""),
            ("F10", "Activate Main Menu"),
            ("Ctrl+q", "Quit"),
            ("Ctrl+o", "Open File"),
            ("Alt+o (Ctrl+Shift+o)", "Open Recent Projects"),
            ("Ctrl+s", "Save Project"),
            ("Alt+s (Ctrl+Shift+s)", "Save Project As"),
            ("Ctrl+e", "Export Project (ASM)"),
            ("Alt+e (Ctrl+Shift+e)", "Export Project As (ASM)"),
            ("Alt+d (Ctrl+Shift+d)", "Document Settings"),
            ("Alt+p (Ctrl+,)", "Settings"),
            ("u", "Undo"),
            ("Ctrl+r", "Redo"),
            (
                "Tab",
                "Switch Pane (Disasm/Hex Dump/Sprites/Charset/Debugger)",
            ),
            ("Alt+1 (Ctrl+1)", "Toggle Blocks View"),
            ("Alt+2 (Ctrl+2)", "Toggle Hex Dump View"),
            ("Alt+3 (Ctrl+3)", "Toggle Sprites View"),
            ("Alt+4 (Ctrl+4)", "Toggle Charset View"),
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
            ("Backspace", "Navigate Back"),
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
            ("c", "Code"),
            ("b", "Byte"),
            ("w", "Word"),
            ("a", "Address"),
            ("p", "Petscii Text"),
            ("s", "Screencode Text"),
            ("e", "External File"),
            ("?", "Undefined"),
            ("d / D", "Next/Prev Immediate Mode Format"),
            ("[", "Pack Lo/Hi Address (Immediate Mode)"),
            ("]", "Pack Hi/Lo Address (Immediate Mode)"),
            ("<", "Lo/Hi Address"),
            (">", "Hi/Lo Address"),
            (",", "Lo/Hi Word"),
            (".", "Hi/Lo Word"),
            (";", "Side Comment"),
            (":", "Line Comment"),
            ("l", "Label"),
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
        ]
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

        let shortcuts = Self::get_shortcuts();

        let items: Vec<ListItem> = shortcuts
            .into_iter()
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
                    let content = format!("{:<25} {}", key, desc);
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
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down();
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k') => {
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
