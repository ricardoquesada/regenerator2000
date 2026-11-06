use crate::state::actions::AppAction;
use crate::state::{Addr, AppState};
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

/// Operation mode for [`PromptDialog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    /// Prompt for a 16-bit hex address to navigate to.
    JumpToAddress,
    /// Prompt for a 1-based decimal visual line number to jump to.
    JumpToLine,
    /// Prompt for a 16-bit hex load/origin address.
    Origin,
    /// Prompt for a 16-bit hex VICE execution breakpoint address.
    BreakpointAddress,
    /// Prompt for a 16-bit hex VICE memory dump address.
    MemoryDumpAddress,
}

impl PromptMode {
    /// Returns the modal title bar text for this prompt mode.
    #[must_use]
    pub fn dialog_title(self) -> &'static str {
        match self {
            Self::JumpToAddress => " Jump To Address (Hex) ",
            Self::JumpToLine => " Jump To Line ",
            Self::Origin => " Change Origin (Hex) ",
            Self::BreakpointAddress => " Breakpoint ",
            Self::MemoryDumpAddress => " Memory Dump Address ",
        }
    }

    /// Returns `true` when input accepts hexadecimal digits (up to 4 chars).
    #[must_use]
    pub fn is_hex(self) -> bool {
        !matches!(self, Self::JumpToLine)
    }

    /// Returns `true` when the input line renders a leading `$` prefix.
    #[must_use]
    fn has_dollar_prefix(self) -> bool {
        matches!(
            self,
            Self::JumpToAddress | Self::BreakpointAddress | Self::MemoryDumpAddress
        )
    }

    /// Returns the maximum number of input characters accepted in this mode.
    #[must_use]
    fn max_input_len(self) -> usize {
        if self.is_hex() { 4 } else { 10 }
    }

    /// Returns the minimum dialog height (including top and bottom borders).
    #[must_use]
    fn min_height(self) -> u16 {
        match self {
            Self::BreakpointAddress | Self::MemoryDumpAddress => 4,
            Self::JumpToAddress | Self::JumpToLine | Self::Origin => 3,
        }
    }
}

/// Unified single-line hex/number input dialog for address jumps, line jumps,
/// origin changes, and VICE debugger breakpoint/memory-dump address prompts.
pub struct PromptDialog {
    pub mode: PromptMode,
    pub input: String,
}

impl PromptDialog {
    /// Creates a prompt dialog for jumping to a hex address.
    #[must_use]
    pub fn new_jump_to_address() -> Self {
        Self {
            mode: PromptMode::JumpToAddress,
            input: String::new(),
        }
    }

    /// Creates a prompt dialog for jumping to a decimal visual line number.
    #[must_use]
    pub fn new_jump_to_line() -> Self {
        Self {
            mode: PromptMode::JumpToLine,
            input: String::new(),
        }
    }

    /// Creates a prompt dialog for changing the document origin address.
    #[must_use]
    pub fn new_origin(current_origin: Addr) -> Self {
        Self {
            mode: PromptMode::Origin,
            input: format!("{:04X}", current_origin.0),
        }
    }

    /// Creates a prompt dialog for toggling a VICE execution breakpoint at an address.
    #[must_use]
    pub fn new_breakpoint_address(prefill: Option<u16>) -> Self {
        Self {
            mode: PromptMode::BreakpointAddress,
            input: prefill.map(|a| format!("{a:04X}")).unwrap_or_default(),
        }
    }

    /// Creates a prompt dialog for setting the VICE debugger memory dump address.
    #[must_use]
    pub fn new_memory_dump_address(prefill: Option<u16>) -> Self {
        Self {
            mode: PromptMode::MemoryDumpAddress,
            input: prefill.map(|a| format!("{a:04X}")).unwrap_or_default(),
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

    fn submit(&self, app_state: &mut AppState, ui_state: &mut UIState) -> WidgetResult {
        match self.mode {
            PromptMode::JumpToAddress => {
                if let Ok(target_addr) = u16::from_str_radix(&self.input, 16) {
                    WidgetResult::Action(AppAction::NavigateToAddress(Addr(target_addr)))
                } else {
                    if !self.input.is_empty() {
                        ui_state.set_status_message("Invalid Hex Address");
                    }
                    WidgetResult::Close
                }
            }
            PromptMode::JumpToLine => {
                if let Ok(line_num) = self.input.parse::<usize>() {
                    if let Some(target_idx) =
                        crate::ui::view_disassembly::DisassemblyView::get_index_for_visual_line(
                            app_state, line_num,
                        )
                    {
                        let target_addr = app_state.disassembly[target_idx]
                            .external_label_address
                            .unwrap_or(app_state.disassembly[target_idx].address);
                        crate::ui::navigable::jump_to_disassembly_at_line_idx(
                            ui_state,
                            Some(target_idx),
                            target_addr,
                        );
                        ui_state.set_status_message(format!("Jumped to line {line_num}"));
                    } else {
                        ui_state.set_status_message(format!("Line {line_num} out of range"));
                    }
                } else if !self.input.is_empty() {
                    ui_state.set_status_message("Invalid Line Number");
                }
                WidgetResult::Close
            }
            PromptMode::Origin => {
                if let Ok(new_origin) = u16::from_str_radix(&self.input, 16) {
                    let size = app_state.raw_data.len();
                    if (new_origin as usize) + size <= 0x10000 {
                        WidgetResult::Action(AppAction::ApplyOrigin(Addr(new_origin)))
                    } else {
                        ui_state.set_status_message("Error: Origin + Size exceeds $FFFF");
                        WidgetResult::Handled
                    }
                } else {
                    ui_state.set_status_message("Invalid Hex Address");
                    WidgetResult::Handled
                }
            }
            PromptMode::BreakpointAddress => {
                if let Ok(addr) = u16::from_str_radix(&self.input, 16) {
                    WidgetResult::Action(AppAction::ViceSetBreakpointAt {
                        address: Addr(addr),
                    })
                } else if self.input.is_empty() {
                    WidgetResult::Close
                } else {
                    ui_state.set_status_message("Invalid hex address");
                    WidgetResult::Handled
                }
            }
            PromptMode::MemoryDumpAddress => {
                if let Ok(addr) = u16::from_str_radix(&self.input, 16) {
                    WidgetResult::Action(AppAction::ViceSetMemoryDumpAddress {
                        address: Addr(addr),
                    })
                } else if self.input.is_empty() {
                    WidgetResult::Close
                } else {
                    ui_state.set_status_message("Invalid hex address");
                    WidgetResult::Handled
                }
            }
        }
    }
}

impl Widget for PromptDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(self.mode.dialog_title(), theme);

        let percent_x = if self.mode == PromptMode::Origin {
            40
        } else {
            30
        };
        let area =
            crate::utils::centered_rect_adaptive(percent_x, 40, 0, self.mode.min_height(), area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let dim = Style::default().fg(theme.comment);
        let value_style = Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD);

        let input_line = if self.mode.has_dollar_prefix() {
            Line::from(vec![
                Span::styled("$", dim),
                Span::styled(self.input.clone(), value_style),
            ])
        } else {
            Line::from(Span::styled(self.input.clone(), value_style))
        };

        let mut lines = vec![input_line];
        if self.mode == PromptMode::BreakpointAddress {
            // Color::Red is intentionally kept for the breakpoint indicator (semantic alert color).
            let warn = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
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
            lines.push(status_line);
        }

        let para = Paragraph::new(lines).block(block);
        f.render_widget(para, area);

        if area.width > 2 && area.height > 1 {
            let prefix_len = u16::from(self.mode.has_dollar_prefix());
            let cursor_x = (area.x + 1 + prefix_len + self.input.chars().count() as u16)
                .min(area.right().saturating_sub(2));
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
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => self.submit(app_state, ui_state),
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) => {
                if self.input.len() < self.mode.max_input_len() {
                    if self.mode.is_hex() && c.is_ascii_hexdigit() {
                        self.input.push(c.to_ascii_uppercase());
                    } else if !self.mode.is_hex() && c.is_ascii_digit() {
                        self.input.push(c);
                    }
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
    use crossterm::event::KeyModifiers;

    #[test]
    fn test_jump_to_line_updates_cursor_and_history() {
        let mut app_state = AppState::new();
        app_state.origin = crate::state::Addr(0x1000);
        app_state.raw_data = vec![0xEA; 10];
        app_state.block_types = vec![crate::state::BlockType::Code; 10];
        app_state.disassemble();

        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 0;

        let mut dialog = PromptDialog::new_jump_to_line();
        dialog.input = "3".to_string();

        let res = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(res, WidgetResult::Close);
        assert_eq!(ui_state.cursor_index, 2);
        assert_eq!(ui_state.scroll_index, 2);
        assert_eq!(ui_state.navigation_history.len(), 1);
        assert_eq!(ui_state.status_message.as_deref(), Some("Jumped to line 3"));
    }

    #[test]
    fn test_hex_prompt_modes_filter_and_submit() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA; 16];
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        let mut jump_dialog = PromptDialog::new_jump_to_address();
        for ch in ['c', '0', '0', '0', 'f'] {
            jump_dialog.handle_input(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            );
        }
        assert_eq!(jump_dialog.input, "C000");
        assert_eq!(
            jump_dialog.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Action(AppAction::NavigateToAddress(Addr(0xC000)))
        );

        let mut origin_dialog = PromptDialog::new_origin(Addr(0x0801));
        assert_eq!(origin_dialog.input, "0801");
        origin_dialog.input = "FFFF".to_string();
        assert_eq!(
            origin_dialog.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );
        assert_eq!(
            ui_state.status_message.as_deref(),
            Some("Error: Origin + Size exceeds $FFFF")
        );
    }
}
