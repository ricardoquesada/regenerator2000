pub mod input;

use crate::ui::ui;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{self, Event, KeyCode, KeyEvent, MouseEvent};
use input::handle_global_input;
use ratatui::{Terminal, backend::Backend};
use regenerator2000_core::Core;
use regenerator2000_core::state::AppState;
use std::io;

pub enum AppEvent {
    Crossterm(Event),
    Mcp(crate::mcp::types::McpRequest),
    McpError(String),
    Vice(crate::vice::ViceEvent),
    Tick,
    UpdateAvailable(String),
}

/// Outcome returned by per-event-type handlers to tell the main loop what to do.
enum EventOutcome {
    /// Event was processed; render if `should_render` is true.
    Continue,
    /// The event was a non-press key repeat/release — skip to the next event.
    Skip,
    /// The user requested to quit.
    Quit,
}

pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut core: Core,
    mut ui_state: UIState,
    event_tx: std::sync::mpsc::Sender<AppEvent>,
    event_rx: std::sync::mpsc::Receiver<AppEvent>,
) -> io::Result<()> {
    // Sync Core's initial view state into TUI's UIState
    ui_state.core = core.view.clone();

    // Initial render before event loop
    terminal
        .draw(|f| ui(f, &core.state, &mut ui_state))
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
                    crate::mcp::handler::handle_request(&req, &mut core.state, &mut ui_state);
                let _ = req.response_sender.send(response);
                should_render = true;
            }
            AppEvent::McpError(err_msg) => {
                ui_state.set_status_message(format!("MCP Error: {err_msg}"));
                should_render = true;
            }
            AppEvent::Vice(vice_event) => {
                handle_vice_event(vice_event, &mut core.state, &mut ui_state);
                should_render = true;
            }
            AppEvent::Tick => {
                // Optional tick for animations/time-based updates if needed later
            }
            AppEvent::UpdateAvailable(version) => {
                ui_state.new_version_available = Some(version);
                should_render = true;
            }
            AppEvent::Crossterm(crossterm_event) => match crossterm_event {
                Event::Key(key) => {
                    match handle_key_event(
                        key,
                        &mut core,
                        &mut ui_state,
                        &event_tx,
                        &mut should_render,
                    ) {
                        EventOutcome::Quit => return Ok(()),
                        EventOutcome::Skip => continue,
                        EventOutcome::Continue => {}
                    }
                }
                Event::Mouse(mouse) => {
                    match handle_mouse_event(
                        mouse,
                        &mut core,
                        &mut ui_state,
                        &event_tx,
                        &mut should_render,
                    ) {
                        EventOutcome::Quit => return Ok(()),
                        EventOutcome::Continue | EventOutcome::Skip => {}
                    }
                }
                Event::Resize(width, height) => {
                    terminal
                        .resize(ratatui::layout::Rect::new(0, 0, width, height))
                        .map_err(|e| io::Error::other(e.to_string()))?;
                    should_render = true;
                }
                _ => {}
            },
        }

        // Render AFTER event processing (only when something changed)
        if should_render {
            sync_views_before_render(&core.state, &mut ui_state);

            terminal
                .draw(|f| ui(f, &core.state, &mut ui_state))
                .map_err(|e| io::Error::other(e.to_string()))?;

            // After render, sync BACK any TUI-driven view changes to Core
            ui_state.sync_tui_to_core();
            core.view = ui_state.core.clone();

            should_render = false;
        }
    }
}

// ---------------------------------------------------------------------------
// VICE protocol event handling
// ---------------------------------------------------------------------------

fn handle_vice_event(
    vice_event: crate::vice::ViceEvent,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) {
    match vice_event {
        crate::vice::ViceEvent::Connected => {
            app_state.vice_state.connected = true;
            ui_state.right_pane = crate::ui_state::RightPane::Debugger;
            ui_state.active_pane = ActivePane::Debugger;
            // Sync existing breakpoints from VICE (e.g. set in a previous session)
            if let Some(client) = &app_state.vice_client {
                client.send_checkpoint_list();
            }
            ui_state.set_status_message("Connected to VICE Monitor");
        }
        crate::vice::ViceEvent::Disconnected(reason) => {
            app_state.vice_state.connected = false;
            app_state.vice_state.reset_registers();
            ui_state.set_status_message(format!("Disconnected from VICE: {reason}"));
        }
        crate::vice::ViceEvent::Message(msg) => {
            handle_vice_message(&msg, app_state, ui_state);
        }
    }
}

fn handle_vice_message(
    msg: &crate::vice::ViceMessage,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) {
    if msg.command == crate::vice::ViceCommand::REGISTERS_GET {
        handle_vice_registers_get(msg, app_state, ui_state);
    } else if msg.command == crate::vice::ViceCommand::MEMORY_GET && msg.error_code == 0 {
        handle_vice_memory_get(msg, app_state);
    } else if (msg.command == crate::vice::ViceCommand::CHECKPOINT_SET
        || msg.command == crate::vice::ViceCommand::CHECKPOINT_GET)
        && msg.error_code == 0
    {
        handle_vice_checkpoint(msg, app_state);
    } else if msg.command == crate::vice::ViceCommand::CHECKPOINT_LIST && msg.error_code == 0 {
        // CHECKPOINT_LIST response: just a count (4 bytes).
        // VICE then sends individual CHECKPOINT_GET (0x11) responses for each
        // checkpoint — those are handled above. Clear the list here so the
        // incoming 0x11 responses repopulate it cleanly.
        app_state.vice_state.breakpoints.clear();
    } else if msg.command == crate::vice::ViceCommand::CHECKPOINT_DELETE && msg.error_code == 0 {
        let id = msg.request_id;
        app_state.vice_state.breakpoints.retain(|bp| bp.id != id);
    } else if msg.command == crate::vice::ViceCommand::RESUMED {
        app_state.vice_state.running = true;
        app_state.vice_state.stop_reason = None;
        app_state.vice_state.last_hit_checkpoint_id = None;
        ui_state.debugger_flash_remaining = 0;
    } else if msg.command == crate::vice::ViceCommand::STOPPED {
        app_state.vice_state.running = false;

        // Take a snapshot of the current state right before we start fetching new state,
        // so the debugger can highlight changed values.
        app_state.vice_state.previous = Some(app_state.vice_state.snapshot());

        // Start flash countdown for the debugger status line
        ui_state.debugger_flash_remaining = 8;

        if let Some(client) = &app_state.vice_client {
            // If we had any pending temporary breakpoints, delete them.
            // VICE auto-deletes temp breakpoints when they are hit, but it
            // does NOT delete them if the CPU stopped for another reason
            // (e.g. user manually paused, or another breakpoint was hit).
            // Deleting a breakpoint that was already auto-deleted is harmless.
            for id in &app_state.vice_state.temporary_breakpoints {
                client.send_checkpoint_delete(*id);
            }
            app_state.vice_state.temporary_breakpoints.clear();

            // CPU stopped (step complete, step-over complete, or checkpoint hit).
            // Fetch registers so the debugger panel and live view update.
            client.send_registers_get();
        }
    } else if msg.error_code != 0 {
        // Ignore error responses silently (e.g. memory not accessible)
    } else {
        // Silence known commands that need no further handling
        let silent = matches!(
            msg.command,
            crate::vice::ViceCommand::RESUMED | crate::vice::ViceCommand::EXIT_MONITOR
        );
        if !silent {
            ui_state.set_status_message(format!("VICE Msg: {:02x}", msg.command));
        }
    }
}

fn handle_vice_registers_get(
    msg: &crate::vice::ViceMessage,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) {
    let Some(regs) = crate::vice::parse_registers(&msg.payload) else {
        return;
    };

    app_state.vice_state.a = regs.a;
    app_state.vice_state.x = regs.x;
    app_state.vice_state.y = regs.y;
    app_state.vice_state.sp = regs.sp;
    app_state.vice_state.p = regs.p;

    let Some(pc) = regs.pc else {
        ui_state.set_status_message("VICE Registers: did not find PC".to_string());
        return;
    };

    app_state.vice_state.pc = Some(pc);
    ui_state.set_status_message(format!("VICE PC: ${pc:04X}"));

    // Jump static disassembly cursor + scroll to PC.
    // unwrap_or_else(|i| i-1) finds the containing instruction
    // when PC is mid-instruction or in a data region.
    if !app_state.disassembly.is_empty() {
        let idx = app_state
            .disassembly
            .binary_search_by_key(&pc, |l| l.address.0)
            .unwrap_or_else(|i| i.saturating_sub(1).min(app_state.disassembly.len() - 1));
        let sub_idx = crate::ui::view_disassembly::DisassemblyView::get_sub_index_for_address(
            &app_state.disassembly[idx],
            app_state,
            pc,
        );
        ui_state.cursor_index = idx;
        ui_state.sub_cursor_index = sub_idx;
        ui_state.scroll_index = idx;
        ui_state.scroll_sub_index = sub_idx;
    }

    // Build a descriptive stop_reason (if not already set by handle_vice_checkpoint
    // via the currently_hit flag in the checkpoint response).
    if app_state.vice_state.stop_reason.is_none() {
        let hit_id = app_state.vice_state.last_hit_checkpoint_id;
        app_state.vice_state.stop_reason = app_state
            .vice_state
            .breakpoints
            .iter()
            .find(|bp| {
                hit_id.is_some_and(|id| bp.id == id)
                    || (bp.kind == crate::vice::state::BreakpointKind::Exec && bp.address == pc)
            })
            .map(|bp| {
                if bp.kind == crate::vice::state::BreakpointKind::Exec {
                    format!("Breakpoint #{} at ${:04X}", bp.id, bp.address)
                } else {
                    format!(
                        "Watchpoint #{} at ${:04X} [{}]",
                        bp.id,
                        bp.address,
                        bp.kind.label()
                    )
                }
            });
    }
    app_state.vice_state.last_hit_checkpoint_id = None;

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

        let is_commodore = app_state.settings.platform == "Commodore 64"
            || app_state.settings.platform == "Commodore 128";
        if is_commodore {
            client.send_memory_get(0xD000, 0xDFFF, 3);
            client.send_memory_get(0x0000, 0x0001, 4);
        }

        client.send_memory_get(0xFFFA, 0xFFFF, 5);

        if let Some(dump_addr) = app_state.vice_state.dump_address {
            client.send_memory_get(dump_addr, dump_addr.saturating_add(63), 6);
        }

        app_state.vice_state.live_memory_start = mem_start;
    }
}

fn handle_vice_memory_get(msg: &crate::vice::ViceMessage, app_state: &mut AppState) {
    let Some(resp) = crate::vice::parse_memory_get(&msg.payload) else {
        return;
    };

    match msg.request_id {
        1 => {
            // Live disassembly window around PC
            app_state.vice_state.live_memory = Some(resp.bytes);
        }
        2 => {
            // Stack page response ($0100–$01FF)
            app_state.vice_state.stack_memory = Some(resp.bytes);
        }
        3 => {
            // I/O block snapshot ($D000–$DFFF)
            app_state.vice_state.io_memory = Some(resp.bytes);
        }
        4 => {
            app_state.vice_state.zp00_01 = Some(resp.bytes);
        }
        5 => {
            app_state.vice_state.vectors = Some(resp.bytes);
        }
        6 => {
            app_state.vice_state.dump_memory = Some(resp.bytes);
        }
        _ => {}
    }
}

fn handle_vice_checkpoint(msg: &crate::vice::ViceMessage, app_state: &mut AppState) {
    // CHECKPOINT_SET (0x12) and CHECKPOINT_GET (0x11) both carry the
    // same checkpoint_info body:
    //   CN(4) CH(1) SA(2) EA(2) ST(1) EN(1) OP(1) TM(1) HC(4) IC(4) CE(1) MS(1)
    //
    // VICE uses 0x11 for:
    //   - Individual responses after a CHECKPOINT_LIST request
    //   - CHECKPOINT_SET acknowledgment in some VICE versions
    //   - Unsolicited notification when a checkpoint is hit (currently_hit=1)
    // VICE uses 0x12 for:
    //   - CHECKPOINT_SET acknowledgment in other VICE versions
    // Handling both here makes us robust to all versions.
    let Some(info) = crate::vice::parse_checkpoint_info(&msg.payload) else {
        return;
    };

    let kind = crate::vice::state::BreakpointKind::from_cpu_op(info.cpu_op);

    // The `currently_hit` flag (byte 4) is set when this checkpoint caused the
    // CPU to stop. Set `stop_reason` immediately — this is especially important
    // for watchpoints where the PC differs from the watched address, so the
    // PC-based fallback in handle_vice_registers_get won't match.
    if info.currently_hit {
        app_state.vice_state.last_hit_checkpoint_id = Some(info.id);
        let reason = if kind == crate::vice::state::BreakpointKind::Exec {
            format!("Breakpoint #{} at ${:04X}", info.id, info.address)
        } else {
            format!(
                "Watchpoint #{} at ${:04X} [{}]",
                info.id,
                info.address,
                kind.label()
            )
        };
        app_state.vice_state.stop_reason = Some(reason);
    }

    if info.temporary {
        // It's a temporary breakpoint (e.g. Run To Cursor). Keep track of it
        // so we can delete it if the emulator stops prematurely.
        if !app_state
            .vice_state
            .temporary_breakpoints
            .contains(&info.id)
        {
            app_state.vice_state.temporary_breakpoints.push(info.id);
        }
    } else {
        // Avoid duplicates (e.g. if both 0x11 and 0x12 arrive for same checkpoint)
        if !app_state
            .vice_state
            .breakpoints
            .iter()
            .any(|bp| bp.id == info.id)
        {
            app_state
                .vice_state
                .breakpoints
                .push(crate::vice::state::ViceBreakpoint {
                    id: info.id,
                    address: info.address,
                    enabled: info.enabled,
                    kind,
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Keyboard event handling
// ---------------------------------------------------------------------------

fn handle_key_event(
    key: KeyEvent,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
    should_render: &mut bool,
) -> EventOutcome {
    if key.kind != event::KeyEventKind::Press {
        return EventOutcome::Skip;
    }

    if !ui_state.dismiss_logo {
        ui_state.dismiss_logo = true;
    }
    *should_render = true;

    // Handle Active Dialog (Generic)
    if let Some(mut dialog) = ui_state.active_dialog.take() {
        let result = dialog.handle_input(key, &mut core.state, ui_state);
        match result {
            crate::ui::widget::WidgetResult::Ignored => {
                ui_state.active_dialog = Some(dialog);
            }
            crate::ui::widget::WidgetResult::Handled => {
                ui_state.active_dialog = Some(dialog);
                *should_render = true;
            }
            crate::ui::widget::WidgetResult::Close => {
                // Dialog closed.
                *should_render = true;
            }
            crate::ui::widget::WidgetResult::Action(action) => {
                if !action.closes_dialog() {
                    ui_state.active_dialog = Some(dialog);
                }
                *should_render = true;
                dispatch_menu_action(action, core, ui_state, event_tx, false);
            }
        }
        if ui_state.should_quit {
            return EventOutcome::Quit;
        }
    } else if ui_state.menu.active {
        use crate::ui::widget::Widget;
        let result = crate::ui::menu::Menu.handle_input(key, &mut core.state, ui_state);
        if let crate::ui::widget::WidgetResult::Action(action) = result {
            dispatch_menu_action(action, core, ui_state, event_tx, false);
        }
    } else if ui_state.vim_search_active {
        match key.code {
            KeyCode::Esc => {
                ui_state.vim_search_active = false;
                ui_state.set_status_message("Ready");
            }
            KeyCode::Enter => {
                ui_state.last_search_query = ui_state.vim_search_input.clone();
                ui_state.vim_search_active = false;
                crate::ui::dialog_search::perform_search(&mut core.state, ui_state, true);
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
        handle_key_event_active_view(key, core, ui_state, event_tx);
    }

    if ui_state.should_quit {
        return EventOutcome::Quit;
    }

    EventOutcome::Continue
}

/// Delegate a key event to the currently active view/pane.
fn handle_key_event_active_view(
    key: KeyEvent,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
) {
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

    match active_view.handle_input(key, &mut core.state, ui_state) {
        WidgetResult::Handled => {
            // Event handled, will render below
        }
        WidgetResult::Action(action) => {
            dispatch_menu_action(action, core, ui_state, event_tx, false);
        }
        WidgetResult::Ignored => {
            // Try global input handler
            handle_global_input(key, core, ui_state);
        }
        WidgetResult::Close => {}
    }
}

// ---------------------------------------------------------------------------
// Mouse event handling
// ---------------------------------------------------------------------------

fn handle_mouse_event(
    mouse: MouseEvent,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
    should_render: &mut bool,
) -> EventOutcome {
    if !ui_state.dismiss_logo {
        ui_state.dismiss_logo = true;
        *should_render = true;
    }

    // Handle Active Dialog (Modal) - Capture all mouse events
    if let Some(dialog) = ui_state.active_dialog.take() {
        let outcome = handle_mouse_dialog(mouse, dialog, core, ui_state, event_tx, should_render);
        if ui_state.should_quit {
            return EventOutcome::Quit;
        }
        return outcome;
    }

    // No dialog active — route to menu bar or views
    handle_mouse_views(mouse, core, ui_state, event_tx, should_render);

    if ui_state.should_quit {
        return EventOutcome::Quit;
    }

    EventOutcome::Continue
}

/// Handle mouse events when a modal dialog is open.
fn handle_mouse_dialog(
    mouse: MouseEvent,
    mut dialog: Box<dyn crate::ui::widget::Widget>,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
    should_render: &mut bool,
) -> EventOutcome {
    // Check for Close Button Click (Top-Right [x])
    let area = ui_state.active_dialog_area;
    let is_close_click =
        if mouse.kind == event::MouseEventKind::Down(crossterm::event::MouseButton::Left) {
            mouse.row == area.y
                && mouse.column >= area.right().saturating_sub(4)
                && mouse.column < area.right()
        } else {
            false
        };

    if is_close_click {
        // Dialog close requested via [x] — drop the dialog
        *should_render = true;
        return EventOutcome::Continue;
    }

    let result = dialog.handle_mouse(mouse, &mut core.state, ui_state);
    match result {
        crate::ui::widget::WidgetResult::Ignored => {
            ui_state.active_dialog = Some(dialog);
        }
        crate::ui::widget::WidgetResult::Handled => {
            ui_state.active_dialog = Some(dialog);
            *should_render = true;
        }
        crate::ui::widget::WidgetResult::Close => {
            // Dialog closed.
            *should_render = true;
        }
        crate::ui::widget::WidgetResult::Action(action) => {
            if !action.closes_dialog() {
                ui_state.active_dialog = Some(dialog);
            }
            *should_render = true;
            dispatch_menu_action(action, core, ui_state, event_tx, false);
        }
    }

    EventOutcome::Continue
}

/// Handle mouse events against menu bar and view panes (no dialog active).
fn handle_mouse_views(
    mouse: MouseEvent,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
    should_render: &mut bool,
) {
    let col = mouse.column;
    let row = mouse.row;

    let mut widget_result = crate::ui::widget::WidgetResult::Ignored;
    let is_inside = |rect: ratatui::layout::Rect, col: u16, row: u16| {
        col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
    };

    use crate::ui::widget::Widget;

    if is_inside(ui_state.menu_area, col, row) {
        widget_result = crate::ui::menu::Menu.handle_mouse(mouse, &mut core.state, ui_state);
    } else if ui_state.menu.active {
        // If menu is active and we clicked outside menu area.
        // We let Menu handle it (it might detect click in popup).
        widget_result = crate::ui::menu::Menu.handle_mouse(mouse, &mut core.state, ui_state);

        // If the menu ignored it (e.g. click outside both bar and popup), close the menu.
        if matches!(widget_result, crate::ui::widget::WidgetResult::Ignored)
            && mouse.kind == event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        {
            ui_state.menu.active = false;
            ui_state.menu.selected_item = None;
            widget_result = crate::ui::widget::WidgetResult::Handled;
        }
    }

    let prev_active_pane = ui_state.active_pane;

    if widget_result == crate::ui::widget::WidgetResult::Ignored {
        if is_inside(ui_state.disassembly_area, col, row) {
            ui_state.active_pane = ActivePane::Disassembly;
            use crate::ui::view_disassembly::DisassemblyView;
            widget_result = DisassemblyView.handle_mouse(mouse, &mut core.state, ui_state);
        } else if is_inside(ui_state.right_pane_area, col, row) {
            match ui_state.right_pane {
                crate::ui_state::RightPane::HexDump => {
                    ui_state.active_pane = ActivePane::HexDump;
                }
                crate::ui_state::RightPane::Sprites => {
                    ui_state.active_pane = ActivePane::Sprites;
                }
                crate::ui_state::RightPane::Charset => {
                    ui_state.active_pane = ActivePane::Charset;
                }
                crate::ui_state::RightPane::Bitmap => {
                    ui_state.active_pane = ActivePane::Bitmap;
                }
                crate::ui_state::RightPane::Blocks => {
                    ui_state.active_pane = ActivePane::Blocks;
                }
                crate::ui_state::RightPane::Debugger => {
                    ui_state.active_pane = ActivePane::Debugger;
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

            let mut active_view: Box<dyn Widget> = match ui_state.active_pane {
                ActivePane::Disassembly => Box::new(DisassemblyView),
                ActivePane::HexDump => Box::new(HexDumpView),
                ActivePane::Sprites => Box::new(SpritesView),
                ActivePane::Charset => Box::new(CharsetView),
                ActivePane::Bitmap => Box::new(BitmapView),
                ActivePane::Blocks => Box::new(BlocksView),
                ActivePane::Debugger => Box::new(DebuggerView),
            };
            widget_result = active_view.handle_mouse(mouse, &mut core.state, ui_state);
        }
    }

    if ui_state.active_pane != prev_active_pane {
        *should_render = true;
    }

    if matches!(
        widget_result,
        crate::ui::widget::WidgetResult::Handled | crate::ui::widget::WidgetResult::Action(_)
    ) {
        *should_render = true;
    }

    if let crate::ui::widget::WidgetResult::Action(action) = widget_result {
        dispatch_menu_action(action, core, ui_state, event_tx, true);
    }
}

// ---------------------------------------------------------------------------
// Shared helper: dispatch a AppAction (de-duplicates ViceConnectAddress logic)
// ---------------------------------------------------------------------------

/// Route a `AppAction` to either the VICE connect flow or the generic handler.
///
/// When `send_registers_on_connect` is true (mouse-originated connections),
/// we immediately request registers after connecting so the debugger panel
/// populates.
fn dispatch_menu_action(
    action: crate::state::actions::AppAction,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
    send_registers_on_connect: bool,
) {
    if let crate::state::actions::AppAction::ViceConnectAddress(addr) = action {
        let (vice_tx, vice_rx) = std::sync::mpsc::channel();
        let app_tx = event_tx.clone();
        std::thread::spawn(move || {
            while let Ok(event) = vice_rx.recv() {
                if app_tx.send(AppEvent::Vice(event)).is_err() {
                    break;
                }
            }
        });
        if let Ok(client) = crate::vice::ViceClient::connect(&addr, vice_tx) {
            if send_registers_on_connect {
                client.send_registers_get();
            }
            core.state.vice_client = Some(client);
        } else {
            ui_state.set_status_message("Failed to connect to VICE");
        }
    } else {
        crate::ui::menu::handle_menu_action(core, ui_state, action);
    }
}

// ---------------------------------------------------------------------------
// View synchronization (runs before each render)
// ---------------------------------------------------------------------------

/// Sync right-pane views with the disassembly cursor position and update
/// menu availability. Called once before each render.
fn sync_views_before_render(app_state: &AppState, ui_state: &mut UIState) {
    // Update menu availability
    ui_state.menu.update_availability(
        app_state,
        ui_state.cursor_index,
        ui_state.last_search_query.is_empty(),
        ui_state.active_pane,
    );

    // All sync logic only applies when the disassembly pane is active
    if ui_state.active_pane != ActivePane::Disassembly {
        return;
    }

    let Some(line) = app_state.disassembly.get(ui_state.cursor_index) else {
        return;
    };
    let target_addr = line.address;

    // Sync Blocks view
    if ui_state.right_pane == crate::ui_state::RightPane::Blocks
        && app_state.system_config.sync_blocks_view
        && let Some(idx) = app_state.get_block_index_for_address(target_addr)
    {
        ui_state.blocks_list_state.select(Some(idx));
    }

    // Sync HexDump view
    if ui_state.right_pane == crate::ui_state::RightPane::HexDump
        && app_state.system_config.sync_hex_dump
    {
        let origin = app_state.origin.0 as usize;
        let alignment_padding = origin % 16;
        let aligned_origin = origin - alignment_padding;
        let addr = target_addr.0 as usize;

        if addr >= aligned_origin {
            let offset = addr - aligned_origin;
            let row = offset / 16;
            let bytes_per_row = 16;
            let total_len = app_state.raw_data.len() + alignment_padding;
            let max_rows = total_len.div_ceil(bytes_per_row);
            if row < max_rows {
                ui_state.hex_cursor_index = row;
            }
        }
    }

    // Sync Charset view
    if ui_state.right_pane == crate::ui_state::RightPane::Charset
        && app_state.system_config.sync_charset_view
    {
        let origin = app_state.origin.0 as usize;
        let base_alignment = 0x400;
        let aligned_start_addr = (origin / base_alignment) * base_alignment;
        let addr = target_addr.0 as usize;

        if addr >= aligned_start_addr {
            let char_offset = addr - aligned_start_addr;
            let idx = char_offset / 8;

            let end_addr = origin + app_state.raw_data.len();
            let total_chars = (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

            if idx < total_chars {
                ui_state.charset_cursor_index = idx;
            }
        }
    }

    // Sync Sprites view
    if ui_state.right_pane == crate::ui_state::RightPane::Sprites
        && app_state.system_config.sync_sprites_view
    {
        let origin = app_state.origin.0 as usize;
        let aligned_origin = (origin / 64) * 64;
        let addr = target_addr.0 as usize;

        if addr >= aligned_origin {
            let offset = addr - aligned_origin;
            let idx = offset / 64;

            let data_len = app_state.raw_data.len();
            let end_addr = origin + data_len;
            let total_sprites = (end_addr.saturating_sub(aligned_origin)).div_ceil(64);

            if idx < total_sprites {
                ui_state.sprites_cursor_index = idx;
            }
        }
    }

    // Sync Bitmap view
    if ui_state.right_pane == crate::ui_state::RightPane::Bitmap
        && app_state.system_config.sync_bitmap_view
    {
        let origin = app_state.origin.0 as usize;
        let addr = target_addr.0 as usize;

        // Bitmaps must be aligned to 8192-byte ($2000) boundaries
        let first_aligned_addr = (origin / 8192) * 8192;

        if addr >= first_aligned_addr {
            let offset = addr - first_aligned_addr;
            let idx = offset / 8192;

            let data_len = app_state.raw_data.len();
            let end_addr = origin + data_len;
            let total_bitmaps = (end_addr.saturating_sub(first_aligned_addr)).div_ceil(8192);

            if idx < total_bitmaps {
                ui_state.bitmap_cursor_index = idx;
            }
        }
    }
}
