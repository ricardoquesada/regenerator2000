use crate::state::AppState;
use crate::ui_state::{RightPane, UIState};

pub(crate) mod dialog_about;
pub(crate) mod dialog_apply_enum;
pub(crate) mod dialog_bookmarks;
pub(crate) mod dialog_comment;
pub(crate) mod dialog_complete_address;
pub(crate) mod dialog_confirmation;
pub mod dialog_container_picker;
pub mod dialog_crt_picker;
pub(crate) mod dialog_document_settings;
pub(crate) mod dialog_edit_enum;
pub(crate) mod dialog_file_browser;
pub(crate) mod dialog_file_info;
pub(crate) mod dialog_find_references;
pub(crate) mod dialog_go_to_symbol;
pub(crate) mod dialog_import_context;
pub(crate) mod dialog_keyboard_shortcut;
pub(crate) mod dialog_label;
pub(crate) mod dialog_manage_enums;
pub(crate) mod dialog_open_recent;
pub(crate) mod dialog_prompt;
pub(crate) mod dialog_search;
pub(crate) mod dialog_settings;
pub(crate) mod dialog_unpack;
pub(crate) mod dialog_vice_connect;
pub(crate) mod dialog_watchpoint_address;
pub(crate) mod graphics_common;
pub(crate) mod menu;
pub(crate) mod minimap_bar;
pub(crate) mod navigable;
pub(crate) mod statusbar;
pub(crate) mod view_bitmap;
pub(crate) mod view_blocks;
pub(crate) mod view_charset;
pub(crate) mod view_debugger;
pub(crate) mod view_disassembly;
pub(crate) mod view_hexdump;
pub(crate) mod view_sprites;
pub mod widget;

use crate::ui::widget::Widget;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

pub fn ui(f: &mut Frame, app_state: &AppState, ui_state: &mut UIState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Menu
            Constraint::Length(1), // Minimap Bar
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    ui_state.menu_area = chunks[0];
    ui_state.minimap_area = chunks[1];
    ui_state.main_area = chunks[2];
    ui_state.status_bar_area = chunks[3];

    menu::Menu.render(f, chunks[0], app_state, ui_state);
    minimap_bar::MinimapBar.render(f, ui_state.minimap_area, app_state, ui_state);
    render_main_view(f, chunks[2], app_state, ui_state);
    statusbar::StatusBar.render(f, chunks[3], app_state, ui_state);

    // Render the menu popup last so it stays on top.
    if ui_state.menu.active {
        menu::render_menu_popup(f, chunks[0], &ui_state.menu, &ui_state.theme);
    }

    // Generic Active Dialog Handler (Refactored Dialogs)
    if let Some(dialog) = ui_state.active_dialog.take() {
        dialog.render(f, f.area(), app_state, ui_state);
        ui_state.active_dialog = Some(dialog);
    }
}

fn render_main_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    // Calculate required width for Right Pane
    let right_pane_width = match ui_state.right_pane {
        RightPane::None => 0,
        RightPane::HexDump16 => 78,
        RightPane::HexDump8 => 42,
        RightPane::Sprites2Col => 62, // Two sprites side-by-side + divider + border
        RightPane::Sprites1Col => 36, // 24 chars + border + padding
        RightPane::Charset8Col => 76, // Grid view: 8 cols * (8+1) width + padding
        RightPane::Charset4Col => 40, // Grid view: 4 cols * (8+1) width + padding
        RightPane::Bitmap => 80,      // Compact view, ratatui-image scales to fit
        RightPane::Blocks => 42,
        RightPane::Debugger => 76,
    };
    let disasm_view_width = area.width.saturating_sub(right_pane_width);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(disasm_view_width),
            Constraint::Length(right_pane_width),
        ])
        .split(area);

    ui_state.disassembly_area = layout[0];
    ui_state.right_pane_area = layout[1];

    view_disassembly::DisassemblyView.render(f, layout[0], app_state, ui_state);

    match ui_state.right_pane {
        RightPane::None => {}
        RightPane::HexDump16 | RightPane::HexDump8 => {
            view_hexdump::HexDumpView.render(f, layout[1], app_state, ui_state);
        }
        RightPane::Sprites2Col | RightPane::Sprites1Col => {
            view_sprites::SpritesView.render(f, layout[1], app_state, ui_state);
        }
        RightPane::Charset8Col | RightPane::Charset4Col => {
            view_charset::CharsetView.render(f, layout[1], app_state, ui_state);
        }
        RightPane::Bitmap => view_bitmap::BitmapView.render(f, layout[1], app_state, ui_state),
        RightPane::Blocks => view_blocks::BlocksView.render(f, layout[1], app_state, ui_state),
        RightPane::Debugger => {
            view_debugger::DebuggerView.render(f, layout[1], app_state, ui_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::theme::Theme;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn ui_populates_layout_rects_and_splits_right_pane() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0800);
        app_state.raw_data = vec![0xEA; 32];
        app_state.block_types = vec![BlockType::Code; 32];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.dismiss_logo = true;
        ui_state.right_pane = RightPane::HexDump16;

        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app_state, &mut ui_state)).unwrap();

        assert_eq!(ui_state.menu_area, Rect::new(0, 0, 140, 1));
        assert_eq!(ui_state.minimap_area, Rect::new(0, 1, 140, 1));
        assert_eq!(ui_state.main_area, Rect::new(0, 2, 140, 27));
        assert_eq!(ui_state.status_bar_area, Rect::new(0, 29, 140, 1));
        assert_eq!(ui_state.right_pane_area.width, 78);
        assert_eq!(ui_state.disassembly_area.width, 140 - 78);

        ui_state.right_pane = RightPane::None;
        terminal.draw(|f| ui(f, &app_state, &mut ui_state)).unwrap();
        assert_eq!(ui_state.right_pane_area.width, 0);
        assert_eq!(ui_state.disassembly_area.width, 140);
    }

    #[test]
    fn ui_renders_and_restores_active_dialog() {
        let app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        ui_state.dismiss_logo = true;
        ui_state.push_dialog(Box::new(
            crate::ui::dialog_keyboard_shortcut::ShortcutsDialog::new(),
        ));

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app_state, &mut ui_state)).unwrap();

        assert!(ui_state.active_dialog.is_some());
        assert!(ui_state.active_dialog_area.width > 0);
    }
}
