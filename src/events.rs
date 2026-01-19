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
            ui_state.search_dialog.last_search.is_empty(),
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
            if ui_state.jump_to_address_dialog.active {
                crate::ui::dialog_jump_to_address::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.jump_to_line_dialog.active {
                crate::ui::dialog_jump_to_line::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.save_as_dialog.active {
                crate::ui::dialog_save_as::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.export_as_dialog.active {
                crate::ui::dialog_export_as::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.label_dialog.active {
                crate::ui::dialog_label::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.comment_dialog.active {
                crate::ui::dialog_comment::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.open_dialog.active {
                crate::ui::dialog_open::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.search_dialog.active {
                crate::ui::dialog_search::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.menu.active {
                crate::ui::menu::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.about_dialog.active {
                crate::ui::dialog_about::handle_input(key, &mut ui_state);
            } else if ui_state.shortcuts_dialog.active {
                crate::ui::dialog_keyboard_shortcut::handle_input(key, &mut ui_state);
            } else if ui_state.confirmation_dialog.active {
                crate::ui::dialog_confirmation::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.settings_dialog.active {
                crate::ui::dialog_document_settings::handle_input(
                    key,
                    &mut app_state,
                    &mut ui_state,
                );
            } else if ui_state.system_settings_dialog.active {
                crate::ui::dialog_settings::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.origin_dialog.active {
                crate::ui::dialog_origin::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.vim_search_active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.vim_search_active = false;
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        ui_state.search_dialog.last_search = ui_state.vim_search_input.clone();
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
                if ui_state.active_pane == ActivePane::Disassembly {
                    use crate::ui::view_disassembly::InputResult;
                    match crate::ui::view_disassembly::handle_input(
                        key,
                        &mut app_state,
                        &mut ui_state,
                    ) {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            crate::ui::menu::handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                action,
                            );
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                if ui_state.active_pane == ActivePane::HexDump {
                    use crate::ui::view_hexdump::InputResult;
                    match crate::ui::view_hexdump::handle_input(key, &mut app_state, &mut ui_state)
                    {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            crate::ui::menu::handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                action,
                            );
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                if ui_state.active_pane == ActivePane::Sprites {
                    use crate::ui::view_sprites::InputResult;
                    match crate::ui::view_sprites::handle_input(key, &mut app_state, &mut ui_state)
                    {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            crate::ui::menu::handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                action,
                            );
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                if ui_state.active_pane == ActivePane::Charset {
                    use crate::ui::view_charset::InputResult;
                    match crate::ui::view_charset::handle_input(key, &mut app_state, &mut ui_state)
                    {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            crate::ui::menu::handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                action,
                            );
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                if ui_state.active_pane == ActivePane::Blocks {
                    use crate::ui::view_blocks::InputResult;
                    match crate::ui::view_blocks::handle_input(key, &mut app_state, &mut ui_state) {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            crate::ui::menu::handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                action,
                            );
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                handle_global_input(key, &mut app_state, &mut ui_state);
            }

            if ui_state.should_quit {
                return Ok(());
            }
        }
    }
}
