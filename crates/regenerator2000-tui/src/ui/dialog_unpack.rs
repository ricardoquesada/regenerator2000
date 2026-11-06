use crate::state::AppState;
use crate::ui_state::{AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};
use regenerator2000_core::unpacker::UnpackConfig;

use crate::ui::widget::{Widget, WidgetResult};

pub struct UnpackDialog {
    pub entry_input: String,
    pub ret_input: String,
    pub dep_input: String,
    pub max_inst_input: String,
    pub active_field: usize, // 0: entry, 1: return, 2: depacker, 3: max_inst, 4: Unpack button, 5: Cancel button
    cached_file_info: std::sync::OnceLock<regenerator2000_core::state::FileInfo>,
}

impl Default for UnpackDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl UnpackDialog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entry_input: String::new(),
            ret_input: String::new(),
            dep_input: String::new(),
            max_inst_input: "50000000".to_string(),
            active_field: 0,
            cached_file_info: std::sync::OnceLock::new(),
        }
    }

    fn build_config(&self) -> Result<UnpackConfig, String> {
        let mut config = UnpackConfig::default();

        let entry_clean = self.entry_input.trim();
        if !entry_clean.is_empty() {
            let val = u16::from_str_radix(entry_clean, 16)
                .map_err(|_| "Invalid Entry Point (expected 4-digit hex, e.g. 0810)".to_string())?;
            config.forced_entry = Some(val);
        }

        let ret_clean = self.ret_input.trim();
        if !ret_clean.is_empty() {
            let val = u16::from_str_radix(ret_clean, 16).map_err(|_| {
                "Invalid Return Address (expected 4-digit hex, e.g. 0800)".to_string()
            })?;
            config.forced_ret_addr = Some(val);
        }

        let dep_clean = self.dep_input.trim();
        if !dep_clean.is_empty() {
            let val = u16::from_str_radix(dep_clean, 16).map_err(|_| {
                "Invalid Depacker Address (expected 4-digit hex, e.g. 033C)".to_string()
            })?;
            config.forced_dep_addr = Some(val);
        }

        let max_inst_clean = self.max_inst_input.trim();
        if !max_inst_clean.is_empty() {
            let val = max_inst_clean.parse::<u64>().map_err(|_| {
                "Invalid Max Instructions (expected integer, e.g. 50000000)".to_string()
            })?;
            if val == 0 {
                return Err("Max Instructions must be > 0".to_string());
            }
            config.max_instructions = val;
        }

        Ok(config)
    }
}

impl Widget for UnpackDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Unpack Binary Options ", theme);

        let area = crate::utils::centered_rect_adaptive(55, 50, 0, 14, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Entry Point
                Constraint::Length(1), // Return Address
                Constraint::Length(1), // Depacker Address
                Constraint::Length(1), // Max Instructions
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Buttons
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Entropy & Packer Info
                Constraint::Min(1),    // Help text
            ])
            .split(inner);

        // --- 0. Entry Point ---
        let entry_selected = self.active_field == 0;
        let entry_text = Line::from(vec![
            Span::raw("Entry Point (Hex):    $"),
            Span::styled(
                format!("{:<4}", self.entry_input),
                if entry_selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
            Span::styled(" (blank for auto-SYS)", Style::default().fg(theme.comment)),
        ]);
        f.render_widget(Paragraph::new(entry_text), layout[0]);

        // --- 1. Return Address ---
        let ret_selected = self.active_field == 1;
        let ret_text = Line::from(vec![
            Span::raw("Return Address (Hex): $"),
            Span::styled(
                format!("{:<4}", self.ret_input),
                if ret_selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
            Span::styled(
                format!(
                    " (blank for ${:04X})",
                    app_state.settings.system.ram_start()
                ),
                Style::default().fg(theme.comment),
            ),
        ]);
        f.render_widget(Paragraph::new(ret_text), layout[1]);

        // --- 2. Depacker Address ---
        let dep_selected = self.active_field == 2;
        let dep_text = Line::from(vec![
            Span::raw("Depacker Address (Hex):$"),
            Span::styled(
                format!("{:<4}", self.dep_input),
                if dep_selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
            Span::styled(" (blank for auto)", Style::default().fg(theme.comment)),
        ]);
        f.render_widget(Paragraph::new(dep_text), layout[2]);

        // --- 3. Max Instructions ---
        let max_selected = self.active_field == 3;
        let max_text = Line::from(vec![
            Span::raw("Max Instructions:     "),
            Span::styled(
                format!("{:<10}", self.max_inst_input),
                if max_selected {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
            Span::styled(" (default 50M)", Style::default().fg(theme.comment)),
        ]);
        f.render_widget(Paragraph::new(max_text), layout[3]);

        // --- Buttons ---
        let unpack_selected = self.active_field == 4;
        let cancel_selected = self.active_field == 5;

        let unpack_style = if unpack_selected {
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
            Span::styled("  < Unpack >  ", unpack_style),
            Span::raw("    "),
            Span::styled("  < Cancel >  ", cancel_style),
        ]);
        f.render_widget(Paragraph::new(buttons_text), layout[5]);

        // --- Entropy & Packer Info (Bottom) ---
        let file_info = self.cached_file_info.get_or_init(|| app_state.file_info());
        let status_str = if let Some(name) = file_info.packer_name {
            format!("Packed with {name}")
        } else if file_info.entropy >= app_state.system_config.entropy_threshold {
            "High Entropy (likely compressed)".to_string()
        } else {
            "Normal Entropy".to_string()
        };
        let info_line = Line::from(vec![
            Span::raw("Entropy: "),
            Span::styled(
                format!("{:.2} / 8.00", file_info.entropy),
                if file_info.entropy >= app_state.system_config.entropy_threshold {
                    Style::default().fg(theme.error_fg)
                } else {
                    Style::default().fg(theme.dialog_fg)
                },
            ),
            Span::styled(
                format!(" ({status_str})"),
                Style::default().fg(theme.comment),
            ),
        ]);
        f.render_widget(Paragraph::new(info_line), layout[7]);

        // --- Help text ---
        let help =
            Paragraph::new("Press Tab/Shift+Tab to navigate, Enter to submit, Esc to cancel.")
                .style(Style::default().fg(theme.comment));
        f.render_widget(help, layout[8]);

        // Set cursor position (clamped to the active row area)
        let cursor_target = if entry_selected {
            Some((
                layout[0],
                23u16.saturating_add(self.entry_input.chars().count() as u16),
            ))
        } else if ret_selected {
            Some((
                layout[1],
                23u16.saturating_add(self.ret_input.chars().count() as u16),
            ))
        } else if dep_selected {
            Some((
                layout[2],
                23u16.saturating_add(self.dep_input.chars().count() as u16),
            ))
        } else if max_selected {
            Some((
                layout[3],
                22u16.saturating_add(self.max_inst_input.chars().count() as u16),
            ))
        } else {
            None
        };
        if let Some((row_area, offset)) = cursor_target
            && row_area.width > 0
            && row_area.height > 0
        {
            let cursor_x = row_area
                .x
                .saturating_add(offset)
                .min(row_area.right().saturating_sub(1));
            f.set_cursor_position((cursor_x, row_area.y));
        }
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
                if self.active_field == 5 {
                    self.active_field = 4;
                }
                WidgetResult::Handled
            }
            KeyCode::Right => {
                if self.active_field == 4 {
                    self.active_field = 5;
                }
                WidgetResult::Handled
            }
            KeyCode::Backspace => {
                match self.active_field {
                    0 => {
                        self.entry_input.pop();
                    }
                    1 => {
                        self.ret_input.pop();
                    }
                    2 => {
                        self.dep_input.pop();
                    }
                    3 => {
                        self.max_inst_input.pop();
                    }
                    _ => {}
                }
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if self.active_field == 5 {
                    WidgetResult::Close
                } else {
                    match self.build_config() {
                        Ok(config) => {
                            ui_state.set_status_message("Unpacking with custom parameters...");
                            WidgetResult::Action(AppAction::UnpackBinaryWithConfig(config))
                        }
                        Err(err) => {
                            ui_state.set_status_message(err);
                            WidgetResult::Handled
                        }
                    }
                }
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                match self.active_field {
                    0 if c.is_ascii_hexdigit() && self.entry_input.len() < 4 => {
                        self.entry_input.push(c.to_ascii_uppercase());
                    }
                    1 if c.is_ascii_hexdigit() && self.ret_input.len() < 4 => {
                        self.ret_input.push(c.to_ascii_uppercase());
                    }
                    2 if c.is_ascii_hexdigit() && self.dep_input.len() < 4 => {
                        self.dep_input.push(c.to_ascii_uppercase());
                    }
                    3 if c.is_ascii_digit() && self.max_inst_input.len() < 12 => {
                        self.max_inst_input.push(c);
                    }
                    _ => {}
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

    #[test]
    fn test_unpack_dialog_build_config_defaults() {
        let dialog = UnpackDialog::new();
        let config = dialog.build_config().unwrap();
        assert_eq!(config, UnpackConfig::default());
    }

    #[test]
    fn test_unpack_dialog_build_config_custom_values() {
        let mut dialog = UnpackDialog::new();
        dialog.entry_input = "0810".to_string();
        dialog.ret_input = "0800".to_string();
        dialog.dep_input = "033C".to_string();
        dialog.max_inst_input = "1000000".to_string();

        let config = dialog.build_config().unwrap();
        assert_eq!(config.forced_entry, Some(0x0810));
        assert_eq!(config.forced_ret_addr, Some(0x0800));
        assert_eq!(config.forced_dep_addr, Some(0x033C));
        assert_eq!(config.max_instructions, 1_000_000);
    }

    #[test]
    fn test_unpack_dialog_build_config_invalid_hex() {
        let mut dialog = UnpackDialog::new();
        dialog.entry_input = "ZZZZ".to_string();
        assert!(dialog.build_config().is_err());
    }

    #[test]
    fn test_unpack_dialog_action_trigger() {
        let mut dialog = UnpackDialog::new();
        dialog.entry_input = "0810".to_string();
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        let res = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );

        let expected = UnpackConfig {
            forced_entry: Some(0x0810),
            ..Default::default()
        };

        assert_eq!(
            res,
            WidgetResult::Action(AppAction::UnpackBinaryWithConfig(expected))
        );
    }

    #[test]
    fn test_unpack_dialog_render_clamps_cursor_on_narrow_terminal() {
        use ratatui::Terminal;
        use ratatui::backend::{Backend, TestBackend};

        let mut dialog = UnpackDialog::new();
        dialog.entry_input = "0810".to_string();
        let app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        let backend = TestBackend::new(20, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                dialog.render(f, f.area(), &app_state, &mut ui_state);
            })
            .unwrap();

        let pos = terminal.backend_mut().get_cursor_position().unwrap();
        assert!(
            pos.x < 20 && pos.y < 12,
            "Expected clamped cursor inside 20x12 terminal, got {pos:?}"
        );
    }

    #[test]
    fn test_unpack_dialog_render_uses_system_ram_start() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let dialog = UnpackDialog::new();
        let mut app_state = AppState::new();
        app_state.settings.system = crate::state::System::from("C128");
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        let backend = TestBackend::new(80, 20);
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
        assert!(rendered.contains("(blank for $1C00)"));
    }
}
