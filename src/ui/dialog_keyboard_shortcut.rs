use crate::ui_state::UIState;
use crate::utils::centered_rect;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

use crate::ui::dialog::{Dialog, DialogResult};

pub struct ShortcutsDialog {
    pub scroll_offset: usize,
}

impl ShortcutsDialog {
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
}

impl Dialog for ShortcutsDialog {
    fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        _app_state: &crate::state::AppState,
        ui_state: &UIState,
    ) {
        let theme = &ui_state.theme;
        let area = centered_rect(60, 60, area);
        f.render_widget(Clear, area); // Clear background

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Keyboard Shortcuts ")
            .border_style(Style::default().fg(theme.dialog_border))
            .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let shortcuts = vec![
            ("General", ""),
            ("F10", "Activate Menu"),
            ("Ctrl+q", "Quit"),
            ("Ctrl+o", "Open File"),
            ("Ctrl+s", "Save Project"),
            ("Ctrl+Shift+s", "Save Project As..."),
            ("Ctrl+e", "Export .asm"),
            ("Ctrl+Shift+e", "Export .asm As..."),
            ("Ctrl+Shift+d", "Document Settings"),
            ("Ctrl+,", "Settings"),
            ("u", "Undo"),
            ("Ctrl+r", "Redo"),
            ("Tab", "Switch Pane (Disasm/Hex Dump/Sprites/Charset)"),
            ("Ctrl+2", "Toggle Hex Dump View"),
            ("Ctrl+3", "Toggle Sprites View"),
            ("Ctrl+4", "Toggle Charset View"),
            ("", ""),
            ("Navigation", ""),
            ("Up/Down/j/k", "Move Cursor"),
            ("PageUp/PageDown", "Page Up/Down"),
            ("Home/End", "Start/End of File"),
            ("Ctrl+u / Ctrl+d", "Up/Down 10 Lines"),
            ("g", "Jump to Address (Dialog)"),
            ("Ctrl+Shift+g", "Jump to Line (Dialog)"),
            ("[Number] Shift+g", "Jump to Line / End"),
            ("Enter", "Jump to Operand"),
            ("Backspace", "Navigate Back"),
            ("", ""),
            ("Search", ""),
            ("/", "Vim Search"),
            ("n / N", "Next / Prev Match"),
            ("Ctrl+F", "Search Dialog"),
            ("F3 / Shift+F3", "Find Next / Previous"),
            ("", ""),
            ("Editing", ""),
            ("Shift+v", "Toggle Visual Selection Mode"),
            ("Shift+Arrows", "Select Text"),
            ("c", "Code"),
            ("b", "Byte"),
            ("w", "Word"),
            ("a", "Address"),
            ("t", "Text"),
            ("s", "Screencode"),
            ("?", "Undefined"),
            ("d / D", "Next/Prev Imm. Format"),
            ("<", "Lo/Hi Address"),
            (">", "Hi/Lo Address"),
            (";", "Side Comment"),
            (":", "Line Comment"),
            ("l", "Label"),
            ("Ctrl+a", "Analyze"),
            ("m", "Toggle Petscii (Hex) / Multicolor (Sprites/Charset)"),
            ("Ctrl+k", "Toggle Collapsed Block"),
            ("Shift+m", "Toggle Blocks View"),
            ("|", "Toggle Splitter"),
        ];

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
    ) -> DialogResult {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                ui_state.set_status_message("Ready");
                DialogResult::Close
            }
            KeyCode::Down => {
                self.scroll_down();
                DialogResult::KeepOpen
            }
            KeyCode::Up => {
                self.scroll_up();
                DialogResult::KeepOpen
            }
            _ => DialogResult::KeepOpen,
        }
    }
}
