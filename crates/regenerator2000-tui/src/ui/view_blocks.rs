//! Interactive Blocks View side-pane widget.
//!
//! This module implements [`BlocksView`], which renders the list of memory blocks, manual
//! splitters, and lexical scopes produced by [`AppState::get_blocks_view_items`], and handles
//! keyboard and mouse navigation to jump the disassembly view to selected blocks. Rendering
//! is windowed to the visible viewport (`[render_start, render_end)`) with lightweight 1-line
//! placeholder items outside the viewport to preserve [`ratatui::widgets::ListState`] scroll offsets.

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

/// Side-pane widget displaying memory blocks, manual splitters, and lexical scopes,
/// implementing [`Widget`] and [`Navigable`].
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

        // Fetch the ordered list of blocks, manual splitters, and lexical scopes from core state.
        let block_items = app_state.get_blocks_view_items();
        let total_items = block_items.len();
        let visible_height = inner_area.height as usize;

        // Compute the effective scroll offset that `ratatui::widgets::List` will use
        // after clamping `selected` into `[offset, offset + visible_height)`, so we only
        // allocate formatted strings/spans for the visible viewport window while keeping
        // 1-line placeholder items outside the viewport to preserve `ListState::offset()`.
        let mut effective_offset = ui_state
            .blocks_list_state
            .offset()
            .min(total_items.saturating_sub(1));
        if let Some(selected) = ui_state
            .blocks_list_state
            .selected()
            .map(|s| s.min(total_items.saturating_sub(1)))
        {
            if selected < effective_offset {
                effective_offset = selected;
            } else if visible_height > 0
                && selected >= effective_offset.saturating_add(visible_height)
            {
                effective_offset = selected.saturating_add(1).saturating_sub(visible_height);
            }
        }
        let render_start = effective_offset.saturating_sub(1);
        let render_end = effective_offset
            .saturating_add(visible_height)
            .saturating_add(1)
            .min(total_items);

        // Track `current_scope_end` across all items (including off-screen items before the
        // `if !is_visible` early return) so that when a `BlockItem::Scope` header is scrolled
        // above `render_start`, visible `Block` and `Splitter` rows inside that scope still
        // receive their 2-space (`"  "`) scope indentation.
        let mut current_scope_end: Option<crate::state::types::Addr> = None;
        let items: Vec<ListItem> = block_items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let is_visible = idx >= render_start && idx < render_end;
                match item {
                    crate::state::BlockItem::Scope { start, end, name } => {
                        current_scope_end = Some(*end);
                        if !is_visible {
                            return ListItem::new(Line::default());
                        }
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
                        if !is_visible {
                            return ListItem::new(Line::default());
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
                        if !is_visible {
                            return ListItem::new(Line::default());
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
                }
            })
            .collect();

        // Render as a stateful widget so `ui_state.blocks_list_state` updates its scroll offset in place.
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

        match key.code {
            // Enter (unmodified) jumps to the start address of the selected block in Disassembly.
            KeyCode::Enter if key.modifiers.is_empty() => {
                let blocks = app_state.get_blocks_view_items();
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
            // Ctrl+k collapses/expands the block highlighted at `blocks_list_state.selected()`
            // (synced to `core.view.blocks_selected_index` by `handle_menu_action`).
            KeyCode::Char('k') if key.modifiers == KeyModifiers::CONTROL => {
                WidgetResult::Action(AppAction::ToggleCollapsedBlock)
            }

            _ => WidgetResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use regenerator2000_core::state::Addr;

    // T5.1: Verify scope indentation and mouse click index math when scrolled past Scope header (Landmine F)
    #[test]
    fn test_blocks_view_scope_indentation_when_scrolled_landmine_f() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(Addr(0x1000), vec![0xEA; 0x200])
            .unwrap();
        app_state.block_types = vec![BlockType::Code; 0x200];

        // Define a scope $1000..=$10FF and 30 split sub-blocks inside it
        app_state.annotations.update(Addr(0x1000), |e| {
            e.scope = Some(Addr(0x10FF));
        });
        for i in 1..30u16 {
            app_state.splitters.insert(Addr(0x1000 + i * 4));
        }
        // Also add a splitter outside the scope at $1120
        app_state.splitters.insert(Addr(0x1120));
        app_state.disassemble();

        let items = app_state.get_blocks_view_items();
        assert!(items.len() > 60);

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Blocks;
        // Select an item deep inside the scope so that the Scope header at index 0 is scrolled off-screen
        ui_state.blocks_list_state.select(Some(20));

        let area = Rect::new(0, 0, 50, 10);
        ui_state.right_pane_area = area;
        let backend = TestBackend::new(50, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut view = BlocksView;
        terminal
            .draw(|f| {
                view.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        // Scope header at index 0 must be scrolled off the top of the 8-row inner viewport
        assert!(
            ui_state.blocks_list_state.offset() > 0,
            "Expected list offset > 0 when index 20 is selected in height-10 viewport"
        );

        let buffer = terminal.backend().buffer();
        for y in 1..9 {
            let line_str: String = (1..49)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map_or(String::new(), |c| c.symbol().to_string())
                })
                .collect();
            // Every visible row inside $1000..=$10FF has highlight prefix ("> " or "  ")
            // followed by the 2-space scope indentation ("  ") before collapse char / splitter
            assert!(
                line_str.contains("   $") || line_str.contains(">    $"),
                "Expected scoped indentation in visible scrolled row {y}, got: '{line_str}'"
            );
        }

        // Verify mouse click math selects the exact clicked block in the scrolled list
        let offset_before_click = ui_state.blocks_list_state.offset();
        let click_row = 3u16; // inner_area.y is 1, so row 3 is offset + 2
        let expected_index = offset_before_click + (click_row - 1) as usize;
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: click_row,
            modifiers: KeyModifiers::empty(),
        };
        let res = view.handle_mouse(mouse_event, &mut app_state, &mut ui_state);
        assert_eq!(res, WidgetResult::Handled);
        assert_eq!(ui_state.blocks_list_state.selected(), Some(expected_index));

        // Now scroll to the very end (outside the scope, $1100+) and verify no scope indentation
        let last_idx = items.len() - 1;
        ui_state.blocks_list_state.select(Some(last_idx));
        terminal
            .draw(|f| {
                view.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer_end = terminal.backend().buffer();
        let bottom_row: String = (1..49)
            .map(|x| {
                buffer_end
                    .cell((x, 8))
                    .map_or(String::new(), |c| c.symbol().to_string())
            })
            .collect();
        assert!(
            bottom_row.starts_with(">   $1120"),
            "Expected unindented block after scope end ($1100+), got: '{bottom_row}'"
        );
    }

    // T5.2: Verify BlocksView::handle_input for Enter, Ctrl+k, and unhandled keys
    #[test]
    fn test_blocks_view_handle_input_unhandled_and_handled_keys() {
        let mut app_state = AppState::new();
        app_state.load_binary(Addr(0x1000), vec![0xEA; 16]).unwrap();
        app_state.block_types = vec![BlockType::Code; 16];
        app_state.annotations.update(Addr(0x1000), |e| {
            e.scope = Some(Addr(0x1007));
        });
        app_state.splitters.insert(Addr(0x1008));
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Blocks;
        let mut view = BlocksView;

        // 1. Unhandled keys return WidgetResult::Ignored without mutating state
        ui_state.blocks_list_state.select(Some(1));
        for key in [
            KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()),
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()),
            KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT),
        ] {
            let result = view.handle_input(key, &mut app_state, &mut ui_state);
            assert_eq!(result, WidgetResult::Ignored);
            assert_eq!(ui_state.blocks_list_state.selected(), Some(1));
        }

        // 2. Ctrl+k returns WidgetResult::Action(AppAction::ToggleCollapsedBlock)
        let ctrl_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(
            view.handle_input(ctrl_k, &mut app_state, &mut ui_state),
            WidgetResult::Action(AppAction::ToggleCollapsedBlock)
        );

        // 3. Enter jumps to selected Scope (idx 0), Block (idx 1), and Splitter (idx 2)
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        for idx in 0..3 {
            ui_state.blocks_list_state.select(Some(idx));
            ui_state.active_pane = ActivePane::Blocks;
            let res = view.handle_input(enter, &mut app_state, &mut ui_state);
            assert_eq!(res, WidgetResult::Handled);
            assert_eq!(ui_state.active_pane, ActivePane::Disassembly);
        }
    }
}
