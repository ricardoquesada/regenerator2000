pub mod input;

use crate::state::AppState;
use crate::ui::ui;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{self, Event, KeyCode};
use input::handle_global_input;
use ratatui::{Terminal, backend::Backend};
use std::io;

pub enum AppEvent {
    Crossterm(Event),
    Mcp(crate::mcp::types::McpRequest),
    McpError(String),
    Vice(crate::vice::ViceEvent),
    Tick,
}

pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app_state: AppState,
    mut ui_state: UIState,
    event_tx: std::sync::mpsc::Sender<AppEvent>,
    event_rx: std::sync::mpsc::Receiver<AppEvent>,
) -> io::Result<()> {
    // Initial render before event loop
    terminal
        .draw(|f| ui(f, &app_state, &mut ui_state))
        .map_err(|e| io::Error::other(e.to_string()))?;

    let mut should_render = false;

    loop {
        // Wait for event (blocking)
        let event = match event_rx.recv() {
            Ok(e) => e,
            Err(_) => return Ok(()), // Channel closed
        };

        match event {
            AppEvent::Mcp(req) => {
                let response =
                    crate::mcp::handler::handle_request(&req, &mut app_state, &mut ui_state);
                let _ = req.response_sender.send(response);
                should_render = true;
            }
            AppEvent::McpError(err_msg) => {
                ui_state.set_status_message(format!("MCP Error: {}", err_msg));
                should_render = true;
            }
            AppEvent::Vice(vice_event) => {
                match vice_event {
                    crate::vice::ViceEvent::Connected => {
                        app_state.vice_state.connected = true;
                        if ui_state.right_pane == crate::ui_state::RightPane::None {
                            ui_state.right_pane = crate::ui_state::RightPane::Debugger;
                            ui_state.active_pane = ActivePane::Debugger;
                        }
                        // Sync existing breakpoints from VICE (e.g. set in a previous session)
                        if let Some(client) = &app_state.vice_client {
                            client.send_checkpoint_list();
                        }
                        ui_state.set_status_message("Connected to VICE Monitor");
                    }
                    crate::vice::ViceEvent::Disconnected(reason) => {
                        app_state.vice_state.connected = false;
                        app_state.vice_state.reset_registers();
                        ui_state.set_status_message(format!("Disconnected from VICE: {}", reason));
                    }
                    crate::vice::ViceEvent::Message(msg) => {
                        // Handle register get to update PC
                        if msg.command == crate::vice::ViceCommand::REGISTERS_GET {
                            let payload = &msg.payload;
                            if payload.len() >= 2 {
                                let ref_count = u16::from_le_bytes([payload[0], payload[1]]);
                                let mut offset = 2;
                                let mut pc_found = None;

                                for _ in 0..ref_count {
                                    if offset >= payload.len() {
                                        break;
                                    }
                                    let item_size = payload[offset] as usize;
                                    if offset + 1 + item_size > payload.len() {
                                        break;
                                    }

                                    let reg_id = payload[offset + 1];
                                    match reg_id {
                                        0x00 if item_size >= 2 => {
                                            app_state.vice_state.a = Some(payload[offset + 2]);
                                        }
                                        0x01 if item_size >= 2 => {
                                            app_state.vice_state.x = Some(payload[offset + 2]);
                                        }
                                        0x02 if item_size >= 2 => {
                                            app_state.vice_state.y = Some(payload[offset + 2]);
                                        }
                                        0x03 if item_size >= 3 => {
                                            let pc_val = u16::from_le_bytes([
                                                payload[offset + 2],
                                                payload[offset + 3],
                                            ]);
                                            pc_found = Some(pc_val);
                                        }
                                        0x04 if item_size >= 2 => {
                                            app_state.vice_state.sp = Some(payload[offset + 2]);
                                        }
                                        0x05 if item_size >= 2 => {
                                            app_state.vice_state.p = Some(payload[offset + 2]);
                                        }
                                        _ => {}
                                    }

                                    offset += 1 + item_size;
                                }

                                if let Some(pc) = pc_found {
                                    app_state.vice_state.pc = Some(pc);
                                    ui_state.set_status_message(format!("VICE PC: ${:04X}", pc));

                                    // Jump static disassembly cursor + scroll to PC.
                                    // unwrap_or_else(|i| i-1) finds the containing instruction
                                    // when PC is mid-instruction or in a data region.
                                    if !app_state.disassembly.is_empty() {
                                        let idx = app_state
                                            .disassembly
                                            .binary_search_by_key(&pc, |l| l.address)
                                            .unwrap_or_else(|i| {
                                                i.saturating_sub(1)
                                                    .min(app_state.disassembly.len() - 1)
                                            });
                                        let sub_idx = crate::ui::view_disassembly::DisassemblyView::get_sub_index_for_address(
                                            &app_state.disassembly[idx],
                                            &app_state,
                                            pc,
                                        );
                                        ui_state.cursor_index = idx;
                                        ui_state.sub_cursor_index = sub_idx;
                                        ui_state.scroll_index = idx;
                                        ui_state.scroll_sub_index = sub_idx;
                                    }

                                    // Request live memory around the PC for live disassembly.
                                    // Fetch 32 bytes before PC and 96 bytes after (128 total window).
                                    // This covers roughly 40+ instructions around the current PC.
                                    // Also fetch the stack page ($0100–$01FF) for the stack view.
                                    if let Some(client) = &app_state.vice_client {
                                        let before: u16 = 32;
                                        let after: u16 = 95;
                                        let mem_start = pc.saturating_sub(before);
                                        let mem_end = pc.saturating_add(after);
                                        client.send_memory_get(mem_start, mem_end, 1);
                                        client.send_memory_get(0x0100, 0x01FF, 2);
                                        // Also store mem_start temporarily in vice_state or just calculate it
                                        // We can store it directly when sending, or reconstruct it on receive.
                                        app_state.vice_state.live_memory_start = mem_start;
                                    }
                                } else {
                                    ui_state.set_status_message(
                                        "VICE Registers: did not find PC".to_string(),
                                    );
                                }
                            }
                        } else if msg.command == crate::vice::ViceCommand::MEMORY_GET
                            && msg.error_code == 0
                        {
                            // MEMORY_GET response payload: length (2 LE) + bytes
                            let payload = &msg.payload;
                            if payload.len() >= 2 {
                                let mem_len = u16::from_le_bytes([payload[0], payload[1]]) as usize;
                                if payload.len() >= 2 + mem_len && mem_len > 0 {
                                    let bytes = payload[2..2 + mem_len].to_vec();
                                    if msg.request_id == 2 {
                                        // Stack page response ($0100–$01FF)
                                        app_state.vice_state.stack_memory = Some(bytes);
                                    } else if msg.request_id == 1 {
                                        // Live disassembly window around PC
                                        // live_memory_start was saved when the request was sent
                                        app_state.vice_state.live_memory = Some(bytes);
                                    }
                                }
                            }
                        } else if (msg.command == crate::vice::ViceCommand::CHECKPOINT_SET
                            || msg.command == crate::vice::ViceCommand::CHECKPOINT_GET)
                            && msg.error_code == 0
                        {
                            // CHECKPOINT_SET (0x12) and CHECKPOINT_GET (0x11) both carry the
                            // same checkpoint_info body:
                            //   id(4) start(2) end(2) stop(1) enabled(1) cpu_op(1) temporary(1)
                            //   hit_count(4) ignore_count(4) has_cond(1) — 21 bytes total
                            //
                            // VICE uses 0x11 for:
                            //   - Individual responses after a CHECKPOINT_LIST request
                            //   - CHECKPOINT_SET acknowledgment in some VICE versions
                            // VICE uses 0x12 for:
                            //   - CHECKPOINT_SET acknowledgment in other VICE versions
                            // Handling both here makes us robust to all versions.
                            let p = &msg.payload;
                            if p.len() >= 13 {
                                let id = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
                                let addr = u16::from_le_bytes([p[5], p[6]]);
                                let enabled = p[10] != 0;
                                let temporary = p[12] != 0;
                                // Only track persistent breakpoints (not run-to-cursor temps)
                                if !temporary {
                                    // Avoid duplicates (e.g. if both 0x11 and 0x12 arrive for same checkpoint)
                                    if !app_state
                                        .vice_state
                                        .breakpoints
                                        .iter()
                                        .any(|bp| bp.id == id)
                                    {
                                        app_state.vice_state.breakpoints.push(
                                            crate::vice::state::ViceBreakpoint {
                                                id,
                                                address: addr,
                                                enabled,
                                            },
                                        );
                                    }
                                }
                            }
                        } else if msg.command == crate::vice::ViceCommand::CHECKPOINT_LIST
                            && msg.error_code == 0
                        {
                            // CHECKPOINT_LIST response: just a count (4 bytes).
                            // VICE then sends individual CHECKPOINT_GET (0x11) responses for each
                            // checkpoint — those are handled above. Clear the list here so the
                            // incoming 0x11 responses repopulate it cleanly.
                            app_state.vice_state.breakpoints.clear();
                        } else if msg.command == crate::vice::ViceCommand::STOPPED {
                            // CPU stopped (step complete, step-over complete, or checkpoint hit).
                            // Fetch registers so the debugger panel and live view update.
                            if let Some(client) = &app_state.vice_client {
                                client.send_registers_get();
                            }
                        } else if msg.command == crate::vice::ViceCommand::ADVANCE_INSTRUCTION
                            && msg.error_code == 0
                        {
                            // Step/step-over acknowledged — fetch updated registers.
                            if let Some(client) = &app_state.vice_client {
                                client.send_registers_get();
                            }
                        } else if msg.error_code != 0 {
                            // Ignore error responses silently (e.g. memory not accessible)
                        } else {
                            // Silence known commands that need no further handling
                            let silent = matches!(
                                msg.command,
                                crate::vice::ViceCommand::CHECKPOINT_DELETE
                                    | crate::vice::ViceCommand::RESUMED
                                    | crate::vice::ViceCommand::EXIT_MONITOR
                            );
                            if !silent {
                                ui_state
                                    .set_status_message(format!("VICE Msg: {:02x}", msg.command));
                            }
                        }
                    }
                }
                should_render = true;
            }
            AppEvent::Tick => {
                // Optional tick for animations/time-based updates if needed later
            }
            AppEvent::Crossterm(event) => {
                match event {
                    Event::Key(key) => {
                        if key.kind != event::KeyEventKind::Press {
                            continue;
                        }

                        if !ui_state.dismiss_logo {
                            ui_state.dismiss_logo = true;
                        }
                        should_render = true;

                        // Handle Active Dialog (Generic)
                        if let Some(mut dialog) = ui_state.active_dialog.take() {
                            let result = dialog.handle_input(key, &mut app_state, &mut ui_state);
                            match result {
                                crate::ui::widget::WidgetResult::Ignored => {
                                    ui_state.active_dialog = Some(dialog)
                                }
                                crate::ui::widget::WidgetResult::Handled => {
                                    ui_state.active_dialog = Some(dialog);
                                    should_render = true;
                                }
                                crate::ui::widget::WidgetResult::Close => {
                                    // Dialog closed.
                                    should_render = true;
                                }
                                crate::ui::widget::WidgetResult::Action(action) => {
                                    let is_vice_connect = matches!(
                                        action,
                                        crate::ui::menu::MenuAction::ViceConnectAddress(_)
                                    );
                                    if !is_vice_connect {
                                        ui_state.active_dialog = Some(dialog);
                                    }
                                    should_render = true;
                                    if let crate::ui::menu::MenuAction::ViceConnectAddress(addr) =
                                        action
                                    {
                                        if let Ok(client) = crate::vice::ViceClient::connect(
                                            &addr,
                                            event_tx.clone(),
                                        ) {
                                            app_state.vice_client = Some(client);
                                        } else {
                                            ui_state
                                                .set_status_message("Failed to connect to VICE");
                                        }
                                    } else if action == crate::ui::menu::MenuAction::ViceDisconnect
                                    {
                                        app_state.vice_client = None;
                                        app_state.vice_state.connected = false;
                                        ui_state.set_status_message("Disconnected from VICE");
                                    } else if action == crate::ui::menu::MenuAction::ViceStep {
                                        if let Some(client) = &app_state.vice_client {
                                            client.send_advance_instruction();
                                        }
                                    } else {
                                        crate::ui::menu::handle_menu_action(
                                            &mut app_state,
                                            &mut ui_state,
                                            action,
                                        );
                                    }
                                }
                            }
                            if ui_state.should_quit {
                                return Ok(());
                            }
                            // Don't continue - let it fall through to render
                        } else if ui_state.menu.active {
                            use crate::ui::widget::Widget;
                            let result = crate::ui::menu::Menu.handle_input(
                                key,
                                &mut app_state,
                                &mut ui_state,
                            );
                            if let crate::ui::widget::WidgetResult::Action(action) = result {
                                if let crate::ui::menu::MenuAction::ViceConnectAddress(addr) =
                                    action
                                {
                                    if let Ok(client) =
                                        crate::vice::ViceClient::connect(&addr, event_tx.clone())
                                    {
                                        app_state.vice_client = Some(client);
                                    } else {
                                        ui_state.set_status_message("Failed to connect to VICE");
                                    }
                                } else if action == crate::ui::menu::MenuAction::ViceDisconnect {
                                    app_state.vice_client = None;
                                    app_state.vice_state.connected = false;
                                    ui_state.set_status_message("Disconnected from VICE");
                                } else if action == crate::ui::menu::MenuAction::ViceStep {
                                    if let Some(client) = &app_state.vice_client {
                                        client.send_advance_instruction();
                                    }
                                } else {
                                    crate::ui::menu::handle_menu_action(
                                        &mut app_state,
                                        &mut ui_state,
                                        action,
                                    );
                                }
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
                            use crate::ui::view_debugger::DebuggerView;
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
                                ActivePane::Debugger => Box::new(DebuggerView),
                            };

                            match active_view.handle_input(key, &mut app_state, &mut ui_state) {
                                WidgetResult::Handled => {
                                    // Event handled, will render below
                                }
                                WidgetResult::Action(action) => {
                                    if let crate::ui::menu::MenuAction::ViceConnectAddress(addr) =
                                        action
                                    {
                                        if let Ok(client) = crate::vice::ViceClient::connect(
                                            &addr,
                                            event_tx.clone(),
                                        ) {
                                            app_state.vice_client = Some(client);
                                        } else {
                                            ui_state
                                                .set_status_message("Failed to connect to VICE");
                                        }
                                    } else if action == crate::ui::menu::MenuAction::ViceDisconnect
                                    {
                                        app_state.vice_client = None;
                                        app_state.vice_state.connected = false;
                                        ui_state.set_status_message("Disconnected from VICE");
                                    } else if action == crate::ui::menu::MenuAction::ViceStep {
                                        if let Some(client) = &app_state.vice_client {
                                            client.send_advance_instruction();
                                        }
                                    } else {
                                        crate::ui::menu::handle_menu_action(
                                            &mut app_state,
                                            &mut ui_state,
                                            action,
                                        );
                                    }
                                }
                                WidgetResult::Ignored => {
                                    // Try global input handler
                                    handle_global_input(key, &mut app_state, &mut ui_state);
                                }
                                WidgetResult::Close => {}
                            }
                        }

                        if ui_state.should_quit {
                            return Ok(());
                        }
                    }
                    Event::Mouse(mouse) => {
                        if !ui_state.dismiss_logo {
                            ui_state.dismiss_logo = true;
                            should_render = true;
                        }

                        // Handle Active Dialog (Modal) - Capture all mouse events
                        if let Some(mut dialog) = ui_state.active_dialog.take() {
                            // Check for Close Button Click (Top-Right [x])
                            // The [x] is at the top right of the dialog frame.
                            // We assume the [x] is roughly in the last 4 columns of the title bar row.
                            let area = ui_state.active_dialog_area;
                            let is_close_click = if mouse.kind
                                == event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                            {
                                mouse.row == area.y
                                    && mouse.column >= area.right().saturating_sub(4)
                                    && mouse.column < area.right()
                            } else {
                                false
                            };

                            if is_close_click {
                                // Dialog close requested via [x]
                                // We simply drop the dialog (don't put it back in ui_state)
                                // Fall through to render
                                should_render = true;
                            } else {
                                let result =
                                    dialog.handle_mouse(mouse, &mut app_state, &mut ui_state);
                                match result {
                                    crate::ui::widget::WidgetResult::Ignored => {
                                        ui_state.active_dialog = Some(dialog)
                                    }
                                    crate::ui::widget::WidgetResult::Handled => {
                                        ui_state.active_dialog = Some(dialog);
                                        should_render = true;
                                    }
                                    crate::ui::widget::WidgetResult::Close => {
                                        // Dialog closed.
                                        should_render = true;
                                    }
                                    crate::ui::widget::WidgetResult::Action(action) => {
                                        let is_vice_connect = matches!(
                                            action,
                                            crate::ui::menu::MenuAction::ViceConnectAddress(_)
                                        );
                                        if !is_vice_connect {
                                            ui_state.active_dialog = Some(dialog);
                                        }
                                        should_render = true;
                                        if let crate::ui::menu::MenuAction::ViceConnectAddress(
                                            addr,
                                        ) = action
                                        {
                                            if let Ok(client) = crate::vice::ViceClient::connect(
                                                &addr,
                                                event_tx.clone(),
                                            ) {
                                                app_state.vice_client = Some(client);
                                            } else {
                                                ui_state.set_status_message(
                                                    "Failed to connect to VICE",
                                                );
                                            }
                                        } else if action
                                            == crate::ui::menu::MenuAction::ViceDisconnect
                                        {
                                            app_state.vice_client = None;
                                            app_state.vice_state.connected = false;
                                            ui_state.set_status_message("Disconnected from VICE");
                                        } else if action == crate::ui::menu::MenuAction::ViceStep {
                                            if let Some(client) = &app_state.vice_client {
                                                client.send_advance_instruction();
                                            }
                                        } else {
                                            crate::ui::menu::handle_menu_action(
                                                &mut app_state,
                                                &mut ui_state,
                                                action,
                                            );
                                        }
                                    }
                                }
                                if ui_state.should_quit {
                                    return Ok(());
                                }
                            }
                        } else {
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
                                widget_result = crate::ui::menu::Menu.handle_mouse(
                                    mouse,
                                    &mut app_state,
                                    &mut ui_state,
                                );
                            } else if ui_state.menu.active {
                                // If menu is active and we clicked outside menu area.
                                // We let Menu handle it (it might detect click in popup).
                                widget_result = crate::ui::menu::Menu.handle_mouse(
                                    mouse,
                                    &mut app_state,
                                    &mut ui_state,
                                );

                                // If the menu ignored it (e.g. click outside both bar and popup), close the menu.
                                if matches!(widget_result, crate::ui::widget::WidgetResult::Ignored)
                                    && mouse.kind
                                        == event::MouseEventKind::Down(
                                            crossterm::event::MouseButton::Left,
                                        )
                                {
                                    ui_state.menu.active = false;
                                    ui_state.menu.selected_item = None;
                                    // Fallthrough to allow clicking on underlying view?
                                    // Usually click-away just closes menu.
                                    // Let's accept that closing the menu consumes the click to avoid accidental action on view.
                                    widget_result = crate::ui::widget::WidgetResult::Handled;
                                }
                            }

                            let prev_active_pane = ui_state.active_pane;

                            if widget_result == crate::ui::widget::WidgetResult::Ignored {
                                if is_inside(ui_state.disassembly_area, col, row) {
                                    ui_state.active_pane = ActivePane::Disassembly;
                                    use crate::ui::view_disassembly::DisassemblyView;
                                    widget_result = DisassemblyView.handle_mouse(
                                        mouse,
                                        &mut app_state,
                                        &mut ui_state,
                                    );
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
                                        crate::ui_state::RightPane::Debugger => {
                                            ui_state.active_pane = ActivePane::Debugger
                                        }
                                        _ => {}
                                    }

                                    use crate::ui::view_bitmap::BitmapView;
                                    use crate::ui::view_blocks::BlocksView;
                                    use crate::ui::view_charset::CharsetView;
                                    use crate::ui::view_debugger::DebuggerView;
                                    use crate::ui::view_disassembly::DisassemblyView;
                                    use crate::ui::view_hexdump::HexDumpView;
                                    use crate::ui::view_sprites::SpritesView;

                                    let mut active_view: Box<dyn Widget> =
                                        match ui_state.active_pane {
                                            ActivePane::Disassembly => Box::new(DisassemblyView),
                                            ActivePane::HexDump => Box::new(HexDumpView),
                                            ActivePane::Sprites => Box::new(SpritesView),
                                            ActivePane::Charset => Box::new(CharsetView),
                                            ActivePane::Bitmap => Box::new(BitmapView),
                                            ActivePane::Blocks => Box::new(BlocksView),
                                            ActivePane::Debugger => Box::new(DebuggerView),
                                        };
                                    widget_result = active_view.handle_mouse(
                                        mouse,
                                        &mut app_state,
                                        &mut ui_state,
                                    );
                                }
                            }

                            if ui_state.active_pane != prev_active_pane {
                                should_render = true;
                            }

                            if matches!(
                                widget_result,
                                crate::ui::widget::WidgetResult::Handled
                                    | crate::ui::widget::WidgetResult::Action(_)
                            ) {
                                should_render = true;
                            }

                            if let crate::ui::widget::WidgetResult::Action(action) = widget_result {
                                if let crate::ui::menu::MenuAction::ViceConnectAddress(addr) =
                                    action
                                {
                                    if let Ok(client) =
                                        crate::vice::ViceClient::connect(&addr, event_tx.clone())
                                    {
                                        client.send_registers_get(); // This will pause execution and yield the PC
                                        app_state.vice_client = Some(client);
                                    } else {
                                        ui_state.set_status_message("Failed to connect to VICE");
                                    }
                                } else if action == crate::ui::menu::MenuAction::ViceDisconnect {
                                    app_state.vice_client = None;
                                    app_state.vice_state.connected = false;
                                    ui_state.set_status_message("Disconnected from VICE");
                                } else if action == crate::ui::menu::MenuAction::ViceStep {
                                    if let Some(client) = &app_state.vice_client {
                                        client.send_advance_instruction();
                                    }
                                } else {
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
                        } // Close the else block for mouse dialog handling
                    }
                    _ => {}
                }
            } // End AppEvent::Crossterm
        } // End match event
        // Render AFTER event processing (only when something changed)
        if should_render {
            // Update sync state
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
                    let total_bitmaps =
                        (end_addr.saturating_sub(first_aligned_addr)).div_ceil(8192);

                    if idx < total_bitmaps {
                        ui_state.bitmap_cursor_index = idx;
                    }
                }
            }

            terminal
                .draw(|f| ui(f, &app_state, &mut ui_state))
                .map_err(|e| io::Error::other(e.to_string()))?;

            should_render = false;
        }
    }
}
