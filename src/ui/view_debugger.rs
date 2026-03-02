use crate::state::AppState;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
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
            if vs.running {
                (
                    "▶ RUNNING",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    "⏸ PAUSED",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            }
        } else {
            ("○ Offline", Style::default().fg(Color::DarkGray))
        };

        // ---- Live Disassembly Panel (if connected and we have memory) ----
        const LIVE_PANEL_HEIGHT: u16 = 12; // rows reserved for live panel
        let (status_area, live_area, debugger_area) = if vs.live_memory.is_some() && vs.connected {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Status line
                    Constraint::Length(LIVE_PANEL_HEIGHT),
                    Constraint::Min(1),
                ])
                .split(inner);
            (Some(chunks[0]), Some(chunks[1]), chunks[2])
        } else {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Min(1)]) // text + empty line space
                .split(inner);
            (Some(chunks[0]), None, chunks[1])
        };

        if let Some(s_area) = status_area {
            let status_para = Paragraph::new(Line::from(Span::styled(status_text, status_style)));
            f.render_widget(status_para, s_area);
        }

        // Render live disassembly panel if we have live memory
        if let (Some(live_rect), Some(live_bytes)) = (live_area, &vs.live_memory) {
            let mem_start = vs.live_memory_start;
            let pc = vs.pc.unwrap_or(mem_start);

            use crate::state::{BlockType, DocumentSettings};
            use std::collections::{BTreeMap, BTreeSet};

            let block_types: Vec<BlockType> = vec![BlockType::Code; live_bytes.len()];
            let empty_labels: BTreeMap<u16, Vec<crate::state::Label>> = BTreeMap::new();
            let empty_comments: BTreeMap<u16, String> = BTreeMap::new();
            let empty_line_comments: BTreeMap<u16, String> = BTreeMap::new();
            let empty_formats: BTreeMap<u16, crate::state::ImmediateFormat> = BTreeMap::new();
            let empty_xrefs: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
            let empty_splitters: BTreeSet<u16> = BTreeSet::new();
            let settings = DocumentSettings::default();
            let collapsed: Vec<(usize, usize)> = Vec::new();

            let live_lines = app_state.disassembler.disassemble(
                live_bytes,
                &block_types,
                &empty_labels,
                mem_start,
                &settings,
                &empty_comments,
                &empty_comments,
                &empty_line_comments,
                &empty_formats,
                &empty_xrefs,
                &collapsed,
                &empty_splitters,
            );

            let pc_live_idx = live_lines.iter().position(|l| l.address == pc).unwrap_or(0);
            let panel_rows = (LIVE_PANEL_HEIGHT as usize).saturating_sub(1);
            // Since status line takes 1 row above, and we have a header,
            // we need exactly 3 rows before the PC to render it at inner.y + 5.
            let rows_before_pc = 3;
            let start_idx = pc_live_idx.saturating_sub(rows_before_pc);
            let end_idx = (start_idx + panel_rows).min(live_lines.len());

            let live_style = Style::default().bg(theme.background).fg(theme.foreground);
            let pc_style = Style::default()
                .bg(theme.border_active)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD);
            let dim_style = Style::default().fg(theme.comment);
            let header_style = Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD);

            let mut live_rendered: Vec<Line> = Vec::new();
            live_rendered.push(Line::from(Span::styled(
                format!(" Live @ ${:04X} ─────────────────", pc),
                header_style,
            )));

            for line in live_lines[start_idx..end_idx].iter() {
                let is_pc_line = line.address == pc;
                let gutter = if is_pc_line { ">" } else { " " };
                let row_text = format!(
                    "{} ${:04X}  {:<10} {} {}",
                    gutter,
                    line.address,
                    if line.show_bytes {
                        line.bytes
                            .iter()
                            .map(|b| format!("{:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else {
                        String::new()
                    },
                    line.mnemonic,
                    line.operand,
                );
                let style = if is_pc_line { pc_style } else { live_style };
                live_rendered.push(Line::from(Span::styled(row_text, style)));
            }

            while live_rendered.len() < LIVE_PANEL_HEIGHT as usize {
                live_rendered.push(Line::from(Span::styled("", dim_style)));
            }

            let live_para =
                Paragraph::new(live_rendered).style(Style::default().bg(theme.background));
            f.render_widget(live_para, live_rect);
        }

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

        let exec_bps: Vec<_> = vs
            .breakpoints
            .iter()
            .filter(|bp| bp.kind == crate::vice::state::BreakpointKind::Exec)
            .collect();
        let watchpoints: Vec<_> = vs
            .breakpoints
            .iter()
            .filter(|bp| bp.kind != crate::vice::state::BreakpointKind::Exec)
            .collect();

        if exec_bps.is_empty() {
            lines.push(Line::from(Span::styled("  (none)", dim_style)));
        } else {
            for bp in &exec_bps {
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

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Watchpoints", heading_style)));

        if watchpoints.is_empty() {
            lines.push(Line::from(Span::styled("  (none)", dim_style)));
        } else {
            for bp in &watchpoints {
                let bp_style = if bp.enabled {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    dim_style
                };
                let flag = if bp.enabled { "●" } else { "○" };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", flag), bp_style),
                    Span::styled(format!("#{:<3}", bp.id), dim_style),
                    Span::styled(format!("${:04X}", bp.address), value_style),
                    Span::styled(format!(" [{}]", bp.kind.label()), bp_style),
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
                Span::styled("  F6  ", label_style),
                Span::styled("Watchpoint...", dim_style),
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

        let is_commodore = app_state.settings.platform == "Commodore 64"
            || app_state.settings.platform == "Commodore 128";

        let (stack_rect, hw_rect) = if is_commodore && debugger_area.width > 50 {
            let ch = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(debugger_area);
            (ch[0], Some(ch[1]))
        } else {
            (debugger_area, None)
        };

        let para = Paragraph::new(lines);
        f.render_widget(para, stack_rect);

        if let Some(hr) = hw_rect {
            let mut right_lines: Vec<Line> = Vec::new();
            if let Some(io_mem) = &vs.io_memory {
                let vic = crate::vice::Vic2State::decode(io_mem);
                let cia1 = crate::vice::CiaState::decode(io_mem, 0xDC00 - 0xD000);
                let cia2 = crate::vice::CiaState::decode(io_mem, 0xDD00 - 0xD000);

                right_lines.push(Line::from(Span::styled("VIC-II Registers", heading_style)));
                right_lines.push(Line::from(vec![
                    Span::styled("  Mode  ", label_style),
                    Span::styled(if vic.bitmap_mode { "Bitmap" } else { "Text" }, value_style),
                    Span::styled(
                        if vic.multicolor_mode { " MC" } else { " Hires" },
                        value_style,
                    ),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  Color ", label_style),
                    Span::styled(
                        if vic.extended_bg_color { "ExtBG " } else { "" },
                        value_style,
                    ),
                    Span::styled(
                        format!("Border: {} BG: {}", vic.border_color, vic.bg_color),
                        dim_style,
                    ),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  Scrl  ", label_style),
                    Span::styled(
                        format!("X: {} Y: {}", vic.x_scroll, vic.y_scroll),
                        value_style,
                    ),
                    Span::styled(format!(" {}x{}", vic.columns, vic.rows), dim_style),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  Rast  ", label_style),
                    Span::styled(format!("{}", vic.raster_line), value_style),
                    Span::styled(if vic.blanking { " (Blank)" } else { "" }, dim_style),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  Mem   ", label_style),
                    Span::styled(
                        format!(
                            "Scrn: ${:04X} Char: ${:04X}",
                            vic.screen_mem_address, vic.charset_address
                        ),
                        dim_style,
                    ),
                ]));

                right_lines.push(Line::from(""));
                right_lines.push(Line::from(Span::styled("CIA 1 ($DC00)", heading_style)));
                right_lines.push(Line::from(vec![
                    Span::styled("  PRA   ", label_style),
                    Span::styled(format!("${:02X} (Joy2/P1)", cia1.pra), value_style),
                ]));
                right_lines.push(Line::from(vec![
                    Span::styled("  PRB   ", label_style),
                    Span::styled(format!("${:02X} (Joy1/P2)", cia1.prb), value_style),
                ]));

                right_lines.push(Line::from(""));
                right_lines.push(Line::from(Span::styled("CIA 2 ($DD00)", heading_style)));
                right_lines.push(Line::from(vec![
                    Span::styled("  PRA   ", label_style),
                    Span::styled(format!("${:02X} (VIC Bank)", cia2.pra), value_style),
                ]));
            } else {
                right_lines.push(Line::from(Span::styled(
                    "Hardware state unavailable",
                    dim_style,
                )));
            }
            let r_para = Paragraph::new(right_lines);
            f.render_widget(r_para, hr);
        }
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
