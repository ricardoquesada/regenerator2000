use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, List, ListItem},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct BlocksView;

impl Widget for BlocksView {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let is_active = ui_state.active_pane == ActivePane::Blocks;
        let border_style = if is_active {
            Style::default().fg(ui_state.theme.border_active)
        } else {
            Style::default().fg(ui_state.theme.border_inactive)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Blocks ")
            .style(
                Style::default()
                    .bg(ui_state.theme.background)
                    .fg(ui_state.theme.foreground),
            );
        let inner_area = block.inner(area);
        f.render_widget(block, area);

        // Filter logic if needed? For now just list provided by AppState
        let block_items = app_state.get_blocks_view_items();
        let items: Vec<ListItem> = block_items
            .iter()
            .map(|item| {
                let text = match item {
                    crate::state::BlockItem::Block { start, end, type_ } => {
                        let start_addr = app_state.origin.wrapping_add(*start);
                        let end_addr = app_state.origin.wrapping_add(*end);
                        format!("${:04X} - ${:04X} [{}]", start_addr, end_addr, type_)
                    }
                    crate::state::BlockItem::Splitter(addr) => {
                        format!("${:04X} -----------------", addr)
                    }
                };
                ListItem::new(text)
            })
            .collect();

        // The list handling is a bit specific because we use `ratatui::widgets::List`.
        // It requires the state to be passed during render.
        // However, `render_widget` takes `state` as `&mut ListState`.
        // We have `ui_state.blocks_list_state` which is `ListState`.

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(ui_state.theme.highlight_fg)
                    .bg(ui_state.theme.highlight_bg),
            )
            .highlight_symbol("> ");

        f.render_stateful_widget(list, inner_area, &mut ui_state.blocks_list_state);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let blocks = app_state.get_blocks_view_items();

        match key.code {
            KeyCode::Down | KeyCode::Char('j')
                if key.modifiers.is_empty() || key.code == KeyCode::Down =>
            {
                ui_state.input_buffer.clear();
                let current = ui_state.blocks_list_state.selected().unwrap_or(0);
                let next = (current + 1).min(blocks.len().saturating_sub(1));
                ui_state.blocks_list_state.select(Some(next));
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k')
                if key.modifiers.is_empty() || key.code == KeyCode::Up =>
            {
                ui_state.input_buffer.clear();
                let current = ui_state.blocks_list_state.selected().unwrap_or(0);
                let next = current.saturating_sub(1);
                ui_state.blocks_list_state.select(Some(next));
                WidgetResult::Handled
            }
            KeyCode::PageDown | KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                ui_state.input_buffer.clear();
                let current = ui_state.blocks_list_state.selected().unwrap_or(0);
                let next = (current + 10).min(blocks.len().saturating_sub(1));
                ui_state.blocks_list_state.select(Some(next));
                WidgetResult::Handled
            }
            KeyCode::PageUp | KeyCode::Char('u')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                ui_state.input_buffer.clear();
                let current = ui_state.blocks_list_state.selected().unwrap_or(0);
                let next = current.saturating_sub(10);
                ui_state.blocks_list_state.select(Some(next));
                WidgetResult::Handled
            }
            KeyCode::Home => {
                ui_state.input_buffer.clear();
                ui_state.blocks_list_state.select(Some(0));
                WidgetResult::Handled
            }
            KeyCode::End => {
                ui_state.input_buffer.clear();
                ui_state
                    .blocks_list_state
                    .select(Some(blocks.len().saturating_sub(1)));
                WidgetResult::Handled
            }
            KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
                let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
                let is_buffer_empty = ui_state.input_buffer.is_empty();
                ui_state.input_buffer.clear();

                let target = if is_buffer_empty {
                    blocks.len()
                } else {
                    entered_number
                };
                let new_selection = if target == 0 {
                    blocks.len().saturating_sub(1)
                } else {
                    target.saturating_sub(1).min(blocks.len().saturating_sub(1))
                };
                ui_state.blocks_list_state.select(Some(new_selection));
                ui_state.set_status_message(format!("Jumped to block {}", target));
                WidgetResult::Handled
            }
            // Enter to jump to address of block
            KeyCode::Enter if key.modifiers.is_empty() => {
                let idx = ui_state.blocks_list_state.selected().unwrap_or(0);
                if idx < blocks.len() {
                    let target_addr = match blocks[idx] {
                        crate::state::BlockItem::Block { start, .. } => {
                            // start is u16 (offset from origin? or absolute?)
                            // Block definition: start: u16, end: u16.
                            // `get_blocks_view_items` logic:
                            // `block_start = self.origin.wrapping_add(block.start as u16);`
                            // But wait, `Block` in `AppState` uses `usize` for start/end (offset).
                            // `BlockItem` (enum) uses `u16`?
                            // Let's check BlockItem def again in Step 425.
                            // `pub enum BlockItem { Block { start: u16, end: u16, type_: BlockType }, Splitter(u16) }`
                            // So `start` in `BlockItem` SHOULD be offset?
                            // Let's check `get_blocks_view_items` logic in Step 425? Steps 425 ended at 1060.
                            // Line 1046: `let block_start = ... wrapping_add ...`
                            // It implies `Block` struct has `start: usize`.
                            // `BlockItem` struct has `start: u16`.
                            // If `BlockItem` stores OFFSET, then `app_state.origin + start`.
                            // If `BlockItem` stores ABSOLUTE ADDR, then just `start`.
                            // Let's assume BlockItem stores OFFSET based on `start: u16` being typical for offsets in C64 context (lines 0-65535).
                            // Actually, 64k size fits in u16.
                            // Re-reading logic in Step 418 (events.rs):
                            // `crate::state::BlockItem::Block { start, .. } => { let offset = start; Some(app_state.origin.wrapping_add(offset)) }`
                            // So `start` is OFFSET.
                            Some(app_state.origin.wrapping_add(start))
                        }
                        crate::state::BlockItem::Splitter(addr) => Some(addr),
                    };

                    if let Some(addr) = target_addr {
                        if let Some(line_idx) = app_state.get_line_index_containing_address(addr) {
                            ui_state
                                .navigation_history
                                .push((ActivePane::Disassembly, ui_state.cursor_index));
                            ui_state.cursor_index = line_idx;
                            ui_state.active_pane = ActivePane::Disassembly;
                            ui_state.sub_cursor_index = 0;
                            ui_state.set_status_message(format!("Jumped to ${:04X}", addr));
                        } else {
                            ui_state.set_status_message(format!("Address ${:04X} not found", addr));
                        }
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Char(' ') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::ToggleCollapsedBlock)
            }

            _ => WidgetResult::Ignored,
        }
    }
}
