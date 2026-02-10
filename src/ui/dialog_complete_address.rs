use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct CompleteAddressDialog {
    pub input: String,
    pub known_byte: u8,
    pub lo_first: bool, // true if known_byte is low byte, false if high byte
    pub address: u16,   // Address of the instruction to modify
}

impl CompleteAddressDialog {
    pub fn new(known_byte: u8, lo_first: bool, address: u16) -> Self {
        Self {
            input: String::new(),
            known_byte,
            lo_first,
            address,
        }
    }

    pub fn get_complete_address(&self) -> Option<u16> {
        if self.input.len() == 2
            && let Ok(entered_byte) = u8::from_str_radix(&self.input, 16)
        {
            let (lo, hi) = if self.lo_first {
                (self.known_byte, entered_byte)
            } else {
                (entered_byte, self.known_byte)
            };
            return Some(((hi as u16) << 8) | (lo as u16));
        }
        None
    }

    fn get_display_text(&self) -> String {
        if self.lo_first {
            // Known byte is low, asking for high
            // Display: $__XX where XX is known low byte
            format!(
                "Lo/Hi Address: ${}{}{}{}",
                if self.input.is_empty() {
                    '_'
                } else {
                    self.input.chars().nth(0).unwrap()
                },
                if self.input.len() < 2 {
                    '_'
                } else {
                    self.input.chars().nth(1).unwrap()
                },
                format!("{:02X}", self.known_byte).chars().nth(0).unwrap(),
                format!("{:02X}", self.known_byte).chars().nth(1).unwrap()
            )
        } else {
            // Known byte is high, asking for low
            // Display: $XX__ where XX is known high byte
            format!(
                "Hi/Lo Address: ${}{}{}{}",
                format!("{:02X}", self.known_byte).chars().nth(0).unwrap(),
                format!("{:02X}", self.known_byte).chars().nth(1).unwrap(),
                if self.input.is_empty() {
                    '_'
                } else {
                    self.input.chars().nth(0).unwrap()
                },
                if self.input.len() < 2 {
                    '_'
                } else {
                    self.input.chars().nth(1).unwrap()
                }
            )
        }
    }
}

impl Widget for CompleteAddressDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = if self.lo_first {
            " Complete Lo/Hi Address (Enter High Byte) "
        } else {
            " Complete Hi/Lo Address (Enter Low Byte) "
        };
        let block = crate::ui::widget::create_dialog_block(title, theme);

        // let layout = Layout::default()
        //     .direction(Direction::Vertical)
        //     .constraints([
        //         Constraint::Fill(1),
        //         Constraint::Length(3),
        //         Constraint::Fill(1),
        //     ])
        //     .split(area);

        let area = crate::utils::centered_rect_adaptive(50, 40, 0, 3, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let display_text = self.get_display_text();
        let input = Paragraph::new(display_text).block(block).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input, area);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Cancelled");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                if let Some(target_address) = self.get_complete_address() {
                    // Create immediate format for this instruction
                    let fmt = if self.lo_first {
                        crate::state::ImmediateFormat::HighByte(target_address)
                    } else {
                        crate::state::ImmediateFormat::LowByte(target_address)
                    };

                    let old_fmt = app_state
                        .immediate_value_formats
                        .get(&self.address)
                        .copied();

                    let command = crate::commands::Command::SetImmediateFormat {
                        address: self.address,
                        new_format: Some(fmt),
                        old_format: old_fmt,
                    };

                    command.apply(app_state);
                    app_state.push_command(command);

                    // Re-analyze to generate new auto-labels for Lo/Hi addresses
                    let (new_labels, new_cross_refs) = crate::analyzer::analyze(app_state);
                    app_state.labels = new_labels;
                    app_state.cross_refs = new_cross_refs;

                    app_state.disassemble();

                    ui_state.set_status_message(format!("Set address: ${:04X}", target_address));
                    WidgetResult::Close
                } else {
                    ui_state.set_status_message("Invalid input - need 2 hex digits");
                    WidgetResult::Handled
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) => {
                if c.is_ascii_hexdigit() && self.input.len() < 2 {
                    self.input.push(c.to_ascii_uppercase());
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
