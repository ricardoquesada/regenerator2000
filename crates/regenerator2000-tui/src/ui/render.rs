//! Top-level TUI frame layout composition and pane dispatch.

use super::menu::{self, Menu};
use super::minimap_bar::MinimapBar;
use super::statusbar::StatusBar;
use super::view_bitmap::BitmapView;
use super::view_blocks::BlocksView;
use super::view_charset::CharsetView;
use super::view_debugger::DebuggerView;
use super::view_disassembly::DisassemblyView;
use super::view_hexdump::HexDumpView;
use super::view_sprites::SpritesView;
use super::widget::Widget;
use crate::state::AppState;
use crate::ui_state::{RightPane, UIState};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Renders a full TUI frame and updates cached hit-test rectangles on `ui_state`.
///
/// Draws active menu popups and modal dialogs last so they cover background panes.
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

    Menu.render(f, chunks[0], app_state, ui_state);
    MinimapBar.render(f, ui_state.minimap_area, app_state, ui_state);
    render_main_view(f, chunks[2], app_state, ui_state);
    StatusBar.render(f, chunks[3], app_state, ui_state);

    // Render the menu popup last so it stays on top.
    if ui_state.menu.active {
        menu::render_menu_popup(f, chunks[0], &ui_state.menu, &ui_state.theme);
    }

    // Generic Active Dialog Handler (Refactored Dialogs)
    if let Some(dialog) = ui_state.active_dialog.take() {
        dialog.render(f, f.area(), app_state, ui_state);
        ui_state.active_dialog = Some(dialog);
    } else {
        ui_state.active_dialog_area = Rect::default();
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

    DisassemblyView.render(f, layout[0], app_state, ui_state);

    match ui_state.right_pane {
        RightPane::None => {}
        RightPane::HexDump16 | RightPane::HexDump8 => {
            HexDumpView.render(f, layout[1], app_state, ui_state);
        }
        RightPane::Sprites2Col | RightPane::Sprites1Col => {
            SpritesView.render(f, layout[1], app_state, ui_state);
        }
        RightPane::Charset8Col | RightPane::Charset4Col => {
            CharsetView.render(f, layout[1], app_state, ui_state);
        }
        RightPane::Bitmap => BitmapView.render(f, layout[1], app_state, ui_state),
        RightPane::Blocks => BlocksView.render(f, layout[1], app_state, ui_state),
        RightPane::Debugger => {
            DebuggerView.render(f, layout[1], app_state, ui_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::theme::Theme;
    use crate::ui::dialog_keyboard_shortcut::ShortcutsDialog;
    use crate::ui::dialog_symbolic_offset::{SymbolicOffsetDialog, SymbolicOffsetDialogParams};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Cell;

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
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal
            .draw(|f| ui(f, &app_state, &mut ui_state))
            .expect("draw split right-pane frame");

        assert_eq!(ui_state.menu_area, Rect::new(0, 0, 140, 1));
        assert_eq!(ui_state.minimap_area, Rect::new(0, 1, 140, 1));
        assert_eq!(ui_state.main_area, Rect::new(0, 2, 140, 27));
        assert_eq!(ui_state.status_bar_area, Rect::new(0, 29, 140, 1));
        assert_eq!(ui_state.right_pane_area.width, 78);
        assert_eq!(ui_state.disassembly_area.width, 140 - 78);

        ui_state.right_pane = RightPane::None;
        terminal
            .draw(|f| ui(f, &app_state, &mut ui_state))
            .expect("draw full-width disassembly frame");
        assert_eq!(ui_state.right_pane_area.width, 0);
        assert_eq!(ui_state.disassembly_area.width, 140);
    }

    #[test]
    fn ui_renders_and_restores_active_dialog() {
        let app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        ui_state.dismiss_logo = true;
        ui_state.push_dialog(Box::new(ShortcutsDialog::new()));

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal
            .draw(|f| ui(f, &app_state, &mut ui_state))
            .expect("draw active dialog frame");

        assert!(ui_state.active_dialog.is_some());
        assert!(ui_state.active_dialog_area.width > 0);
    }

    #[test]
    fn ui_renders_symbolic_offset_dialog_modal_overlay() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0800);
        app_state.raw_data = vec![0x85, 0xFC, 0x60];
        app_state.block_types = vec![BlockType::Code; 3];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.dismiss_logo = true;
        ui_state.active_dialog = Some(Box::new(SymbolicOffsetDialog::new(
            SymbolicOffsetDialogParams {
                site_address: Some(Addr(0x0800)),
                target_address: Addr(0x00FC),
                address_range: None,
                initial_input: "zp_ptr + 1",
                is_global: false,
                lock_global: false,
                is_zero_page_mode: true,
            },
        )));

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal
            .draw(|f| ui(f, &app_state, &mut ui_state))
            .expect("draw symbolic offset modal frame");

        assert!(ui_state.active_dialog.is_some());
        assert_eq!(ui_state.active_dialog_area.height, 8);
        assert!(ui_state.active_dialog_area.width >= 92);

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(Cell::symbol)
            .collect();
        assert!(rendered.contains(" Symbolic Offset for $00FC (at $0800) "));
        assert!(rendered.contains("zp_ptr + 1"));
        assert!(rendered.contains("[ ] Apply globally to all $00FC references"));
    }
}
