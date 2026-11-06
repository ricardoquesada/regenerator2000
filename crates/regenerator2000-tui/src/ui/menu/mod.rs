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
#[allow(unused_imports)]
pub use menu_action::{AppAction, handle_menu_action};
#[allow(unused_imports)]
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
                    menu_state.select_first_enabled_item();
                    return WidgetResult::Handled;
                }
                current_x += width;
            }
            if menu_state.active {
                menu_state.active = false;
                menu_state.selected_item = None;
                return WidgetResult::Handled;
            }
        }

        // 2. Check Popup if active
        if menu_state.active {
            let term_width = area
                .x
                .saturating_add(area.width)
                .max(
                    ui_state
                        .main_area
                        .x
                        .saturating_add(ui_state.main_area.width),
                )
                .max(
                    ui_state
                        .status_bar_area
                        .x
                        .saturating_add(ui_state.status_bar_area.width),
                );
            let known_bottom = area
                .y
                .saturating_add(area.height)
                .max(
                    ui_state
                        .minimap_area
                        .y
                        .saturating_add(ui_state.minimap_area.height),
                )
                .max(
                    ui_state
                        .main_area
                        .y
                        .saturating_add(ui_state.main_area.height),
                )
                .max(
                    ui_state
                        .status_bar_area
                        .y
                        .saturating_add(ui_state.status_bar_area.height),
                );
            let term_height = if known_bottom > area.y.saturating_add(1) {
                known_bottom
            } else {
                u16::MAX
            };
            let bar_area = Rect::new(area.x, area.y, area.width, 1);
            let popup_area =
                menu_state.popup_rect(bar_area, Rect::new(0, 0, term_width, term_height));

            // Check if click is inside popup
            if col >= popup_area.x
                && col < popup_area.x.saturating_add(popup_area.width)
                && row >= popup_area.y
                && row < popup_area.y.saturating_add(popup_area.height)
            {
                let rel_y = row - popup_area.y;
                if rel_y > 0 && rel_y < popup_area.height.saturating_sub(1) {
                    let inner_height = (popup_area.height as usize).saturating_sub(2);
                    let offset = menu_state.popup_scroll_offset(inner_height);
                    let item_idx = offset + (rel_y - 1) as usize;
                    let category = &menu_state.categories[menu_state.selected_category];
                    if let Some(item) = category.items.get(item_idx) {
                        if item.is_separator || item.disabled {
                            return WidgetResult::Handled;
                        }
                        if let Some(action) = &item.action {
                            let action = action.clone();
                            menu_state.active = false;
                            menu_state.selected_item = None;
                            return WidgetResult::Action(action);
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
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut menu_state = MenuState::new();
        menu_state.selected_category = 0; // File menu
        menu_state.active = true;

        let theme = crate::theme::Theme::default();

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
    fn test_next_item_from_none_and_mouse_category_click() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.menu_area = Rect::new(0, 0, 80, 1);
        ui_state.menu.selected_item = None;

        // next_item from None should select index 0, not index 1
        ui_state.menu.next_item();
        assert_eq!(ui_state.menu.selected_item, Some(0));

        // Reset and test mouse category click on "File" (x = 1, y = 0)
        ui_state.menu.active = false;
        ui_state.menu.selected_item = None;
        let mut menu = Menu;
        let click_file = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            menu.handle_mouse(click_file, &mut app_state, &mut ui_state),
            WidgetResult::Handled
        );
        assert!(ui_state.menu.active);
        assert_eq!(ui_state.menu.selected_category, 0);
        assert_eq!(ui_state.menu.selected_item, Some(0));

        // Clicking empty space to the right of all categories on the menu bar closes the menu
        let click_empty_bar = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 75,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            menu.handle_mouse(click_empty_bar, &mut app_state, &mut ui_state),
            WidgetResult::Handled
        );
        assert!(!ui_state.menu.active);
        assert_eq!(ui_state.menu.selected_item, None);
    }

    #[test]
    fn test_popup_scrolling_and_mouse_hit_testing_on_24_row_terminal() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.menu_area = Rect::new(0, 0, 80, 1);
        ui_state.status_bar_area = Rect::new(0, 23, 80, 1);

        // Select "Edit" category (index 1, 39 items) and select the last item ("Document Settings", index 38)
        ui_state.menu.active = true;
        ui_state.menu.selected_category = 1;
        let last_idx = ui_state.menu.categories[1].items.len() - 1;
        ui_state.menu.selected_item = Some(last_idx);

        let popup_rect = ui_state
            .menu
            .popup_rect(ui_state.menu_area, Rect::new(0, 0, 80, 24));
        assert_eq!(popup_rect.y, 1);
        assert_eq!(popup_rect.height, 23);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_menu_popup(f, ui_state.menu_area, &ui_state.menu, &ui_state.theme);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let last_visible_row: String = (popup_rect.x..popup_rect.x + popup_rect.width)
            .map(|x| {
                buffer
                    .cell((x, 22))
                    .map_or(String::new(), |c| c.symbol().to_string())
            })
            .collect();
        assert!(
            last_visible_row.contains("Document Settings"),
            "Expected scrolled popup row 22 to contain 'Document Settings', got: {last_visible_row:?}"
        );

        let mut menu = Menu;
        // Clicking the bottom border at row 23 (`rel_y == popup_rect.height - 1`) is rejected as Handled
        let click_bottom_border = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: popup_rect.x + 2,
            row: 23,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            menu.handle_mouse(click_bottom_border, &mut app_state, &mut ui_state),
            WidgetResult::Handled
        );
        assert!(ui_state.menu.active);

        // Clicking row 22 (the last visible inner row) triggers the scrolled last item ("Document Settings")
        let click_last_visible = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: popup_rect.x + 2,
            row: 22,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            menu.handle_mouse(click_last_visible, &mut app_state, &mut ui_state),
            WidgetResult::Action(AppAction::DocumentSettings)
        );
        assert!(!ui_state.menu.active);
    }
}
