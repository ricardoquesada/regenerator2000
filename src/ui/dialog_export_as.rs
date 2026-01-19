use crate::state::AppState;
use crate::theme::Theme;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct ExportAsDialog {
    pub active: bool,
    pub input: String,
}

impl ExportAsDialog {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.input.clear();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render(f: &mut Frame, area: Rect, dialog: &ExportAsDialog, theme: &Theme) {
    if !dialog.active {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Export Project As... ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

pub fn handle_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    let dialog = &mut ui_state.export_as_dialog;
    match key.code {
        KeyCode::Esc => {
            dialog.close();
            ui_state.set_status_message("Ready");
        }
        KeyCode::Enter => {
            let filename = dialog.input.clone();
            if !filename.is_empty() {
                let mut path = ui_state.file_dialog_current_dir.join(filename);
                if path.extension().is_none() {
                    path.set_extension("asm");
                }
                app_state.export_path = Some(path.clone());
                if let Err(e) = crate::exporter::export_asm(app_state, &path) {
                    ui_state.set_status_message(format!("Error exporting: {}", e));
                } else {
                    ui_state.set_status_message("Project Exported");
                    ui_state.export_as_dialog.close();
                }
            }
        }
        KeyCode::Backspace => {
            dialog.input.pop();
        }
        KeyCode::Char(c) => {
            dialog.input.push(c);
        }
        _ => {}
    }
}
