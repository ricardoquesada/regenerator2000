pub mod input;

use crate::state::AppState;
use crate::ui::ui;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{self, Event, KeyCode};
use input::handle_global_input;
use ratatui::{Terminal, backend::Backend};
use std::io;

pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app_state: AppState,
    mut ui_state: UIState,
) -> io::Result<()> {
    loop {
        // Update menu availability based on current state
        ui_state.menu.update_availability(
            &app_state,
            ui_state.cursor_index,
            ui_state.last_search_query.is_empty(),
            ui_state.active_pane,
        );

        if ui_state.active_pane == ActivePane::Disassembly
            && ui_state.right_pane == crate::ui_state::RightPane::Blocks
            && app_state.system_config.sync_blocks_view
            && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
            && let Some(idx) = app_state.get_block_index_for_address(line.address)
        {
            ui_state.blocks_list_state.select(Some(idx));
        }

        // Sync HexDump view with Disassembly when active on Disassembly
        if ui_state.active_pane == ActivePane::Disassembly
            && ui_state.right_pane == crate::ui_state::RightPane::HexDump
            && app_state.system_config.sync_hex_dump
            && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
        {
            let origin = app_state.origin as usize;
            let alignment_padding = origin % 16;
            let aligned_origin = origin - alignment_padding;
            let target_addr = line.address as usize;

            if target_addr >= aligned_origin {
                let offset = target_addr - aligned_origin;
                let row = offset / 16;
                let bytes_per_row = 16;
                let total_len = app_state.raw_data.len() + alignment_padding;
                let max_rows = total_len.div_ceil(bytes_per_row);
                if row < max_rows {
                    ui_state.hex_cursor_index = row;
                }
            }
        }

        // Sync Charset view with Disassembly when active on Disassembly
        if ui_state.active_pane == ActivePane::Disassembly
            && ui_state.right_pane == crate::ui_state::RightPane::Charset
            && app_state.system_config.sync_charset_view
            && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
        {
            let origin = app_state.origin as usize;
            let base_alignment = 0x400;
            // Use floor alignment to match view indexing
            let aligned_start_addr = (origin / base_alignment) * base_alignment;
            let target_addr = line.address as usize;

            if target_addr >= aligned_start_addr {
                let char_offset = target_addr - aligned_start_addr;
                let idx = char_offset / 8;

                let end_addr = origin + app_state.raw_data.len();
                let total_chars = (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

                if idx < total_chars {
                    ui_state.charset_cursor_index = idx;
                }
            }
        }

        // Sync Sprites view with Disassembly when active on Disassembly
        if ui_state.active_pane == ActivePane::Disassembly
            && ui_state.right_pane == crate::ui_state::RightPane::Sprites
            && app_state.system_config.sync_sprites_view
            && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
        {
            let origin = app_state.origin as usize;
            // Use floor alignment to match view indexing
            let aligned_origin = (origin / 64) * 64;
            let target_addr = line.address as usize;

            if target_addr >= aligned_origin {
                let offset = target_addr - aligned_origin;
                let idx = offset / 64;

                let data_len = app_state.raw_data.len();
                let end_addr = origin + data_len;
                let total_sprites = (end_addr.saturating_sub(aligned_origin)).div_ceil(64);

                if idx < total_sprites {
                    ui_state.sprites_cursor_index = idx;
                }
            }
        }

        // Sync Bitmap view with Disassembly when active on Disassembly
        if ui_state.active_pane == ActivePane::Disassembly
            && ui_state.right_pane == crate::ui_state::RightPane::Bitmap
            && app_state.system_config.sync_bitmap_view
            && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
        {
            let origin = app_state.origin as usize;
            let target_addr = line.address as usize;

            // Bitmaps must be aligned to 8192-byte ($2000) boundaries
            // Use floor alignment to match view indexing
            let first_aligned_addr = (origin / 8192) * 8192;

            if target_addr >= first_aligned_addr {
                let offset = target_addr - first_aligned_addr;
                let idx = offset / 8192;

                // Calculate total number of bitmaps available
                let data_len = app_state.raw_data.len();
                let end_addr = origin + data_len;
                let total_bitmaps = (end_addr.saturating_sub(first_aligned_addr)).div_ceil(8192);

                if idx < total_bitmaps {
                    ui_state.bitmap_cursor_index = idx;
                }
            }
        }

        terminal
            .draw(|f| ui(f, &app_state, &mut ui_state))
            .map_err(|e| io::Error::other(e.to_string()))?;

        match event::read()? {
            Event::Key(key) => {
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }
                ui_state.dismiss_logo = true;

                // Handle Active Dialog (Generic)
                if let Some(mut dialog) = ui_state.active_dialog.take() {
                    let result = dialog.handle_input(key, &mut app_state, &mut ui_state);
                    match result {
                        crate::ui::widget::WidgetResult::Ignored
                        | crate::ui::widget::WidgetResult::Handled => {
                            ui_state.active_dialog = Some(dialog)
                        }
                        crate::ui::widget::WidgetResult::Close => {
                            // Dialog closed.
                        }
                        crate::ui::widget::WidgetResult::Action(action) => {
                            ui_state.active_dialog = Some(dialog);
                            crate::ui::menu::handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                action,
                            );
                        }
                    }
                    if ui_state.should_quit {
                        return Ok(());
                    }
                    continue;
                }
                // Label dialog removed (generic)            // Comment dialog removed (generic)
                if ui_state.menu.active {
                    use crate::ui::widget::Widget;
                    let result =
                        crate::ui::menu::Menu.handle_input(key, &mut app_state, &mut ui_state);
                    if let crate::ui::widget::WidgetResult::Action(action) = result {
                        crate::ui::menu::handle_menu_action(&mut app_state, &mut ui_state, action);
                    }
                    // Confirmation dialog removed (generic)
                    // Origin dialog removed (generic)
                } else if ui_state.vim_search_active {
                    match key.code {
                        KeyCode::Esc => {
                            ui_state.vim_search_active = false;
                            ui_state.set_status_message("Ready");
                        }
                        KeyCode::Enter => {
                            ui_state.last_search_query = ui_state.vim_search_input.clone();
                            ui_state.vim_search_active = false;
                            crate::ui::dialog_search::perform_search(
                                &mut app_state,
                                &mut ui_state,
                                true,
                            );
                        }
                        KeyCode::Backspace => {
                            ui_state.vim_search_input.pop();
                        }
                        KeyCode::Char(c) => {
                            ui_state.vim_search_input.push(c);
                        }
                        _ => {}
                    }
                } else {
                    use crate::ui::view_bitmap::BitmapView;
                    use crate::ui::view_blocks::BlocksView;
                    use crate::ui::view_charset::CharsetView;
                    use crate::ui::view_disassembly::DisassemblyView;
                    use crate::ui::view_hexdump::HexDumpView;
                    use crate::ui::view_sprites::SpritesView;
                    use crate::ui::widget::{Widget, WidgetResult};

                    let mut active_view: Box<dyn Widget> = match ui_state.active_pane {
                        ActivePane::Disassembly => Box::new(DisassemblyView),
                        ActivePane::HexDump => Box::new(HexDumpView),
                        ActivePane::Sprites => Box::new(SpritesView),
                        ActivePane::Charset => Box::new(CharsetView),
                        ActivePane::Bitmap => Box::new(BitmapView),
                        ActivePane::Blocks => Box::new(BlocksView),
                    };

                    match active_view.handle_input(key, &mut app_state, &mut ui_state) {
                        WidgetResult::Handled => continue,
                        WidgetResult::Action(action) => {
                            crate::ui::menu::handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                action,
                            );
                            continue;
                        }
                        WidgetResult::Ignored => {}
                        WidgetResult::Close => {}
                    }

                    handle_global_input(key, &mut app_state, &mut ui_state);
                }

                if ui_state.should_quit {
                    return Ok(());
                }
            }
            Event::Mouse(mouse) => {
                ui_state.dismiss_logo = true;

                // Handle Active Dialog (Modal) - Capture all mouse events
                if let Some(mut dialog) = ui_state.active_dialog.take() {
                    let result = dialog.handle_mouse(mouse, &mut app_state, &mut ui_state);
                    match result {
                        crate::ui::widget::WidgetResult::Ignored
                        | crate::ui::widget::WidgetResult::Handled => {
                            ui_state.active_dialog = Some(dialog)
                        }
                        crate::ui::widget::WidgetResult::Close => {
                            // Dialog closed.
                        }
                        crate::ui::widget::WidgetResult::Action(action) => {
                            ui_state.active_dialog = Some(dialog);
                            crate::ui::menu::handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                action,
                            );
                        }
                    }
                    if ui_state.should_quit {
                        return Ok(());
                    }
                    continue;
                }

                let col = mouse.column;
                let row = mouse.row;

                let mut widget_result = crate::ui::widget::WidgetResult::Ignored;
                let is_inside = |rect: ratatui::layout::Rect, col: u16, row: u16| {
                    col >= rect.x
                        && col < rect.x + rect.width
                        && row >= rect.y
                        && row < rect.y + rect.height
                };

                use crate::ui::widget::Widget;

                if is_inside(ui_state.menu_area, col, row) {
                    widget_result =
                        crate::ui::menu::Menu.handle_mouse(mouse, &mut app_state, &mut ui_state);
                } else if ui_state.menu.active {
                    // If menu is active and we clicked outside menu area.
                    // We let Menu handle it (it might detect click in popup).
                    widget_result =
                        crate::ui::menu::Menu.handle_mouse(mouse, &mut app_state, &mut ui_state);

                    // If the menu ignored it (e.g. click outside both bar and popup), close the menu.
                    if matches!(widget_result, crate::ui::widget::WidgetResult::Ignored)
                        && mouse.kind
                            == event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                    {
                        ui_state.menu.active = false;
                        ui_state.menu.selected_item = None;
                        // Fallthrough to allow clicking on underlying view?
                        // Usually click-away just closes menu.
                        // Let's accept that closing the menu consumes the click to avoid accidental action on view.
                        widget_result = crate::ui::widget::WidgetResult::Handled;
                    }
                }

                if widget_result == crate::ui::widget::WidgetResult::Ignored {
                    if is_inside(ui_state.disassembly_area, col, row) {
                        ui_state.active_pane = ActivePane::Disassembly;
                        use crate::ui::view_disassembly::DisassemblyView;
                        widget_result =
                            DisassemblyView.handle_mouse(mouse, &mut app_state, &mut ui_state);
                    } else if is_inside(ui_state.right_pane_area, col, row) {
                        match ui_state.right_pane {
                            crate::ui_state::RightPane::HexDump => {
                                ui_state.active_pane = ActivePane::HexDump
                            }
                            crate::ui_state::RightPane::Sprites => {
                                ui_state.active_pane = ActivePane::Sprites
                            }
                            crate::ui_state::RightPane::Charset => {
                                ui_state.active_pane = ActivePane::Charset
                            }
                            crate::ui_state::RightPane::Bitmap => {
                                ui_state.active_pane = ActivePane::Bitmap
                            }
                            crate::ui_state::RightPane::Blocks => {
                                ui_state.active_pane = ActivePane::Blocks
                            }
                            _ => {}
                        }

                        use crate::ui::view_bitmap::BitmapView;
                        use crate::ui::view_blocks::BlocksView;
                        use crate::ui::view_charset::CharsetView;
                        use crate::ui::view_disassembly::DisassemblyView;
                        use crate::ui::view_hexdump::HexDumpView;
                        use crate::ui::view_sprites::SpritesView;

                        let mut active_view: Box<dyn Widget> = match ui_state.active_pane {
                            ActivePane::Disassembly => Box::new(DisassemblyView),
                            ActivePane::HexDump => Box::new(HexDumpView),
                            ActivePane::Sprites => Box::new(SpritesView),
                            ActivePane::Charset => Box::new(CharsetView),
                            ActivePane::Bitmap => Box::new(BitmapView),
                            ActivePane::Blocks => Box::new(BlocksView),
                        };
                        widget_result =
                            active_view.handle_mouse(mouse, &mut app_state, &mut ui_state);
                    }
                }

                if let crate::ui::widget::WidgetResult::Action(action) = widget_result {
                    crate::ui::menu::handle_menu_action(&mut app_state, &mut ui_state, action);
                }
            }
            _ => {}
        }
    }
}
