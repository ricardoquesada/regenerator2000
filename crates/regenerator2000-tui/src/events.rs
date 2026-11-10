pub mod input;

use crate::ui::ui;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{self, Event, KeyCode, KeyEvent, MouseEvent};
use input::handle_global_input;
use ratatui::{Terminal, backend::Backend};
use regenerator2000_core::Core;
use regenerator2000_core::state::AppState;
use regenerator2000_core::state::types::System;
use std::io;

pub enum AppEvent {
    Crossterm(Event),
    Mcp(crate::mcp::types::McpRequest),
    McpError(String),
    McpStartRequested,
    McpStopRequested,
    McpServerStarted {
        port: u16,
    },
    McpServerStopped,
    Vice(crate::vice::ViceEvent),
    Tick,
    UpdateAvailable(String),
    /// Progress update from the background unpacker thread (instruction count).
    UnpackProgress(u64),
    /// The background unpacker thread finished.
    UnpackComplete(
        Result<
            regenerator2000_core::unpacker::UnpackResult,
            regenerator2000_core::unpacker::UnpackError,
        >,
    ),
}

/// Tells the main loop how to proceed after an event handler runs.
enum EventOutcome {
    /// Continue the loop and render when `should_render` is true.
    Continue,
    /// Skip non-press key repeat or release events.
    Skip,
    /// Quit the application.
    Quit,
}

/// Runs the main application event loop.
///
/// # Errors
/// Returns an error if terminal rendering or resizing operations fail.
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
    let mut mcp_shutdown_tx: Option<tokio::sync::oneshot::Sender<()>> = None;

    loop {
        // Wait for event (blocking)
        let event = match event_rx.recv() {
            Ok(e) => e,
            Err(_) => break, // Channel closed
        };

        match event {
            AppEvent::Mcp(req) => {
                let response =
                    crate::mcp::handler::handle_request(&req, &mut core.state, &mut ui_state);
                let _ = req.response_sender.send(response);
                should_render = true;
            }
            AppEvent::McpStartRequested => {
                if mcp_shutdown_tx.is_some() || core.state.mcp_server_running {
                    ui_state.set_status_message("MCP Server is already running.".to_string());
                } else {
                    ui_state.set_status_message("Starting MCP Server...".to_string());
                    mcp_shutdown_tx = spawn_mcp_server_thread(&event_tx);
                }
                should_render = true;
            }
            AppEvent::McpStopRequested => {
                core.state.mcp_server_running = false;
                if let Some(tx) = mcp_shutdown_tx.take() {
                    let _ = tx.send(());
                    ui_state.set_status_message("Stopping MCP Server...".to_string());
                } else {
                    ui_state.set_status_message("MCP Server is not running.".to_string());
                }
                should_render = true;
            }
            AppEvent::McpServerStarted { port } => {
                core.state.mcp_server_running = true;
                ui_state.set_status_message(format!(
                    "MCP Server active on http://127.0.0.1:{port}/mcp"
                ));
                should_render = true;
            }
            AppEvent::McpServerStopped => {
                mcp_shutdown_tx = None;
                core.state.mcp_server_running = false;
                ui_state.set_status_message("MCP Server stopped.".to_string());
                should_render = true;
            }
            AppEvent::McpError(err_msg) => {
                mcp_shutdown_tx = None;
                core.state.mcp_server_running = false;
                ui_state.set_status_message(format!("MCP Error: {err_msg}"));
                should_render = true;
            }
            AppEvent::Vice(vice_event) => {
                handle_vice_event(vice_event, &mut core.state, &mut ui_state);
                should_render = true;
            }
            AppEvent::Tick => {
                // Drive time-based animations (e.g. easter egg typing effect and debugger stop flash).
                if ui_state.debugger_flash_remaining > 0 {
                    ui_state.debugger_flash_remaining -= 1;
                }
                should_render = true;
                // Give the active dialog a chance to auto-close on a timer.
                if let Some(mut dialog) = ui_state.active_dialog.take() {
                    let result = dialog.handle_tick(&mut core.state, &mut ui_state);
                    if matches!(result, crate::ui::widget::WidgetResult::Close) {
                        // Dialog requested close — leave active_dialog as None and promote next queued dialog.
                        ui_state.promote_next_dialog();
                    } else {
                        ui_state.active_dialog = Some(dialog);
                    }
                }
            }
            AppEvent::UpdateAvailable(version) => {
                ui_state.new_version_available = Some(version);
                should_render = true;
            }
            AppEvent::UnpackProgress(count) => {
                ui_state.set_status_message(format!("Unpacking... ${count:08X}"));
                should_render = true;
            }
            AppEvent::UnpackComplete(result) => {
                handle_unpack_complete(result, &mut core, &mut ui_state);
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
                        EventOutcome::Quit => break,
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
                        EventOutcome::Quit => break,
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

    if let Some(tx) = mcp_shutdown_tx.take() {
        let _ = tx.send(());
    }

    Ok(())
}

/// Spawns the MCP HTTP server thread on default port `3000` and returns a shutdown sender handle.
#[must_use]
pub fn spawn_mcp_server_thread(
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
) -> Option<tokio::sync::oneshot::Sender<()>> {
    spawn_mcp_server_thread_on_port(event_tx, 3000)
}

/// Spawns the MCP HTTP server thread on `port` and returns a shutdown sender handle.
///
/// The background thread binds `127.0.0.1:{port}` before it emits [`AppEvent::McpServerStarted`].
/// If binding fails, the thread emits [`AppEvent::McpError`] and exits without [`AppEvent::McpServerStopped`].
/// This keeps the bind error visible in the TUI status bar.
#[must_use]
pub fn spawn_mcp_server_thread_on_port(
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
    port: u16,
) -> Option<tokio::sync::oneshot::Sender<()>> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (mcp_req_tx, mut mcp_req_rx) = tokio::sync::mpsc::channel(100);
    let mcp_bridge_tx = event_tx.clone();
    let started_tx = event_tx.clone();
    let stopped_tx = event_tx.clone();

    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Runtime::new() else {
            let _ = mcp_bridge_tx.send(AppEvent::McpError(
                "Failed to initialize Tokio runtime for MCP server".to_string(),
            ));
            return;
        };
        rt.block_on(async move {
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    log::error!("Failed to bind MCP HTTP server on 127.0.0.1:{port}: {e}");
                    let _ = mcp_bridge_tx.send(AppEvent::McpError(format!(
                        "Failed to bind 127.0.0.1:{port}: {e}"
                    )));
                    return;
                }
            };

            let bound_port = listener.local_addr().map_or(port, |a| a.port());
            let server_handle = tokio::spawn(async move {
                regenerator2000_core::mcp::http::serve_listener_with_shutdown(
                    listener,
                    mcp_req_tx,
                    shutdown_rx,
                )
                .await
            });

            let _ = started_tx.send(AppEvent::McpServerStarted { port: bound_port });

            while let Some(req) = mcp_req_rx.recv().await {
                if mcp_bridge_tx.send(AppEvent::Mcp(req)).is_err() {
                    break;
                }
            }

            match server_handle.await {
                Ok(Err(e)) => {
                    log::error!("MCP HTTP server error on 127.0.0.1:{bound_port}: {e}");
                    let _ = stopped_tx.send(AppEvent::McpError(e.to_string()));
                }
                Ok(Ok(())) | Err(_) => {
                    let _ = stopped_tx.send(AppEvent::McpServerStopped);
                }
            }
        });
    });

    Some(shutdown_tx)
}

// ---------------------------------------------------------------------------
// Unpack completion handling
// ---------------------------------------------------------------------------

/// Processes the result of a background unpack operation.
///
/// On success, loads the unpacked binary into the application state, updates
/// the status bar with a summary, and opens the import context dialog.
/// On failure, displays the error in the status bar.
pub fn handle_unpack_complete(
    result: Result<
        regenerator2000_core::unpacker::UnpackResult,
        regenerator2000_core::unpacker::UnpackError,
    >,
    core: &mut Core,
    ui_state: &mut UIState,
) {
    match result {
        Ok(result) => {
            ui_state.bitmap_cache.clear();
            ui_state.bitmap_protocol = None;
            let outcome = core.state.apply_unpack_result(&result);
            regenerator2000_core::navigation::perform_jump_to_address_no_history(
                &core.state,
                &mut core.view,
                outcome.entry_point,
            );
            ui_state.core = core.view.clone();
            ui_state.sync_core_to_tui();
            ui_state.set_status_message(outcome.summary);
            // Show import context dialog for system/origin/entry setup
            ui_state.push_dialog(Box::new(
                crate::ui::dialog_import_context::ImportContextDialog::new(
                    core.state.settings.system.as_str(),
                    outcome.start_addr,
                    Some(outcome.entry_point),
                    None,
                    None,
                    Option::<String>::None,
                ),
            ));
        }
        Err(e) => {
            ui_state.set_status_message(format!("Unpack failed: {e}"));
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
        let was_running = app_state.vice_state.running;
        app_state.vice_state.running = false;

        // Start flash countdown for the debugger status line
        ui_state.debugger_flash_remaining = 8;

        if let Some(client) = &app_state.vice_client
            && was_running
        {
            // If we had any pending temporary breakpoints, delete them.
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

    // Jump static disassembly cursor + scroll to PC when PC lies inside the loaded binary.
    let pc_addr = crate::state::Addr(pc);
    if !app_state.is_external(pc_addr)
        && let Some(mut idx) = app_state
            .get_line_index_containing_address(pc_addr)
            .or_else(|| app_state.get_line_index_for_address(pc_addr))
    {
        while idx + 1 < app_state.disassembly.len()
            && app_state.disassembly[idx].address == pc_addr
            && app_state.disassembly[idx].bytes.is_empty()
            && app_state.disassembly[idx + 1].address == pc_addr
        {
            idx += 1;
        }
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
            .map(|bp| format_checkpoint_stop_reason(bp.id, bp.address, bp.kind));
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

        if app_state.settings.system == System::C64 || app_state.settings.system == System::C128 {
            client.send_memory_get(0xD000, 0xDFFF, 3);
            client.send_memory_get(0x0000, 0x0001, 4);
        } else if app_state.settings.system == System::Vic20 {
            // VIC-I registers: $9000–$900F
            client.send_memory_get(0x9000, 0x900F, 3);
        } else if app_state.settings.system == System::Plus4 {
            // TED chip registers: $FF00–$FF3F
            client.send_memory_get(0xFF00, 0xFF3F, 3);
        }

        client.send_memory_get(0xFFFA, 0xFFFF, 5);

        if let Some(dump_addr) = app_state.vice_state.dump_address {
            client.send_memory_get(dump_addr, dump_addr.saturating_add(63), 6);
        }

        app_state.vice_state.live_memory_start = mem_start;
    }
}

fn format_checkpoint_stop_reason(
    id: u32,
    address: u16,
    kind: crate::vice::state::BreakpointKind,
) -> String {
    if kind == crate::vice::state::BreakpointKind::Exec {
        format!("Breakpoint #{id} at ${address:04X}")
    } else {
        format!("Watchpoint #{id} at ${address:04X} [{}]", kind.label())
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

    // Byte 4 (`currently_hit`) indicates that this checkpoint stopped the CPU.
    // Set `stop_reason` immediately so watchpoints where PC != watched address match.
    if info.currently_hit {
        app_state.vice_state.last_hit_checkpoint_id = Some(info.id);
        app_state.vice_state.stop_reason =
            Some(format_checkpoint_stop_reason(info.id, info.address, kind));
    }

    if info.temporary {
        // Track temporary breakpoints (such as Run To Cursor) so we can delete them on stop.
        if !app_state
            .vice_state
            .temporary_breakpoints
            .contains(&info.id)
        {
            app_state.vice_state.temporary_breakpoints.push(info.id);
        }
    } else {
        // Avoid duplicates when both 0x11 and 0x12 arrive for the same checkpoint.
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

/// Processes one [`KeyEvent`] synchronously through the full TUI input-routing pipeline.
///
/// This function routes input through dialogs, the menu bar, inline Vim search, active views,
/// `Navigable` handlers, and global shortcuts.
/// It then runs `sync_views_before_render` and [`UIState::sync_tui_to_core`].
/// Integration tests call this headless entrypoint to verify keyboard shortcuts without a terminal backend.
pub fn process_key_event(core: &mut Core, ui_state: &mut UIState, key: KeyEvent) {
    let (event_tx, _event_rx) = std::sync::mpsc::channel();
    let mut should_render = false;
    let _ = handle_key_event(key, core, ui_state, &event_tx, &mut should_render);
    if should_render {
        sync_views_before_render(&core.state, ui_state);
        ui_state.sync_tui_to_core();
        core.view = ui_state.core.clone();
    }
}

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
        apply_dialog_result(dialog, result, core, ui_state, event_tx, should_render);
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
                // Commit the inline status-bar Vim search query (`'/'`) to `last_search_query`
                // and delegate execution to `Core` via `AppAction::FindNext` so inline search
                // and modal `Ctrl+F` search share the single `navigation_handler::perform_search`
                // implementation (including collapsed-block matching).
                ui_state.last_search_query = ui_state.vim_search_input.clone();
                ui_state.vim_search_active = false;
                dispatch_menu_action(
                    crate::state::actions::AppAction::FindNext,
                    core,
                    ui_state,
                    event_tx,
                    false,
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
        handle_key_event_active_view(key, core, ui_state, event_tx);
    }

    if ui_state.should_quit {
        return EventOutcome::Quit;
    }

    EventOutcome::Continue
}

/// Applies the [`crate::ui::widget::WidgetResult`] returned by an active modal dialog,
/// restoring or closing the dialog and promoting any queued dialog when it closes.
fn apply_dialog_result(
    dialog: Box<dyn crate::ui::widget::Widget>,
    result: crate::ui::widget::WidgetResult,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
    should_render: &mut bool,
) {
    match result {
        crate::ui::widget::WidgetResult::Ignored => {
            ui_state.active_dialog = Some(dialog);
        }
        crate::ui::widget::WidgetResult::Handled => {
            ui_state.active_dialog = Some(dialog);
            *should_render = true;
        }
        crate::ui::widget::WidgetResult::Close => {
            *should_render = true;
            ui_state.promote_next_dialog();
        }
        crate::ui::widget::WidgetResult::Action(action) => {
            if !action.closes_dialog() {
                ui_state.active_dialog = Some(dialog);
            }
            *should_render = true;
            dispatch_menu_action(action, core, ui_state, event_tx, false);
            ui_state.promote_next_dialog();
        }
    }
}

/// Returns a boxed [`crate::ui::widget::Widget`] for `pane`.
fn active_pane_widget(pane: ActivePane) -> Box<dyn crate::ui::widget::Widget> {
    use crate::ui::view_bitmap::BitmapView;
    use crate::ui::view_blocks::BlocksView;
    use crate::ui::view_charset::CharsetView;
    use crate::ui::view_debugger::DebuggerView;
    use crate::ui::view_disassembly::DisassemblyView;
    use crate::ui::view_hexdump::HexDumpView;
    use crate::ui::view_sprites::SpritesView;

    match pane {
        ActivePane::Disassembly => Box::new(DisassemblyView),
        ActivePane::HexDump => Box::new(HexDumpView),
        ActivePane::Sprites => Box::new(SpritesView),
        ActivePane::Charset => Box::new(CharsetView),
        ActivePane::Bitmap => Box::new(BitmapView),
        ActivePane::Blocks => Box::new(BlocksView),
        ActivePane::Debugger => Box::new(DebuggerView),
    }
}

/// Delegates a key event to the active view or pane.
fn handle_key_event_active_view(
    key: KeyEvent,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
) {
    use crate::ui::widget::WidgetResult;

    let mut active_view = active_pane_widget(ui_state.active_pane);

    match active_view.handle_input(key, &mut core.state, ui_state) {
        WidgetResult::Handled => {
            // Event handled, will render below
        }
        WidgetResult::Action(action) => {
            dispatch_menu_action(action, core, ui_state, event_tx, false);
        }
        WidgetResult::Ignored => {
            // Try global input handler
            handle_global_input(key, core, ui_state, event_tx);
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

/// Handles mouse events when a modal dialog is open.
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
        // Dialog close requested via [x] — drop the dialog and promote any queued dialog.
        *should_render = true;
        ui_state.promote_next_dialog();
        return EventOutcome::Continue;
    }

    let result = dialog.handle_mouse(mouse, &mut core.state, ui_state);
    apply_dialog_result(dialog, result, core, ui_state, event_tx, should_render);

    EventOutcome::Continue
}

/// Handles mouse events against the menu bar and view panes when no dialog is active.
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
        if is_inside(ui_state.minimap_area, col, row) {
            use crate::ui::minimap_bar::MinimapBar;
            widget_result = MinimapBar.handle_mouse(mouse, &mut core.state, ui_state);
        } else if is_inside(ui_state.disassembly_area, col, row) {
            ui_state.active_pane = ActivePane::Disassembly;
            use crate::ui::view_disassembly::DisassemblyView;
            widget_result = DisassemblyView.handle_mouse(mouse, &mut core.state, ui_state);
        } else if is_inside(ui_state.right_pane_area, col, row) {
            match ui_state.right_pane {
                crate::ui_state::RightPane::HexDump16 | crate::ui_state::RightPane::HexDump8 => {
                    ui_state.active_pane = ActivePane::HexDump;
                }
                crate::ui_state::RightPane::Sprites2Col
                | crate::ui_state::RightPane::Sprites1Col => {
                    ui_state.active_pane = ActivePane::Sprites;
                }
                crate::ui_state::RightPane::Charset8Col
                | crate::ui_state::RightPane::Charset4Col => {
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

            let mut active_view = active_pane_widget(ui_state.active_pane);
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

/// Routes an `AppAction` to the VICE connect flow or the generic menu handler.
///
/// When `send_registers_on_connect` is true, this function requests VICE registers
/// immediately after connection so the debugger panel populates.
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
            // Snapshot empty state so the first update appears as changed (yellow)
            core.state.vice_state.previous = Some(core.state.vice_state.snapshot());

            if send_registers_on_connect {
                client.send_registers_get();
            }
            core.state.vice_client = Some(client);
        } else {
            ui_state.set_status_message("Failed to connect to VICE");
        }
    } else {
        if matches!(action, crate::state::actions::AppAction::ApplyOrigin(_)) {
            ui_state.bitmap_cache.clear();
            ui_state.bitmap_protocol = None;
        }
        crate::ui::menu::handle_menu_action(core, ui_state, action, event_tx);
    }
}

// ---------------------------------------------------------------------------
// View synchronization (runs before each render)
// ---------------------------------------------------------------------------

/// Maps an active disassembly line selection to clamped grid item indices `(start, end)`.
fn mirror_disassembly_selection_to_grid(
    app_state: &AppState,
    selection_start: Option<usize>,
    cursor_index: usize,
    target_addr: crate::state::Addr,
    grid: &crate::ui::navigable::AlignedGridRange,
) -> Option<(usize, usize)> {
    let sel_start = selection_start?;
    let (s_idx, e_idx) = if sel_start <= cursor_index {
        (sel_start, cursor_index)
    } else {
        (cursor_index, sel_start)
    };
    let sel_start_addr = app_state
        .disassembly
        .get(s_idx)
        .map_or(target_addr.0 as usize, |l| l.address.0 as usize);
    let sel_end_addr = app_state
        .disassembly
        .get(e_idx)
        .map_or(target_addr.0 as usize, |l| {
            l.address.0 as usize + l.bytes.len().saturating_sub(1)
        });

    let start_item = sel_start_addr.checked_sub(grid.aligned_origin)? / grid.stride;
    let end_item = sel_end_addr.checked_sub(grid.aligned_origin)? / grid.stride;
    let max_idx = grid.item_count().saturating_sub(1);
    Some((start_item.min(max_idx), end_item.min(max_idx)))
}

/// Syncs right-pane views with the disassembly cursor position and updates menu availability.
fn sync_views_before_render(app_state: &AppState, ui_state: &mut UIState) {
    use crate::ui::navigable::AlignedGridRange;

    ui_state.cached_blocks_items = None;

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

    // Sync Blocks view and retain `items` for `BlocksView::render` in the same frame.
    if ui_state.right_pane == crate::ui_state::RightPane::Blocks
        && app_state.system_config.sync_blocks_view
    {
        let items = app_state.get_blocks_view_items();
        if let Some(idx) =
            AppState::find_block_index_in_items(&items, target_addr, line.is_splitter())
        {
            ui_state.blocks_list_state.select(Some(idx));
        }
        ui_state.cached_blocks_items = Some(items);
    }

    // Sync HexDump view
    if matches!(
        ui_state.right_pane,
        crate::ui_state::RightPane::HexDump16 | crate::ui_state::RightPane::HexDump8
    ) && app_state.system_config.sync_hex_dump
    {
        let bytes_per_row: usize = match ui_state.right_pane {
            crate::ui_state::RightPane::HexDump8 => 8,
            _ => 16,
        };
        let grid = AlignedGridRange::uniform(app_state, bytes_per_row);
        if let Some(row) = grid.index_for_addr(target_addr.0 as usize) {
            ui_state.hex_cursor_index = row;
        }
    }

    // Sync Charset view
    if matches!(
        ui_state.right_pane,
        crate::ui_state::RightPane::Charset8Col | crate::ui_state::RightPane::Charset4Col
    ) && app_state.system_config.sync_charset_view
    {
        let grid = AlignedGridRange::new(app_state, 0x400, 8, 8);
        if let Some(idx) = grid.index_for_addr(target_addr.0 as usize) {
            ui_state.charset_cursor_index = idx;
        }

        if let Some((start_char, end_char)) = mirror_disassembly_selection_to_grid(
            app_state,
            ui_state.selection_start,
            ui_state.cursor_index,
            target_addr,
            &grid,
        ) {
            ui_state.charset_selection_start = Some(start_char);
            ui_state.charset_cursor_index = end_char;
        } else if ui_state.selection_start.is_none() {
            ui_state.charset_selection_start = None;
        }
    }

    // Sync Sprites view
    if matches!(
        ui_state.right_pane,
        crate::ui_state::RightPane::Sprites2Col | crate::ui_state::RightPane::Sprites1Col
    ) && app_state.system_config.sync_sprites_view
    {
        let grid = AlignedGridRange::new(app_state, 64, 64, 63);
        if let Some(idx) = grid.index_for_addr(target_addr.0 as usize) {
            ui_state.sprites_cursor_index = idx;
        }

        if let Some((start_sprite, end_sprite)) = mirror_disassembly_selection_to_grid(
            app_state,
            ui_state.selection_start,
            ui_state.cursor_index,
            target_addr,
            &grid,
        ) {
            ui_state.sprites_selection_start = Some(start_sprite);
            ui_state.sprites_cursor_index = end_sprite;
        } else if ui_state.selection_start.is_none() {
            ui_state.sprites_selection_start = None;
        }
    }

    // Sync Bitmap view
    if ui_state.right_pane == crate::ui_state::RightPane::Bitmap
        && app_state.system_config.sync_bitmap_view
    {
        let grid = AlignedGridRange::new(app_state, 8192, 8192, 8000);
        if let Some(idx) = grid.index_for_addr(target_addr.0 as usize) {
            ui_state.bitmap_cursor_index = idx;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regenerator2000_core::unpacker::{BoundSource, ExitReason, UnpackReport, UnpackResult};

    #[test]
    fn status_line_shows_packer_and_exit_reason() {
        let mut core = Core::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        let res = UnpackResult {
            data: vec![0xA9, 0x01, 0x60],
            start_addr: 0x080D,
            end_addr: 0x080F,
            entry_point: 0x080D,
            dep_addr: 0x0334,
            instructions_executed: 42,
            packer: Some("byte_boozer2"),
            packer_name: Some("ByteBoozer 2.0".to_string()),
            report: UnpackReport {
                exit_reason: ExitReason::WrittenCodeReached { pc: 0x080D },
                find_depacker_instructions: 10,
                decompress_instructions: 32,
                start_source: BoundSource::WriteDiff,
                end_source: BoundSource::PointerAfterRun { addr: 0x00AE },
                entry_source: BoundSource::ExitPc,
                trace_tail: Vec::new(),
            },
        };

        handle_unpack_complete(Ok(res), &mut core, &mut ui_state);

        let status = ui_state
            .status_message
            .as_deref()
            .expect("status_message must be set");
        assert!(
            status.contains("ByteBoozer 2.0"),
            "status message missing packer name: {status}"
        );
        assert!(
            status.contains("written_code($080D)"),
            "status message missing exit reason: {status}"
        );
    }

    #[test]
    fn mcp_server_bind_failure_emits_only_mcp_error_without_clobbering() {
        // Hold an ephemeral port so `spawn_mcp_server_thread_on_port` fails with `Address already in use`.
        let occupied =
            std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port for test");
        let port = occupied.local_addr().expect("local_addr").port();

        let (event_tx, event_rx) = std::sync::mpsc::channel::<AppEvent>();
        let _shutdown_tx = spawn_mcp_server_thread_on_port(&event_tx, port);
        drop(event_tx);

        let mut events = Vec::new();
        while let Ok(ev) = event_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            events.push(ev);
        }

        assert_eq!(
            events.len(),
            1,
            "Expected exactly 1 event (McpError) on port bind conflict, got {}",
            events.len()
        );
        match &events[0] {
            AppEvent::McpError(msg) => {
                assert!(
                    msg.contains(&format!("127.0.0.1:{port}")),
                    "Expected error message to mention port {port}, got: {msg}"
                );
            }
            _ => panic!("Expected AppEvent::McpError on port bind conflict"),
        }
    }

    #[test]
    fn mcp_server_clean_start_and_stop_lifecycle() {
        let (event_tx, event_rx) = std::sync::mpsc::channel::<AppEvent>();
        let shutdown_tx =
            spawn_mcp_server_thread_on_port(&event_tx, 0).expect("shutdown_tx returned");
        drop(event_tx);

        let first = event_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("Expected McpServerStarted event");
        match first {
            AppEvent::McpServerStarted { port } => {
                assert!(port > 0, "Expected non-zero bound ephemeral port");
            }
            _ => panic!("Expected AppEvent::McpServerStarted as first event"),
        }

        shutdown_tx.send(()).expect("send shutdown signal");

        let second = event_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("Expected McpServerStopped event");
        assert!(
            matches!(second, AppEvent::McpServerStopped),
            "Expected AppEvent::McpServerStopped after shutdown"
        );
    }

    #[test]
    fn vice_registers_get_locates_pc_with_external_labels_and_ignores_external_rom_pc() {
        use regenerator2000_core::state::{Addr, BlockType, Label, LabelKind, LabelType};

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0800);
        // $0800: LDA #$01 (2 bytes), $0802: JSR $FFD2 (3 bytes), $0805: RTS (1 byte)
        app_state.raw_data = vec![0xA9, 0x01, 0x20, 0xD2, 0xFF, 0x60];
        app_state.block_types = vec![BlockType::Code; app_state.raw_data.len()];
        app_state.labels.insert(
            Addr(0xFFD2),
            vec![Label {
                name: "CHROUT".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ExternalJump,
            }],
        );
        app_state.settings.all_labels = true;
        app_state.disassemble();

        let mut ui_state = UIState::new(crate::theme::Theme::default());
        let build_pc_msg = |pc: u16| {
            let mut payload = Vec::new();
            payload.extend_from_slice(&1u16.to_le_bytes());
            payload.extend_from_slice(&[3, 0x03]);
            payload.extend_from_slice(&pc.to_le_bytes());
            crate::vice::ViceMessage {
                command: crate::vice::ViceCommand::REGISTERS_GET,
                error_code: 0,
                request_id: 1,
                payload,
            }
        };

        // 1. In-range PC ($0802) must land on the JSR $FFD2 line despite prepended external labels.
        let expected_idx = app_state
            .get_line_index_containing_address(Addr(0x0802))
            .expect("JSR instruction at $0802");
        handle_vice_registers_get(&build_pc_msg(0x0802), &mut app_state, &mut ui_state);
        assert_eq!(ui_state.cursor_index, expected_idx);
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0802)
        );

        // 2. External ROM PC ($EA31) must not snap the static disassembly cursor to the last line.
        handle_vice_registers_get(&build_pc_msg(0xEA31), &mut app_state, &mut ui_state);
        assert_eq!(ui_state.cursor_index, expected_idx);
    }

    #[test]
    fn mouse_dialog_close_promotes_queued_dialog() {
        use crate::ui::widget::{Widget, WidgetResult};
        use ratatui::{Frame, layout::Rect};

        struct DummyDialog {
            mouse_result: Option<WidgetResult>,
        }
        impl Widget for DummyDialog {
            fn render(&self, _f: &mut Frame, _area: Rect, _app: &AppState, _ui: &mut UIState) {}
            fn handle_input(
                &mut self,
                _key: KeyEvent,
                _app: &mut AppState,
                _ui: &mut UIState,
            ) -> WidgetResult {
                WidgetResult::Ignored
            }
            fn handle_mouse(
                &mut self,
                _mouse: MouseEvent,
                _app: &mut AppState,
                _ui: &mut UIState,
            ) -> WidgetResult {
                self.mouse_result.take().unwrap_or(WidgetResult::Ignored)
            }
        }

        let mut core = Core::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.active_dialog_area = Rect::new(10, 5, 40, 10);
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut should_render = false;

        // 1. Clicking [x] on the top-right border promotes the queued dialog.
        ui_state.push_dialog(Box::new(DummyDialog {
            mouse_result: Some(WidgetResult::Ignored),
        }));
        ui_state.push_dialog(Box::new(DummyDialog {
            mouse_result: Some(WidgetResult::Close),
        }));
        ui_state.push_dialog(Box::new(DummyDialog {
            mouse_result: Some(WidgetResult::Action(
                crate::state::actions::AppAction::FindNext,
            )),
        }));
        assert_eq!(ui_state.dialog_queue.len(), 2);

        let close_x_click = MouseEvent {
            kind: event::MouseEventKind::Down(event::MouseButton::Left),
            column: 48,
            row: 5,
            modifiers: event::KeyModifiers::NONE,
        };
        let _ = handle_mouse_event(
            close_x_click,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert!(ui_state.active_dialog.is_some());
        assert_eq!(ui_state.dialog_queue.len(), 1);

        // 2. Dialog returning WidgetResult::Close on mouse input promotes the next queued dialog.
        let body_click = MouseEvent {
            kind: event::MouseEventKind::Down(event::MouseButton::Left),
            column: 20,
            row: 8,
            modifiers: event::KeyModifiers::NONE,
        };
        let _ = handle_mouse_event(
            body_click,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert!(ui_state.active_dialog.is_some());
        assert_eq!(ui_state.dialog_queue.len(), 0);

        // 3. Dialog returning a closing WidgetResult::Action closes cleanly when queue is empty.
        let _ = handle_mouse_event(
            body_click,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert!(ui_state.active_dialog.is_none());
    }

    #[test]
    fn sync_views_before_render_mirrors_cursor_and_selection_across_panes() {
        use crate::ui_state::RightPane;
        use regenerator2000_core::state::{Addr, BlockType};

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x2000);
        app_state.raw_data = vec![0xEA; 0x2040];
        app_state.block_types = vec![BlockType::Code; app_state.raw_data.len()];
        app_state.system_config.sync_charset_view = true;
        app_state.system_config.sync_sprites_view = true;
        app_state.system_config.sync_bitmap_view = true;
        app_state.disassemble();

        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.active_pane = ActivePane::Disassembly;
        ui_state.cursor_index = app_state
            .get_line_index_containing_address(Addr(0x4010))
            .expect("line at $4010");

        ui_state.right_pane = RightPane::HexDump16;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.hex_cursor_index, (0x4010 - 0x2000) / 16);

        ui_state.right_pane = RightPane::Bitmap;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.bitmap_cursor_index, 1);

        let sel_start_idx = app_state
            .get_line_index_containing_address(Addr(0x2008))
            .expect("line at $2008");
        ui_state.selection_start = Some(sel_start_idx);

        ui_state.right_pane = RightPane::Charset8Col;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.charset_selection_start, Some(1));
        assert_eq!(ui_state.charset_cursor_index, (0x4010 - 0x2000) / 8);

        ui_state.right_pane = RightPane::Sprites2Col;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(0));
        assert_eq!(ui_state.sprites_cursor_index, (0x4010 - 0x2000) / 64);

        ui_state.selection_start = None;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, None);
    }
}
