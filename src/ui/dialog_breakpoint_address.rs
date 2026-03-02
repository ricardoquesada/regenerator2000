use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crate::vice::state::BreakpointKind;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub struct BreakpointAddressDialog {
    pub input: String,
}

impl BreakpointAddressDialog {
    pub fn new(prefill: Option<u16>) -> Self {
        Self {
            input: prefill.map(|a| format!("{:04X}", a)).unwrap_or_default(),
        }
    }

    fn existing_breakpoint<'a>(
        input: &str,
        app_state: &'a AppState,
    ) -> Option<&'a crate::vice::state::ViceBreakpoint> {
        u16::from_str_radix(input, 16).ok().and_then(|addr| {
            app_state
                .vice_state
                .breakpoints
                .iter()
                .find(|bp| bp.address == addr && bp.kind == BreakpointKind::Exec)
        })
    }
}

impl Widget for BreakpointAddressDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Breakpoint ", theme);

        let area = crate::utils::centered_rect_adaptive(30, 40, 0, 4, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let dollar = Style::default().fg(theme.comment);
        let addr_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(theme.comment);
        let warn = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);

        let addr_line = Line::from(vec![
            Span::styled("$", dollar),
            Span::styled(self.input.clone(), addr_style),
        ]);

        let status_line = match Self::existing_breakpoint(&self.input, app_state) {
            Some(bp) => Line::from(vec![
                Span::styled("● ", warn),
                Span::styled(
                    format!("${:04X} — Enter removes breakpoint", bp.address),
                    dim,
                ),
            ]),
            None => Line::from(Span::styled("Enter sets breakpoint", dim)),
        };

        let para = Paragraph::new(vec![addr_line, status_line]).block(block);
        f.render_widget(para, area);
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
            KeyCode::Enter => {
                if let Ok(addr) = u16::from_str_radix(&self.input, 16) {
                    WidgetResult::Action(crate::ui::menu::MenuAction::ViceSetBreakpointAt {
                        address: addr,
                    })
                } else if self.input.is_empty() {
                    WidgetResult::Close
                } else {
                    ui_state.set_status_message("Invalid hex address");
                    WidgetResult::Handled
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) if c.is_ascii_hexdigit() && self.input.len() < 4 => {
                self.input.push(c.to_ascii_uppercase());
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
