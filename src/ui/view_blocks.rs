use crate::state::{AppState, BlockType};
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::ui::widget::{Widget, WidgetResult};

use crate::ui::navigable::{Navigable, handle_nav_input};

pub struct BlocksView;

impl Navigable for BlocksView {
    fn len(&self, app_state: &AppState) -> usize {
        app_state.get_blocks_view_items().len()
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.blocks_list_state.selected().unwrap_or(0)
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        let len = self.len(app_state);
        if len == 0 {
            return;
        }
        let current = self.current_index(app_state, ui_state);
        let next = (current + amount).min(len.saturating_sub(1));
        ui_state.blocks_list_state.select(Some(next));
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        let current = self.current_index(_app_state, ui_state);
        let next = current.saturating_sub(amount);
        ui_state.blocks_list_state.select(Some(next));
    }

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_down(app_state, ui_state, 10);
    }

    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_up(app_state, ui_state, 10);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let len = self.len(app_state);
        ui_state
            .blocks_list_state
            .select(Some(index.min(len.saturating_sub(1))));
    }

    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        let len = self.len(app_state);
        let target = if input == 0 {
            len.saturating_sub(1)
        } else {
            input.saturating_sub(1).min(len.saturating_sub(1))
        };
        ui_state.blocks_list_state.select(Some(target));
    }

    fn item_name(&self) -> &str {
        "block"
    }
}

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
            .map(|item| match item {
                crate::state::BlockItem::Block {
                    start,
                    end,
                    type_,
                    collapsed,
                } => {
                    let start_addr = app_state.origin.wrapping_add(*start);
                    let end_addr = app_state.origin.wrapping_add(*end);
                    let color = match type_ {
                        BlockType::Code => ui_state.theme.block_code_fg,
                        BlockType::DataByte => ui_state.theme.block_data_byte_fg,
                        BlockType::DataWord => ui_state.theme.block_data_word_fg,
                        BlockType::Address => ui_state.theme.block_address_fg,
                        BlockType::Text => ui_state.theme.block_text_fg,
                        BlockType::Screencode => ui_state.theme.block_screencode_fg,
                        BlockType::LoHi => ui_state.theme.block_lohi_fg,
                        BlockType::HiLo => ui_state.theme.block_hilo_fg,
                        BlockType::ExternalFile => ui_state.theme.block_external_file_fg,
                        BlockType::Undefined => ui_state.theme.block_undefined_fg,
                    };

                    let collapse_char = if *collapsed { "+" } else { " " };
                    let text = format!(
                        "{} ${:04X} - ${:04X} [{}]",
                        collapse_char, start_addr, end_addr, type_
                    );
                    ListItem::new(Line::from(Span::styled(text, Style::default().fg(color))))
                }
                crate::state::BlockItem::Splitter(addr) => {
                    let text = format!("  ${:04X} -----------------", addr);
                    ListItem::new(Line::from(Span::styled(
                        text,
                        Style::default().fg(ui_state.theme.block_splitter_fg),
                    )))
                }
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
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            return WidgetResult::Handled;
        }

        let blocks = app_state.get_blocks_view_items();

        match key.code {
            // Enter to jump to address of block
            KeyCode::Enter if key.modifiers.is_empty() => {
                let idx = ui_state.blocks_list_state.selected().unwrap_or(0);
                if idx < blocks.len() {
                    let target_addr = match blocks[idx] {
                        crate::state::BlockItem::Block { start, .. } => {
                            // start is u16 (offset from origin)
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
            KeyCode::Char('k') if key.modifiers == KeyModifiers::CONTROL => {
                WidgetResult::Action(MenuAction::ToggleCollapsedBlock)
            }

            _ => WidgetResult::Ignored,
        }
    }
}
