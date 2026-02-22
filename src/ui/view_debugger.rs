use crate::state::AppState;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::KeyEvent;
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

        let mut lines: Vec<Line> = vec![
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
            Line::from(Span::styled("Breakpoints", heading_style)),
        ];

        if vs.breakpoints.is_empty() {
            lines.push(Line::from(Span::styled("  (none)", dim_style)));
        } else {
            for bp in &vs.breakpoints {
                let bp_style = if bp.enabled {
                    Style::default()
                        .fg(ui_state.theme.error_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    dim_style
                };
                let flag = if bp.enabled { "●" } else { "○" };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", flag), bp_style),
                    Span::styled(format!("#{:<3}", bp.id), dim_style),
                    Span::styled(format!("${:04X}", bp.address), value_style),
                ]));
            }
        }

        // Stack view
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Stack", heading_style)));

        if let (Some(sp), Some(stack_mem)) = (vs.sp, &vs.stack_memory) {
            let sp_addr = 0x0100u16 + sp as u16;
            lines.push(Line::from(vec![
                Span::styled("  SP  ", label_style),
                Span::styled(format!("${:02X}", sp), value_style),
                Span::styled(format!("  →${:04X}", sp_addr), dim_style),
            ]));

            // Show up to 5 entries above SP (the used stack), from top (SP+1) downward
            let top = 0x01FFu16;
            let stack_top_addr = sp_addr.saturating_add(1);
            if stack_top_addr > top {
                lines.push(Line::from(Span::styled("  (empty)", dim_style)));
            } else {
                let max_entries = 5usize;
                let entries_available = (top - stack_top_addr + 1) as usize;
                let count = max_entries.min(entries_available);
                for i in 0..count {
                    let entry_addr = stack_top_addr + i as u16;
                    let byte_idx = (entry_addr - 0x0100) as usize;
                    let byte = stack_mem.get(byte_idx).copied();
                    let is_top = i == 0;
                    let addr_span = Span::styled(
                        format!("  ${:04X}  ", entry_addr),
                        if is_top { value_style } else { dim_style },
                    );
                    let val_span = match byte {
                        Some(b) => Span::styled(
                            format!("${:02X}", b),
                            if is_top { value_style } else { dim_style },
                        ),
                        None => Span::styled("??", dim_style),
                    };
                    if is_top {
                        lines.push(Line::from(vec![
                            addr_span,
                            val_span,
                            Span::styled("  ← top", dim_style),
                        ]));
                    } else {
                        lines.push(Line::from(vec![addr_span, val_span]));
                    }
                }
                if entries_available > max_entries {
                    lines.push(Line::from(Span::styled(
                        format!("  … {} more", entries_available - max_entries),
                        dim_style,
                    )));
                }
            }
        } else {
            lines.push(Line::from(Span::styled("  (no data)", dim_style)));
        }

        lines.extend([
            Line::from(""),
            Line::from(Span::styled("Controls", heading_style)),
            Line::from(vec![
                Span::styled("  F9  ", label_style),
                Span::styled("Continue", dim_style),
            ]),
            Line::from(vec![
                Span::styled("  F4  ", label_style),
                Span::styled("Run to cursor", dim_style),
            ]),
            Line::from(vec![
                Span::styled("  F2  ", label_style),
                Span::styled("Toggle breakpoint", dim_style),
            ]),
            Line::from(vec![
                Span::styled("  F7  ", label_style),
                Span::styled("Step into", dim_style),
            ]),
            Line::from(vec![
                Span::styled("  F8  ", label_style),
                Span::styled("Step over", dim_style),
            ]),
            Line::from(vec![
                Span::styled("  S-F8", label_style),
                Span::styled("Step out", dim_style),
            ]),
        ]);

        let para = Paragraph::new(lines);
        f.render_widget(para, inner);
    }

    fn handle_input(
        &mut self,
        _key: KeyEvent,
        _app_state: &mut AppState,
        _ui_state: &mut UIState,
    ) -> WidgetResult {
        WidgetResult::Ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::ui_state::UIState;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_debugger_view_handle_input_ignored() {
        let mut view = DebuggerView;
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        let key1 = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(
            view.handle_input(key1, &mut app_state, &mut ui_state),
            WidgetResult::Ignored
        );

        let key2 = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            view.handle_input(key2, &mut app_state, &mut ui_state),
            WidgetResult::Ignored
        );
    }
}
