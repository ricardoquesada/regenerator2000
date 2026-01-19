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

        terminal
            .draw(|f| ui(f, &app_state, &mut ui_state))
            .map_err(|e| io::Error::other(e.to_string()))?;

        if let Event::Key(key) = event::read()? {
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
                        crate::ui::menu::handle_menu_action(&mut app_state, &mut ui_state, action);
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
                let result = crate::ui::menu::Menu.handle_input(key, &mut app_state, &mut ui_state);
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
                    ActivePane::Blocks => Box::new(BlocksView),
                };

                match active_view.handle_input(key, &mut app_state, &mut ui_state) {
                    WidgetResult::Handled => continue,
                    WidgetResult::Action(action) => {
                        crate::ui::menu::handle_menu_action(&mut app_state, &mut ui_state, action);
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
    }
}
