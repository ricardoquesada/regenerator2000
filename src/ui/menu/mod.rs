use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Frame, layout::Rect};

// Sub-modules
mod menu_action;
mod menu_model;
mod menu_render;

// Re-exports: preserve the existing public API surface so external `use crate::ui::menu::*` paths
// continue to work without changes.
pub use menu_action::{
    AppAction, execute_menu_action, handle_menu_action, perform_jump_to_address,
    perform_jump_to_address_no_history,
};
pub use menu_model::{MenuCategory, MenuItem, MenuState};
pub use menu_render::{render_menu, render_menu_popup};

pub struct Menu;

impl Widget for Menu {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        render_menu(
            f,
            area,
            &ui_state.menu,
            &ui_state.theme,
            ui_state.new_version_available.as_deref(),
        );
    }

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return WidgetResult::Ignored;
        }

        let menu_state = &mut ui_state.menu;
        let area = ui_state.menu_area;
        let col = mouse.column;
        let row = mouse.row;

        // 1. Check Menu Bar
        if row == area.y && col >= area.x && col < area.x + area.width {
            let mut current_x = area.x;
            for (i, category) in menu_state.categories.iter().enumerate() {
                let width = (category.name.len() + 2) as u16; // " name "
                if col >= current_x && col < current_x + width {
                    menu_state.selected_category = i;
                    menu_state.active = true;
                    menu_state.selected_item = None;
                    return WidgetResult::Handled;
                }
                current_x += width;
            }
        }

        // 2. Check Popup if active
        if menu_state.active {
            // Replicate popup geometry calculation
            let mut x_offset = 0;
            for i in 0..menu_state.selected_category {
                x_offset += menu_state.categories[i].name.len() as u16 + 2;
            }

            let category = &menu_state.categories[menu_state.selected_category];
            let mut max_name_len = 0;
            let mut max_shortcut_len = 0;
            for item in &category.items {
                max_name_len = max_name_len.max(item.name.len());
                max_shortcut_len = max_shortcut_len
                    .max(item.shortcut.as_ref().map_or(0, std::string::String::len));
            }
            let content_width = max_name_len + 2 + max_shortcut_len;
            let width = (content_width as u16 + 2).max(20);
            let height = category.items.len() as u16 + 2;

            let popup_x = area.x + x_offset;
            let popup_y = area.y + 1;

            // Check if click is inside popup
            if col >= popup_x && col < popup_x + width && row >= popup_y && row < popup_y + height {
                // Clicked inside popup
                let rel_y = row - popup_y;
                if rel_y > 0 && rel_y < height - 1 {
                    // Inside borders
                    let item_idx = (rel_y - 1) as usize;
                    if item_idx < category.items.len() {
                        let item = &category.items[item_idx];
                        if item.is_separator {
                            return WidgetResult::Handled;
                        }
                        if item.disabled {
                            return WidgetResult::Handled;
                        }
                        // Execute action
                        if let Some(action) = &item.action {
                            menu_state.active = false;
                            menu_state.selected_item = None;
                            return WidgetResult::Action(action.clone());
                        }
                    }
                }
                return WidgetResult::Handled;
            }
        }

        WidgetResult::Ignored
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.menu.active = false;
                ui_state.menu.selected_item = None;
                ui_state.set_status_message("Ready");
                WidgetResult::Handled
            }
            KeyCode::Right => {
                ui_state.menu.next_category();
                WidgetResult::Handled
            }
            KeyCode::Left => {
                ui_state.menu.previous_category();
                WidgetResult::Handled
            }
            KeyCode::Char('f') if key.modifiers == KeyModifiers::ALT => {
                ui_state.menu.selected_category = 0;
                ui_state.menu.select_first_enabled_item();
                WidgetResult::Handled
            }
            KeyCode::Char('h') if key.modifiers == KeyModifiers::ALT => {
                if let Some(pos) = ui_state
                    .menu
                    .categories
                    .iter()
                    .position(|c| c.name == "Help")
                {
                    ui_state.menu.selected_category = pos;
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            KeyCode::Char('j') if key.modifiers == KeyModifiers::ALT => {
                if let Some(pos) = ui_state
                    .menu
                    .categories
                    .iter()
                    .position(|c| c.name == "Jump")
                {
                    ui_state.menu.selected_category = pos;
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            KeyCode::Char('v') if key.modifiers == KeyModifiers::ALT => {
                if let Some(pos) = ui_state
                    .menu
                    .categories
                    .iter()
                    .position(|c| c.name == "View")
                {
                    ui_state.menu.selected_category = pos;
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::ALT => {
                if let Some(pos) = ui_state
                    .menu
                    .categories
                    .iter()
                    .position(|c| c.name == "Search")
                {
                    ui_state.menu.selected_category = pos;
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            KeyCode::Char('t') if key.modifiers == KeyModifiers::ALT => {
                if let Some(pos) = ui_state
                    .menu
                    .categories
                    .iter()
                    .position(|c| c.name == "Edit")
                {
                    ui_state.menu.selected_category = pos;
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::ALT => {
                if let Some(pos) = ui_state
                    .menu
                    .categories
                    .iter()
                    .position(|c| c.name == "Debugger")
                {
                    ui_state.menu.selected_category = pos;
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            KeyCode::Down => {
                ui_state.menu.next_item();
                WidgetResult::Handled
            }
            KeyCode::Up => {
                ui_state.menu.previous_item();
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if let Some(item_idx) = ui_state.menu.selected_item {
                    let category_idx = ui_state.menu.selected_category;
                    let item = &ui_state.menu.categories[category_idx].items[item_idx];

                    if item.disabled {
                        // Optional: Feedback that it's disabled
                        ui_state.set_status_message("Item is disabled");
                    } else {
                        let action = item.action.clone();
                        if let Some(action) = action {
                            // Close menu after valid action
                            ui_state.menu.active = false;
                            ui_state.menu.selected_item = None;
                            return WidgetResult::Action(action);
                        }
                    }
                } else {
                    // Enter on category -> open first item?
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_render_menu_popup_bounds_panic() {
        // Create a very small terminal (20x5)
        // The default "File" menu is longer than 5 lines
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut menu_state = MenuState::new();
        menu_state.selected_category = 0; // File menu
        menu_state.active = true;

        let theme = crate::theme::Theme::default();

        // This should NOT panic with the fix
        let res = terminal.draw(|f| {
            let area = f.area();
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(0),
                ])
                .split(area);

            let top_area = chunks[0];
            render_menu_popup(f, top_area, &menu_state, &theme);
        });

        assert!(res.is_ok());
    }

    #[test]
    fn test_apply_block_type_single_line_cursor_preservation() {
        let mut app_state = AppState::default();
        app_state.origin = crate::state::Addr(0xC000);
        // 3 bytes: A9 00 EA (LDA #$00; NOP)
        app_state.raw_data = vec![0xA9, 0x00, 0xEA];
        app_state.block_types = vec![crate::state::BlockType::DataByte; 3];
        app_state.disassemble();

        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 2; // Pointing to $C002 (EA)
        ui_state.selection_start = None;
        ui_state.active_pane = crate::ui_state::ActivePane::Disassembly;

        // Change C002 to Code.
        menu_action::apply_block_type(&mut app_state, &mut ui_state, crate::state::BlockType::Code);

        // Should still be at index 2 ($C002)
        assert_eq!(ui_state.cursor_index, 2);
    }
}
