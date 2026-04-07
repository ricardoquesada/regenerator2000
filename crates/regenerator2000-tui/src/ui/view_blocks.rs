use crate::state::{AppState, BlockType};
use crate::ui_state::{ActivePane, AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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

    fn item_name(&self) -> &'static str {
        "block"
    }
}

impl Widget for BlocksView {
    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let area = ui_state.right_pane_area;
        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        if mouse.column < inner_area.x
            || mouse.column >= inner_area.x + inner_area.width
            || mouse.row < inner_area.y
            || mouse.row >= inner_area.y + inner_area.height
        {
            return WidgetResult::Ignored;
        }

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.move_down(app_state, ui_state, 3);
                WidgetResult::Handled
            }
            MouseEventKind::ScrollUp => {
                self.move_up(app_state, ui_state, 3);
                WidgetResult::Handled
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let index = ui_state.blocks_list_state.offset()
                    + (mouse.row.saturating_sub(inner_area.y) as usize);
                let blocks = app_state.get_blocks_view_items();
                if index < blocks.len() {
                    ui_state.blocks_list_state.select(Some(index));

                    let (target_addr, line_idx) = match &blocks[index] {
                        crate::state::BlockItem::Block { start, .. } => {
                            let idx = app_state.disassembly.iter().position(|line| {
                                line.address == *start
                                    && (!line.bytes.is_empty() || line.is_collapsed)
                            });
                            (*start, idx)
                        }
                        crate::state::BlockItem::Splitter(addr) => {
                            let idx = app_state.disassembly.iter().position(|line| {
                                line.address == *addr && line.mnemonic == "{splitter}"
                            });
                            (*addr, idx)
                        }
                        crate::state::BlockItem::Scope { start, .. } => {
                            (*start, app_state.get_line_index_containing_address(*start))
                        }
                    };

                    crate::ui::navigable::jump_to_disassembly_at_line_idx(
                        ui_state,
                        line_idx,
                        target_addr,
                    );
                    WidgetResult::Handled
                } else {
                    WidgetResult::Ignored
                }
            }
            _ => WidgetResult::Ignored,
        }
    }

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
        let mut current_scope_end: Option<crate::state::types::Addr> = None;
        let items: Vec<ListItem> = block_items
            .iter()
            .map(|item| match item {
                crate::state::BlockItem::Scope { start, end, name } => {
                    current_scope_end = Some(*end);
                    let name_str = name.as_deref().unwrap_or("Unnamed");
                    let text = format!(" Scope: ${start:04X} - ${end:04X} [{name_str}]");
                    ListItem::new(Line::from(Span::styled(
                        text,
                        Style::default()
                            .fg(ui_state.theme.label_def)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    )))
                }
                crate::state::BlockItem::Block {
                    start,
                    end,
                    type_,
                    collapsed,
                } => {
                    if let Some(scope_end) = current_scope_end
                        && *start > scope_end
                    {
                        current_scope_end = None;
                    }
                    let indent = if current_scope_end.is_some() {
                        "  "
                    } else {
                        ""
                    };
                    let start_addr = *start;
                    let end_addr = *end;
                    let color = match type_ {
                        BlockType::Code => ui_state.theme.block_code_fg,
                        BlockType::DataByte => ui_state.theme.block_data_byte_fg,
                        BlockType::DataWord => ui_state.theme.block_data_word_fg,
                        BlockType::Address => ui_state.theme.block_address_fg,
                        BlockType::PetsciiText => ui_state.theme.block_petscii_text_fg,
                        BlockType::ScreencodeText => ui_state.theme.block_screencode_text_fg,
                        BlockType::LoHiAddress => ui_state.theme.block_lohi_fg,
                        BlockType::HiLoAddress => ui_state.theme.block_hilo_fg,
                        BlockType::LoHiWord => ui_state.theme.block_lohi_fg,
                        BlockType::HiLoWord => ui_state.theme.block_hilo_fg,
                        BlockType::ExternalFile => ui_state.theme.block_external_file_fg,
                        BlockType::Undefined => ui_state.theme.block_undefined_fg,
                    };

                    let collapse_char = if *collapsed { "+" } else { " " };
                    let text = format!(
                        "{indent}{collapse_char} ${start_addr:04X} - ${end_addr:04X} [{type_}]"
                    );
                    ListItem::new(Line::from(Span::styled(text, Style::default().fg(color))))
                }
                crate::state::BlockItem::Splitter(addr) => {
                    if let Some(scope_end) = current_scope_end
                        && *addr > scope_end
                    {
                        current_scope_end = None;
                    }
                    let indent = if current_scope_end.is_some() {
                        "  "
                    } else {
                        ""
                    };
                    let text = format!("{indent}  ${addr:04X} -----------------");
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
                    let (target_addr, line_idx) = match &blocks[idx] {
                        crate::state::BlockItem::Block { start, .. } => {
                            let idx = app_state.disassembly.iter().position(|line| {
                                line.address == *start
                                    && (!line.bytes.is_empty() || line.is_collapsed)
                            });
                            (*start, idx)
                        }
                        crate::state::BlockItem::Splitter(addr) => {
                            let idx = app_state.disassembly.iter().position(|line| {
                                line.address == *addr && line.mnemonic == "{splitter}"
                            });
                            (*addr, idx)
                        }
                        crate::state::BlockItem::Scope { start, .. } => {
                            (*start, app_state.get_line_index_containing_address(*start))
                        }
                    };

                    crate::ui::navigable::jump_to_disassembly_at_line_idx(
                        ui_state,
                        line_idx,
                        target_addr,
                    );
                }
                WidgetResult::Handled
            }
            KeyCode::Char('k') if key.modifiers == KeyModifiers::CONTROL => {
                WidgetResult::Action(AppAction::ToggleCollapsedBlock)
            }

            _ => WidgetResult::Ignored,
        }
    }
}
