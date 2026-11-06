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

pub struct WatchpointAddressDialog {
    pub input: String,
    pub kind: BreakpointKind, // W is default
}

impl WatchpointAddressDialog {
    #[must_use]
    pub fn new(prefill: Option<u16>) -> Self {
        Self {
            input: prefill.map(|a| format!("{a:04X}")).unwrap_or_default(),
            kind: BreakpointKind::Store, // W default
        }
    }

    fn cycle_kind(&mut self) {
        self.kind = match self.kind {
            BreakpointKind::Store => BreakpointKind::Load,
            BreakpointKind::Load => BreakpointKind::LoadStore,
            BreakpointKind::LoadStore => BreakpointKind::Store,
            BreakpointKind::Exec => BreakpointKind::Store,
        };
    }

    fn existing_watchpoint<'a>(
        input: &str,
        app_state: &'a AppState,
    ) -> Option<&'a crate::vice::state::ViceBreakpoint> {
        u16::from_str_radix(input, 16).ok().and_then(|addr| {
            app_state
                .vice_state
                .breakpoints
                .iter()
                .find(|bp| bp.address == addr && bp.kind != BreakpointKind::Exec)
        })
    }
}

impl Widget for WatchpointAddressDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Watchpoint ", theme);

        let area = crate::utils::centered_rect_adaptive(30, 40, 0, 4, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let dollar = Style::default().fg(theme.comment);
        let addr_style = Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(theme.comment);
        let sel = Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD);
        let unsel = Style::default()
            .fg(theme.comment)
            .add_modifier(Modifier::DIM);
        let warn = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);

        let addr_line = Line::from(vec![
            Span::styled("$", dollar),
            Span::styled(self.input.clone(), addr_style),
        ]);

        let status_line = match Self::existing_watchpoint(&self.input, app_state) {
            Some(bp) => Line::from(vec![
                Span::styled("● ", warn),
                Span::styled(
                    format!("${:04X} [{}] — Enter removes", bp.address, bp.kind.label()),
                    dim,
                ),
            ]),
            None => Line::from(vec![
                Span::styled(
                    "[W]",
                    if self.kind == BreakpointKind::Store {
                        sel
                    } else {
                        unsel
                    },
                ),
                Span::raw("  "),
                Span::styled(
                    "[R]",
                    if self.kind == BreakpointKind::Load {
                        sel
                    } else {
                        unsel
                    },
                ),
                Span::raw("  "),
                Span::styled(
                    "[RW]",
                    if self.kind == BreakpointKind::LoadStore {
                        sel
                    } else {
                        unsel
                    },
                ),
                Span::styled("  Tab", dim),
            ]),
        };

        let para = Paragraph::new(vec![addr_line, status_line]).block(block);
        f.render_widget(para, area);

        // Show blinking cursor at end of input (after "$" prefix)
        f.set_cursor_position((area.x + 1 + 1 + self.input.len() as u16, area.y + 1));
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Tab => {
                // Only cycle type when no existing watchpoint at this address
                if Self::existing_watchpoint(&self.input, app_state).is_none() {
                    self.cycle_kind();
                }
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if let Ok(addr) = u16::from_str_radix(&self.input, 16) {
                    // If a watchpoint already exists, remove it (use its kind to trigger toggle)
                    let kind = Self::existing_watchpoint(&self.input, app_state)
                        .map_or(self.kind, |bp| bp.kind);
                    WidgetResult::Action(crate::state::actions::AppAction::ViceSetWatchpoint {
                        address: crate::state::Addr(addr),
                        kind,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Addr;
    use crate::state::actions::AppAction;
    use crate::theme::Theme;
    use crate::vice::state::ViceBreakpoint;
    use crossterm::event::KeyModifiers;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_watchpoint_dialog_cycle_kind_and_existing_watchpoint_toggle() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = WatchpointAddressDialog::new(None);

        // Type "d020g" -> filters 'g' and uppercases to "D020".
        for ch in ['d', '0', '2', '0', 'g'] {
            dialog.handle_input(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            );
        }
        assert_eq!(dialog.input, "D020");

        // Tab cycles Store -> Load -> LoadStore -> Store.
        assert_eq!(dialog.kind, BreakpointKind::Store);
        dialog.handle_input(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.kind, BreakpointKind::Load);
        dialog.handle_input(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.kind, BreakpointKind::LoadStore);

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Action(AppAction::ViceSetWatchpoint {
                address: Addr(0xD020),
                kind: BreakpointKind::LoadStore,
            })
        );

        // When a watchpoint already exists at $D020, Tab is ignored and Enter uses the existing kind.
        app_state.vice_state.breakpoints.push(ViceBreakpoint {
            id: 1,
            address: 0xD020,
            enabled: true,
            kind: BreakpointKind::Load,
        });
        dialog.kind = BreakpointKind::Store;
        dialog.handle_input(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.kind, BreakpointKind::Store);

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                dialog.render(f, f.area(), &app_state, &mut ui_state);
            })
            .unwrap();

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Action(AppAction::ViceSetWatchpoint {
                address: Addr(0xD020),
                kind: BreakpointKind::Load,
            })
        );
    }
}
