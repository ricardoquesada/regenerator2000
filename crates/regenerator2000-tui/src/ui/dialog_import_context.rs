use crate::state::{Addr, AppState};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct ImportContextDialog {
    pub platforms: Vec<String>,
    pub selected_platform_idx: usize,
    pub origin_input: String,
    pub start_input: String,
    pub disassemble_sequence: bool,
    pub active_field: usize, // 0: Platform, 2: Origin, 3: Start, 4: Checkbox
    pub entropy: Option<f32>,
}

impl ImportContextDialog {
    #[must_use]
    pub fn new(current_platform: &str, current_origin: Addr, suggested_entry: Option<Addr>, entropy: Option<f32>) -> Self {
        let platforms = crate::assets::get_available_platforms();
        let selected_platform_idx = platforms
            .iter()
            .position(|p| p == current_platform)
            .unwrap_or(0);

        Self {
            platforms,
            selected_platform_idx,
            origin_input: format!("{:04X}", current_origin.0),
            start_input: format!("{:04X}", suggested_entry.unwrap_or(current_origin).0),
            disassemble_sequence: true,
            active_field: 0,
            entropy,
        }
    }
}

impl Widget for ImportContextDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Import Context Setup ", theme);

        // Dialog fits content: 10 lines + 2 borders
        let area = crate::utils::centered_rect_adaptive(50, 45, 0, 12, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Platform
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Origin
                Constraint::Length(1), // Start
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Checkbox
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Buttons
                Constraint::Length(1), // Warning
            ])
            .split(inner);

        // --- 1. Platform ---
        let platform_selected = self.active_field == 0;
        let platform_name = self
            .platforms
            .get(self.selected_platform_idx)
            .cloned()
            .unwrap_or_default();
        let platform_text = Line::from(vec![
            Span::raw("Platform: "),
            Span::styled(
                format!("< {} >", platform_name),
                if platform_selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
        ]);
        f.render_widget(Paragraph::new(platform_text), layout[1]);

        // --- 2. Origin ---
        let origin_selected = self.active_field == 1;
        let origin_text = Line::from(vec![
            Span::raw("Binary Origin: "),
            Span::styled(
                format!("${}", self.origin_input),
                if origin_selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
        ]);
        f.render_widget(Paragraph::new(origin_text), layout[3]);

        // --- 3. Start Address ---
        let start_selected = self.active_field == 2;
        let start_text = Line::from(vec![
            Span::raw("Binary Entry Point: "),
            Span::styled(
                format!("${}", self.start_input),
                if start_selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
        ]);
        f.render_widget(Paragraph::new(start_text), layout[4]);

        // --- 4. Checkbox ---
        let checkbox_selected = self.active_field == 3;
        let checkbox_text = Line::from(vec![
            Span::styled(
                if self.disassemble_sequence {
                    "[X]"
                } else {
                    "[ ]"
                },
                if checkbox_selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
            Span::raw(" Disassemble from Entry Point Address"),
        ]);
        f.render_widget(Paragraph::new(checkbox_text), layout[6]);

        // --- Buttons ---
        let confirm_selected = self.active_field == 4;
        let cancel_selected = self.active_field == 5;

        let confirm_style = if confirm_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        };
        let cancel_style = if cancel_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        };

        let buttons_text = Line::from(vec![
            Span::styled("  < Confirm >  ", confirm_style),
            Span::raw("    "),
            Span::styled("  < Cancel >  ", cancel_style),
        ]);
        f.render_widget(Paragraph::new(buttons_text), layout[8]);

        // --- High Entropy Warning ---
        if let Some(ent) = self.entropy {
            use ratatui::widgets::Paragraph;
            let warn_text = Line::from(vec![Span::styled(
                format!("⚠️ High Entropy ({ent:.2}). Possibly packed or compressed."),
                Style::default()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]);
            f.render_widget(
                Paragraph::new(warn_text).alignment(ratatui::layout::Alignment::Center),
                layout[9],
            );
        }

        // Show blinking cursor at end of input
        if origin_selected {
            f.set_cursor_position((
                layout[3].x + 16 + self.origin_input.len() as u16,
                layout[3].y,
            ));
        } else if start_selected {
            f.set_cursor_position((
                layout[4].x + 21 + self.start_input.len() as u16,
                layout[4].y,
            ));
        }
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
            KeyCode::Tab | KeyCode::Down => {
                self.active_field = (self.active_field + 1) % 6;
                WidgetResult::Handled
            }
            KeyCode::BackTab | KeyCode::Up => {
                if self.active_field == 0 {
                    self.active_field = 5;
                } else {
                    self.active_field -= 1;
                }
                WidgetResult::Handled
            }
            KeyCode::Left => {
                if self.active_field == 0 && !self.platforms.is_empty() {
                    if self.selected_platform_idx == 0 {
                        self.selected_platform_idx = self.platforms.len() - 1;
                    } else {
                        self.selected_platform_idx -= 1;
                    }
                } else if self.active_field == 4 || self.active_field == 5 {
                    // Toggle confirmation buttons
                    if self.active_field == 5 {
                        self.active_field = 4;
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Right => {
                if self.active_field == 0 && !self.platforms.is_empty() {
                    self.selected_platform_idx =
                        (self.selected_platform_idx + 1) % self.platforms.len();
                } else if self.active_field == 4 {
                    self.active_field = 5;
                }
                WidgetResult::Handled
            }
            KeyCode::Backspace => {
                if self.active_field == 1 {
                    self.origin_input.pop();
                } else if self.active_field == 2 {
                    self.start_input.pop();
                }
                WidgetResult::Handled
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                if self.active_field == 3 {
                    self.disassemble_sequence = !self.disassemble_sequence;
                    WidgetResult::Handled
                } else if self.active_field == 4
                    || (key.code == KeyCode::Enter
                        && (self.active_field == 1 || self.active_field == 2))
                {
                    // Apply changes
                    if let Ok(new_origin) = u16::from_str_radix(&self.origin_input, 16) {
                        if let Ok(new_start) = u16::from_str_radix(&self.start_input, 16) {
                            let platform_name = self
                                .platforms
                                .get(self.selected_platform_idx)
                                .cloned()
                                .unwrap_or_default();

                            // 1. Update Platform
                            app_state.settings.platform =
                                crate::state::Platform::from(platform_name);
                            app_state.load_system_assets();

                            // 2. Apply Origin (Trigger command so it's undoable)
                            // Note: Origin application might require a core command update or UIAction.
                            // We can use the existing ApplyOrigin action if available or implement it.

                            // For prototype, we apply directly to see feedback
                            app_state.origin = Addr(new_origin);

                            // 3. Disassemble entry sequence if requested
                            if self.disassemble_sequence {
                                let ranges =
                                    crate::analyzer::flow_analyze(app_state, Addr(new_start));
                                for range in ranges {
                                    for i in range.start..range.end {
                                        if i < app_state.block_types.len() {
                                            app_state.block_types[i] =
                                                crate::state::BlockType::Code;
                                        }
                                    }
                                }
                            }

                            app_state.disassemble();
                            ui_state.set_status_message("Context applied");

                            // Navigate cursor to Entry point
                            if let Some(idx) = app_state.get_line_index_for_address(Addr(new_start))
                            {
                                ui_state.cursor_index = idx;
                            }

                            WidgetResult::Close
                        } else {
                            ui_state.set_status_message("Invalid Start Address");
                            WidgetResult::Handled
                        }
                    } else {
                        ui_state.set_status_message("Invalid Origin Address");
                        WidgetResult::Handled
                    }
                } else if self.active_field == 5 {
                    WidgetResult::Close
                } else {
                    WidgetResult::Handled
                }
            }
            KeyCode::Char(c) => {
                if c.is_ascii_hexdigit() {
                    if self.active_field == 1 && self.origin_input.len() < 4 {
                        self.origin_input.push(c.to_ascii_uppercase());
                    } else if self.active_field == 2 && self.start_input.len() < 4 {
                        self.start_input.push(c.to_ascii_uppercase());
                    }
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
