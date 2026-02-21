use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct DebuggerView;

impl Widget for DebuggerView {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let is_active = ui_state.active_pane == ActivePane::Debugger;
        let theme = &ui_state.theme;

        let border_style = if is_active {
            Style::default().fg(theme.border_active)
        } else {
            Style::default().fg(theme.border_inactive)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Debugger ")
            .border_style(border_style)
            .style(Style::default().bg(theme.background).fg(theme.foreground));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let vs = &app_state.vice_state;

        // Connection status line
        let (status_text, status_style) = if vs.connected {
            (
                "● CONNECTED",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("○ Offline", Style::default().fg(Color::DarkGray))
        };

        let fmt_byte = |v: Option<u8>| -> String {
            match v {
                Some(b) => format!("${:02X}", b),
                None => "?".to_string(),
            }
        };
        let fmt_word = |v: Option<u16>| -> String {
            match v {
                Some(w) => format!("${:04X}", w),
                None => "?".to_string(),
            }
        };

        // P register bits: N V - B D I Z C (bit 7 → bit 0)
        let p_flags_str = match vs.p {
            Some(p) => {
                let bits = ['N', 'V', '-', 'B', 'D', 'I', 'Z', 'C'];
                bits.iter()
                    .enumerate()
                    .map(|(i, &c)| if (p >> (7 - i)) & 1 == 1 { c } else { '.' })
                    .collect::<String>()
            }
            None => "????????".to_string(),
        };
        let p_bits_str = match vs.p {
            Some(p) => format!("{:08b}", p),
            None => "????????".to_string(),
        };

        let label_style = Style::default().fg(theme.foreground);
        let value_style = Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD);
        let dim_style = Style::default().fg(theme.comment);
        let heading_style = Style::default()
            .fg(theme.label)
            .add_modifier(Modifier::BOLD);

        let lines: Vec<Line> = vec![
            Line::from(Span::styled(status_text, status_style)),
            Line::from(""),
            Line::from(Span::styled("CPU Registers", heading_style)),
            Line::from(vec![
                Span::styled("  PC  ", label_style),
                Span::styled(fmt_word(vs.pc), value_style),
            ]),
            Line::from(vec![
                Span::styled("  A   ", label_style),
                Span::styled(format!("{:<6}", fmt_byte(vs.a)), value_style),
                Span::styled("X   ", label_style),
                Span::styled(fmt_byte(vs.x), value_style),
            ]),
            Line::from(vec![
                Span::styled("  Y   ", label_style),
                Span::styled(format!("{:<6}", fmt_byte(vs.y)), value_style),
                Span::styled("SP  ", label_style),
                Span::styled(fmt_byte(vs.sp), value_style),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  P   ", label_style),
                Span::styled("NV-BDIZC", dim_style),
            ]),
            Line::from(vec![
                Span::styled("      ", label_style),
                Span::styled(p_flags_str, value_style),
            ]),
            Line::from(vec![
                Span::styled("      ", label_style),
                Span::styled(p_bits_str, dim_style),
            ]),
            Line::from(""),
            Line::from(Span::styled("Controls", heading_style)),
            Line::from(vec![
                Span::styled("  F5  ", label_style),
                Span::styled("Continue", dim_style),
            ]),
            Line::from(vec![
                Span::styled("  F8  ", label_style),
                Span::styled("Run to cursor", dim_style),
            ]),
            Line::from(vec![
                Span::styled("  F10 ", label_style),
                Span::styled("Step into", dim_style),
            ]),
            Line::from(vec![
                Span::styled("  F11 ", label_style),
                Span::styled("Step over", dim_style),
            ]),
        ];

        let para = Paragraph::new(lines);
        f.render_widget(para, inner);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        _ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::F(5) => WidgetResult::Action(MenuAction::ViceContinue),
            KeyCode::F(10) => WidgetResult::Action(MenuAction::ViceStep),
            KeyCode::F(11) => WidgetResult::Action(MenuAction::ViceStepOver),
            _ => WidgetResult::Ignored,
        }
    }
}
