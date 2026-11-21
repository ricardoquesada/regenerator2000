//! Modal dialog (`[` / `]`) for completing a split low/high immediate address.

use crate::state::{Addr, AppState, ImmediateFormat};
use crate::ui::view_disassembly::DisassemblyView;
use crate::ui::widget::{self, Widget, WidgetResult};
use crate::ui_state::UIState;
use crate::utils;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Clear, Paragraph};

/// Whether the fixed byte in [`CompleteAddressDialog`] is the low or high byte of the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownBytePosition {
    /// Known byte is the low byte; user enters the high byte.
    Low,
    /// Known byte is the high byte; user enters the low byte.
    High,
}

/// Modal dialog for entering the missing byte of a split low/high immediate address.
pub struct CompleteAddressDialog {
    /// Two-digit hexadecimal string typed by the user.
    pub input: String,
    /// Known half of the 16-bit target address.
    pub known_byte: u8,
    /// Position of [`Self::known_byte`] within the 16-bit target address.
    pub known_byte_position: KnownBytePosition,
    /// Address of the instruction being formatted.
    pub address: Addr,
}

impl CompleteAddressDialog {
    /// Creates a new [`CompleteAddressDialog`] for completing `known_byte` at `address`.
    #[must_use]
    pub fn new(known_byte: u8, known_byte_position: KnownBytePosition, address: Addr) -> Self {
        Self {
            input: String::new(),
            known_byte,
            known_byte_position,
            address,
        }
    }

    /// Combines the known byte and two typed hex digits into a 16-bit address, if complete.
    #[must_use]
    pub fn complete_address(&self) -> Option<u16> {
        if self.input.len() == 2
            && let Ok(entered_byte) = u8::from_str_radix(&self.input, 16)
        {
            let (lo, hi) = match self.known_byte_position {
                KnownBytePosition::Low => (self.known_byte, entered_byte),
                KnownBytePosition::High => (entered_byte, self.known_byte),
            };
            return Some((u16::from(hi) << 8) | u16::from(lo));
        }
        None
    }

    fn display_text(&self) -> String {
        // Helper: safely get nth char from a string, or '?' if out of bounds
        let nth = |s: &str, n: usize| -> char { s.chars().nth(n).unwrap_or('?') };
        let known_hex = format!("{:02X}", self.known_byte);

        match self.known_byte_position {
            KnownBytePosition::Low => {
                // Known byte is low, asking for high
                // Display: $__XX where XX is known low byte
                format!(
                    "Lo/Hi Address: ${}{}{}{}",
                    if self.input.is_empty() {
                        '_'
                    } else {
                        nth(&self.input, 0)
                    },
                    if self.input.len() < 2 {
                        '_'
                    } else {
                        nth(&self.input, 1)
                    },
                    nth(&known_hex, 0),
                    nth(&known_hex, 1)
                )
            }
            KnownBytePosition::High => {
                // Known byte is high, asking for low
                // Display: $XX__ where XX is known high byte
                format!(
                    "Hi/Lo Address: ${}{}{}{}",
                    nth(&known_hex, 0),
                    nth(&known_hex, 1),
                    if self.input.is_empty() {
                        '_'
                    } else {
                        nth(&self.input, 0)
                    },
                    if self.input.len() < 2 {
                        '_'
                    } else {
                        nth(&self.input, 1)
                    }
                )
            }
        }
    }
}

impl Widget for CompleteAddressDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = match self.known_byte_position {
            KnownBytePosition::Low => " Complete Lo/Hi Address (Enter High Byte) ",
            KnownBytePosition::High => " Complete Hi/Lo Address (Enter Low Byte) ",
        };
        let block = widget::create_dialog_block(title, theme);

        let area = utils::centered_rect_adaptive(50, 40, 0, 3, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);

        let display_text = self.display_text();
        let cursor_offset = match self.known_byte_position {
            KnownBytePosition::Low => 16 + self.input.len() as u16,
            KnownBytePosition::High => 18 + self.input.len() as u16,
        };
        let input = Paragraph::new(display_text).block(block).style(
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input, area);

        if area.width > 2 && area.height > 1 {
            let cursor_x = (area.x + 1 + cursor_offset).min(area.right().saturating_sub(2));
            f.set_cursor_position((cursor_x, area.y + 1));
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
                ui_state.set_status_message("Cancelled");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                if let Some(target_address) = self.complete_address() {
                    let restore_addr = app_state
                        .disassembly
                        .get(ui_state.cursor_index)
                        .map(|l| l.address);
                    let old_sub_cursor = ui_state.sub_cursor_index;

                    let fmt = match self.known_byte_position {
                        KnownBytePosition::Low => ImmediateFormat::LowByte(Addr(target_address)),
                        KnownBytePosition::High => ImmediateFormat::HighByte(Addr(target_address)),
                    };

                    app_state.apply_immediate_format(self.address, Some(fmt));

                    if let Some(addr) = restore_addr
                        && let Some(new_idx) = app_state.line_index_for_address(addr)
                    {
                        ui_state.cursor_index = new_idx;
                        if let Some(line) = app_state.disassembly.get(new_idx) {
                            let counts = DisassemblyView::visual_line_counts(line, app_state);
                            if old_sub_cursor < counts.total() {
                                ui_state.sub_cursor_index = old_sub_cursor;
                            } else {
                                ui_state.sub_cursor_index = 0;
                            }
                        }
                    }

                    ui_state.set_status_message(format!("Set address: ${target_address:04X}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::BlockType;
    use crate::theme::Theme;
    use crossterm::event::KeyModifiers;

    #[test]
    fn test_complete_address_batches_format_and_analysis_for_undo() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        // LDA #$02; NOP
        app_state.raw_data = vec![0xA9, 0x02, 0xEA];
        app_state.block_types = vec![BlockType::Code; 3];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = CompleteAddressDialog::new(0x02, KnownBytePosition::Low, Addr(0x1000));
        dialog.input = "10".to_string();

        let res = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(res, WidgetResult::Close);
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(ImmediateFormat::LowByte(Addr(0x1002)))
        );
        assert!(app_state.labels.contains_key(&Addr(0x1002)));

        let undo_msg = app_state.undo_last_command();
        assert_eq!(undo_msg, "Undone");
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            None
        );
        assert!(!app_state.labels.contains_key(&Addr(0x1002)));
    }
}
