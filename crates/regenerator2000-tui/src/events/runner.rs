//! Main TUI event loop, keyboard/mouse routing, and pre-render pane synchronization.

use super::app_event::AppEvent;
use super::input::handle_global_input;
use super::mcp::spawn_mcp_server_thread;
use super::vice::{connect_vice_client, handle_vice_event};
use crate::state::actions::AppAction;
use crate::ui::dialog_import_context::ImportContextDialog;
use crate::ui::menu::{Menu, SearchHistory, handle_menu_action};
use crate::ui::minimap_bar::MinimapBar;
use crate::ui::ui;
use crate::ui::view_bitmap::BitmapView;
use crate::ui::view_blocks::BlocksView;
use crate::ui::view_charset::CharsetView;
use crate::ui::view_debugger::DebuggerView;
use crate::ui::view_disassembly::DisassemblyView;
use crate::ui::view_hexdump::HexDumpView;
use crate::ui::view_sprites::SpritesView;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::{ActivePane, RightPane, UIState};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use regenerator2000_core::Core;
use regenerator2000_core::mcp::handler;
use regenerator2000_core::navigation::{
    self, AlignedGridRange, sync_grid_pane_from_disassembly, sync_hex_pane_from_disassembly,
};
use regenerator2000_core::state::{AppState, BlockItemTarget, EntryAnalysisMode};
use regenerator2000_core::unpacker::{UnpackError, UnpackResult};
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;

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
    event_tx: &Sender<AppEvent>,
    event_rx: &Receiver<AppEvent>,
) -> io::Result<()> {
    // Sync Core's initial view state into TUI's UIState
    ui_state.core = core.view.clone();

    // Initial render before event loop
    terminal
        .draw(|f| ui(f, &core.state, &mut ui_state))
        .map_err(|e| io::Error::other(e.to_string()))?;

    let mut should_render = false;
    let mut mcp_shutdown_tx: Option<oneshot::Sender<()>> = None;

    loop {
        // Wait for event (blocking)
        let Ok(event) = event_rx.recv() else { break };

        match event {
            AppEvent::Mcp(req) => {
                let response = handler::handle_request(&req, &mut core.state, &mut ui_state);
                let _ = req.response_sender.send(response);
                should_render = true;
            }
            AppEvent::McpStartRequested => {
                if mcp_shutdown_tx.is_some() || core.state.mcp_server_running {
                    ui_state.set_status_message("MCP Server is already running.".to_string());
                } else {
                    ui_state.set_status_message("Starting MCP Server...".to_string());
                    mcp_shutdown_tx = spawn_mcp_server_thread(event_tx);
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
            AppEvent::Vice {
                connection_id,
                event,
            } => {
                handle_vice_event(connection_id, event, &mut core.state, &mut ui_state);
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
                    if matches!(result, WidgetResult::Close) {
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
                        event_tx,
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
                        event_tx,
                        &mut should_render,
                    ) {
                        EventOutcome::Quit => break,
                        EventOutcome::Continue | EventOutcome::Skip => {}
                    }
                }
                Event::Resize(width, height) => {
                    terminal
                        .resize(Rect::new(0, 0, width, height))
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

/// Processes the result of a background unpack operation.
///
/// On success, loads the unpacked binary into the application state without running
/// entry-point flow analysis yet ([`EntryAnalysisMode::SkipEntryAnalysis`]), updates the
/// status bar with a summary, and opens `ImportContextDialog` inheriting any pending
/// [`EntryAnalysisMode`] from the pre-unpack dialog so disassembly only runs if confirmed.
/// On failure, clears any pending [`EntryAnalysisMode`] and displays the error in the status bar.
pub fn handle_unpack_complete(
    result: Result<UnpackResult, UnpackError>,
    core: &mut Core,
    ui_state: &mut UIState,
) {
    let entry_mode = ui_state
        .pending_unpack_entry_mode
        .take()
        .unwrap_or_default();
    match result {
        Ok(result) => {
            ui_state.invalidate_bitmap_cache();
            let outcome = core
                .state
                .apply_unpack_result_with_mode(&result, EntryAnalysisMode::SkipEntryAnalysis);
            navigation::perform_jump_to_address_no_history(
                &core.state,
                &mut core.view,
                outcome.entry_point,
            );
            ui_state.core = core.view.clone();
            ui_state.sync_core_to_tui();
            ui_state.set_status_message(outcome.summary);
            // Show import context dialog for system/origin/entry setup.
            let mut dialog = ImportContextDialog::new(
                core.state.settings.system.as_str(),
                outcome.start_addr,
                Some(outcome.entry_point),
                None,
                None,
                Option::<String>::None,
            );
            dialog.entry_mode = entry_mode;
            ui_state.push_dialog(Box::new(dialog));
        }
        Err(e) => {
            ui_state.set_status_message(format!("Unpack failed: {e}"));
        }
    }
}

/// Processes one [`KeyEvent`] synchronously through the full TUI input-routing pipeline.
///
/// This function routes input through dialogs, the menu bar, inline Vim search, active views,
/// `Navigable` handlers, and global shortcuts.
/// It then runs `sync_views_before_render` and [`UIState::sync_tui_to_core`].
/// Integration tests call this headless entrypoint to verify keyboard shortcuts without a terminal backend.
pub fn process_key_event(core: &mut Core, ui_state: &mut UIState, key: KeyEvent) {
    let (event_tx, _event_rx) = mpsc::channel();
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
    event_tx: &Sender<AppEvent>,
    should_render: &mut bool,
) -> EventOutcome {
    if key.kind != KeyEventKind::Press {
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
        let result = Menu.handle_input(key, &mut core.state, ui_state);
        if let WidgetResult::Action(action) = result {
            dispatch_menu_action(action, core, ui_state, event_tx);
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
                dispatch_menu_action(AppAction::FindNext, core, ui_state, event_tx);
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

/// Applies the [`WidgetResult`] returned by an active modal dialog,
/// restoring or closing the dialog and promoting any queued dialog when it closes.
fn apply_dialog_result(
    dialog: Box<dyn Widget>,
    result: WidgetResult,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &Sender<AppEvent>,
    should_render: &mut bool,
) {
    match result {
        WidgetResult::Ignored => {
            ui_state.active_dialog = Some(dialog);
        }
        WidgetResult::Handled => {
            ui_state.active_dialog = Some(dialog);
            *should_render = true;
        }
        WidgetResult::Close => {
            *should_render = true;
            ui_state.promote_next_dialog();
        }
        WidgetResult::Action(action) => {
            if !action.closes_dialog() {
                ui_state.active_dialog = Some(dialog);
            }
            *should_render = true;
            dispatch_menu_action(action, core, ui_state, event_tx);
            ui_state.promote_next_dialog();
        }
    }
}

/// Returns a boxed [`Widget`] for `pane`.
fn active_pane_widget(pane: ActivePane) -> Box<dyn Widget> {
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
    event_tx: &Sender<AppEvent>,
) {
    let mut active_view = active_pane_widget(ui_state.active_pane);

    match active_view.handle_input(key, &mut core.state, ui_state) {
        WidgetResult::Handled => {
            // Event handled, will render below
        }
        WidgetResult::Action(action) => {
            dispatch_menu_action(action, core, ui_state, event_tx);
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
    event_tx: &Sender<AppEvent>,
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
    mut dialog: Box<dyn Widget>,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &Sender<AppEvent>,
    should_render: &mut bool,
) -> EventOutcome {
    // Check for Close Button Click (Top-Right [x]) only once the active dialog has rendered bounds.
    let area = ui_state.active_dialog_area;
    let is_close_click = if area.width > 0
        && area.height > 0
        && mouse.kind == MouseEventKind::Down(MouseButton::Left)
    {
        mouse.row == area.y
            && mouse.column >= area.right().saturating_sub(4)
            && mouse.column < area.right()
    } else {
        false
    };

    if is_close_click {
        // Dialog close requested via [x] — restore status, drop the dialog, and promote any queued dialog.
        *should_render = true;
        ui_state.set_status_message("Ready");
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
    event_tx: &Sender<AppEvent>,
    should_render: &mut bool,
) {
    let col = mouse.column;
    let row = mouse.row;

    let mut widget_result = WidgetResult::Ignored;
    let is_inside = |rect: Rect, col: u16, row: u16| {
        col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
    };

    if is_inside(ui_state.menu_area, col, row) || ui_state.menu.active {
        widget_result = Menu.handle_mouse(mouse, &mut core.state, ui_state);

        // If the menu ignored a left click (e.g. click outside both bar and popup), close the menu.
        if !is_inside(ui_state.menu_area, col, row)
            && matches!(widget_result, WidgetResult::Ignored)
            && mouse.kind == MouseEventKind::Down(MouseButton::Left)
        {
            ui_state.menu.active = false;
            ui_state.menu.selected_item = None;
            widget_result = WidgetResult::Handled;
        }
    }

    let prev_active_pane = ui_state.active_pane;

    if widget_result == WidgetResult::Ignored {
        if is_inside(ui_state.minimap_area, col, row) {
            widget_result = MinimapBar.handle_mouse(mouse, &mut core.state, ui_state);
        } else if is_inside(ui_state.disassembly_area, col, row) {
            ui_state.active_pane = ActivePane::Disassembly;
            widget_result = DisassemblyView.handle_mouse(mouse, &mut core.state, ui_state);
        } else if is_inside(ui_state.right_pane_area, col, row) {
            match ui_state.right_pane {
                RightPane::HexDump16 | RightPane::HexDump8 => {
                    ui_state.active_pane = ActivePane::HexDump;
                }
                RightPane::Sprites2Col | RightPane::Sprites1Col => {
                    ui_state.active_pane = ActivePane::Sprites;
                }
                RightPane::Charset8Col | RightPane::Charset4Col => {
                    ui_state.active_pane = ActivePane::Charset;
                }
                RightPane::Bitmap => {
                    ui_state.active_pane = ActivePane::Bitmap;
                }
                RightPane::Blocks => {
                    ui_state.active_pane = ActivePane::Blocks;
                }
                RightPane::Debugger => {
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
        WidgetResult::Handled | WidgetResult::Action(_)
    ) {
        *should_render = true;
    }

    if let WidgetResult::Action(action) = widget_result {
        dispatch_menu_action(action, core, ui_state, event_tx);
    }
}

// ---------------------------------------------------------------------------
// Shared helper: dispatch an AppAction (de-duplicates ViceConnectAddress logic)
// ---------------------------------------------------------------------------

/// Routes an `AppAction` to the VICE connect flow or the generic menu handler.
fn dispatch_menu_action(
    action: AppAction,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &Sender<AppEvent>,
) {
    if let AppAction::ViceConnectAddress(addr) = action {
        if let Err(err) = connect_vice_client(&mut core.state, &addr, event_tx) {
            ui_state.set_status_message(err.to_string());
        }
    } else {
        if matches!(action, AppAction::ApplyOrigin(_)) {
            ui_state.invalidate_bitmap_cache();
        }
        handle_menu_action(core, ui_state, action, event_tx);
    }
}

// ---------------------------------------------------------------------------
// View synchronization (runs before each render)
// ---------------------------------------------------------------------------

/// Syncs right-pane views with the disassembly cursor position and updates menu availability.
fn sync_views_before_render(app_state: &AppState, ui_state: &mut UIState) {
    ui_state.cached_blocks_items = None;

    // Update menu availability
    let search_history = if ui_state.last_search_query.is_empty() {
        SearchHistory::Empty
    } else {
        SearchHistory::HasQuery
    };
    ui_state.menu.update_availability(
        app_state,
        ui_state.cursor_index,
        search_history,
        ui_state.active_pane,
    );

    // All sync logic only applies when the disassembly pane is active
    if ui_state.active_pane != ActivePane::Disassembly {
        return;
    }

    let Some(line) = app_state.disassembly.get(ui_state.cursor_index) else {
        return;
    };
    // Skip cursor sync on external equates (`$D020`), but still sync active selections and
    // clear stale companion anchors.
    let allow_cursor_sync = line.external_label_address.is_none();
    let target_addr = line.address;
    let is_splitter = line.is_splitter();
    let sync_target_addr = allow_cursor_sync.then_some(target_addr);
    let disassembly_selection = ui_state.selection_start;
    let disassembly_cursor = ui_state.cursor_index;

    // Sync Blocks view and retain `items` for `BlocksView::render` in the same frame.
    if ui_state.right_pane == RightPane::Blocks && app_state.system_config.sync_blocks_view {
        let items = app_state.blocks_view_items();
        if allow_cursor_sync
            && let Some(index) = {
                let target = if is_splitter {
                    BlockItemTarget::Splitter
                } else {
                    BlockItemTarget::BlockOrScope
                };
                AppState::find_block_index_in_items(&items, target_addr, target)
            }
        {
            ui_state.blocks_list_state.select(Some(index));
        }
        ui_state.cached_blocks_items = Some(items);
    }

    // Sync HexDump cursor and selection while preserving sub-instruction hex anchors.
    if matches!(
        ui_state.right_pane,
        RightPane::HexDump16 | RightPane::HexDump8
    ) && app_state.system_config.sync_hex_dump
    {
        sync_hex_pane_from_disassembly(app_state, &mut ui_state.core, sync_target_addr);
    }

    // Sync Charset view
    if matches!(
        ui_state.right_pane,
        RightPane::Charset8Col | RightPane::Charset4Col
    ) && app_state.system_config.sync_charset_view
    {
        let grid = AlignedGridRange::charset(app_state);
        sync_grid_pane_from_disassembly(
            app_state,
            disassembly_selection,
            disassembly_cursor,
            sync_target_addr,
            &grid,
            &mut ui_state.core.charset_cursor_index,
            &mut ui_state.core.charset_selection_start,
        );
    }

    // Sync Sprites view
    if matches!(
        ui_state.right_pane,
        RightPane::Sprites2Col | RightPane::Sprites1Col
    ) && app_state.system_config.sync_sprites_view
    {
        let grid = AlignedGridRange::sprites(app_state);
        sync_grid_pane_from_disassembly(
            app_state,
            disassembly_selection,
            disassembly_cursor,
            sync_target_addr,
            &grid,
            &mut ui_state.core.sprites_cursor_index,
            &mut ui_state.core.sprites_selection_start,
        );
    }

    // Sync Bitmap view
    if ui_state.right_pane == RightPane::Bitmap && app_state.system_config.sync_bitmap_view {
        let grid = AlignedGridRange::bitmap(app_state);
        sync_grid_pane_from_disassembly(
            app_state,
            disassembly_selection,
            disassembly_cursor,
            sync_target_addr,
            &grid,
            &mut ui_state.core.bitmap_cursor_index,
            &mut ui_state.core.bitmap_selection_start,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::ui::dialog_jump_to_operand::JumpToOperandDialog;
    use crate::ui::dialog_keyboard_shortcut::ShortcutsDialog;
    use crossterm::event::KeyModifiers;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use regenerator2000_core::state::types::OperandJumpTarget;
    use regenerator2000_core::state::{Addr, Assembler, BlockType, Label, LabelKind, LabelType};
    use regenerator2000_core::unpacker::{
        BoundSource, ExitReason, UnpackError, UnpackReport, UnpackResult,
    };
    use regenerator2000_core::vice::{ViceConnectionId, ViceEvent};
    use std::net::TcpListener;
    use std::time::Duration;

    fn sample_unpack_result() -> UnpackResult {
        UnpackResult {
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
        }
    }

    #[test]
    fn status_line_shows_packer_and_exit_reason() {
        let mut core = Core::new();
        let mut ui_state = UIState::new(Theme::default());
        let unpack_result = sample_unpack_result();

        handle_unpack_complete(Ok(unpack_result), &mut core, &mut ui_state);

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
    fn unpack_complete_defers_disassembly_until_confirm_and_cancel_leaves_bytes_unexplored() {
        let mut core = Core::new();
        let mut ui_state = UIState::new(Theme::default());
        let unpack_result = sample_unpack_result();

        handle_unpack_complete(Ok(unpack_result), &mut core, &mut ui_state);

        assert!(
            core.state
                .block_types
                .iter()
                .all(|&block_type| block_type == BlockType::Undefined),
            "unpacked bytes should remain Undefined while the 2nd dialog is open"
        );
        assert!(
            !core
                .state
                .labels
                .values()
                .any(|labels| labels.iter().any(|label| label.name == "start")),
            "no 'start' label should exist before the 2nd dialog is confirmed"
        );
        assert!(
            core.state
                .disassembly
                .iter()
                .any(|line| line.mnemonic == ".byte"),
            "deferred disassembly should render .byte lines while the 2nd dialog is open"
        );
        assert!(
            !core
                .state
                .disassembly
                .iter()
                .any(|line| line.mnemonic == "lda" || line.mnemonic == "rts"),
            "no instructions should be disassembled before confirming the 2nd dialog"
        );

        let mut dialog = ui_state
            .active_dialog
            .take()
            .expect("2nd ImportContextDialog must be active");
        let outcome = dialog.handle_input(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut core.state,
            &mut ui_state,
        );
        assert_eq!(outcome, WidgetResult::Close);
        assert!(
            core.state
                .block_types
                .iter()
                .all(|&block_type| block_type == BlockType::Undefined),
            "canceling the 2nd dialog should leave unpacked bytes Undefined"
        );
        assert!(
            !core
                .state
                .labels
                .values()
                .any(|labels| labels.iter().any(|label| label.name == "start")),
            "canceling the 2nd dialog should not add a 'start' label"
        );
    }

    #[test]
    fn unpack_failure_clears_pending_unpack_entry_mode() {
        let mut core = Core::new();
        let mut ui_state = UIState::new(Theme::default());
        ui_state.pending_unpack_entry_mode = Some(EntryAnalysisMode::SkipEntryAnalysis);

        handle_unpack_complete(Err(UnpackError::NoEntryPoint), &mut core, &mut ui_state);

        assert!(
            ui_state.pending_unpack_entry_mode.is_none(),
            "failed unpack must clear pending_unpack_entry_mode so future menu unpacks start fresh"
        );
        assert!(
            ui_state
                .status_message
                .as_deref()
                .is_some_and(|message| message.starts_with("Unpack failed:")),
            "failed unpack should display an 'Unpack failed:' status message, got {:?}",
            ui_state.status_message
        );
    }

    #[test]
    fn mouse_dialog_close_promotes_queued_dialog() {
        struct DummyDialog {
            mouse_result: Option<WidgetResult>,
        }
        impl Widget for DummyDialog {
            fn render(
                &self,
                _f: &mut ratatui::Frame,
                _area: Rect,
                _app: &AppState,
                _ui: &mut UIState,
            ) {
            }
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
        let mut ui_state = UIState::new(Theme::default());
        let (tx, _rx) = mpsc::channel();
        let mut should_render = false;

        // 1. Clicking [x] on the top-right border promotes the queued dialog.
        ui_state.push_dialog(Box::new(DummyDialog {
            mouse_result: Some(WidgetResult::Ignored),
        }));
        ui_state.push_dialog(Box::new(DummyDialog {
            mouse_result: Some(WidgetResult::Close),
        }));
        ui_state.push_dialog(Box::new(DummyDialog {
            mouse_result: Some(WidgetResult::Action(AppAction::FindNext)),
        }));
        assert_eq!(ui_state.dialog_queue.len(), 2);
        // Simulate DummyDialog #1 having been rendered with active bounds before clicking [x].
        ui_state.active_dialog_area = Rect::new(10, 5, 40, 10);

        let close_x_click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 48,
            row: 5,
            modifiers: KeyModifiers::NONE,
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
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 20,
            row: 8,
            modifiers: KeyModifiers::NONE,
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
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x2000);
        app_state.raw_data = vec![0xEA; 0x2040];
        app_state.block_types = vec![BlockType::Code; app_state.raw_data.len()];
        app_state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ExternalJump,
            }],
        );
        app_state.settings.all_labels = true;
        app_state.system_config.sync_charset_view = true;
        app_state.system_config.sync_sprites_view = true;
        app_state.system_config.sync_bitmap_view = true;
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Disassembly;
        ui_state.cursor_index = app_state
            .line_index_containing_address(Addr(0x4010))
            .expect("line at $4010");

        ui_state.right_pane = RightPane::HexDump16;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.hex_cursor_index, (0x4010 - 0x2000) / 16);

        ui_state.right_pane = RightPane::Bitmap;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.bitmap_cursor_index, 1);

        // When sync_bitmap_view is false, Disassembly cursor movement leaves bitmap_cursor_index alone.
        app_state.system_config.sync_bitmap_view = false;
        ui_state.bitmap_cursor_index = 0;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.bitmap_cursor_index, 0);
        app_state.system_config.sync_bitmap_view = true;

        let sel_start_idx = app_state
            .line_index_containing_address(Addr(0x2008))
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

        // External equate preamble lines must not overwrite right-pane cursors.
        let ext_idx = app_state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0xD020)))
            .expect("external label line at $D020");
        ui_state.cursor_index = ext_idx;

        ui_state.hex_cursor_index = 2;
        ui_state.right_pane = RightPane::HexDump16;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.hex_cursor_index, 2);

        ui_state.charset_cursor_index = 3;
        ui_state.right_pane = RightPane::Charset8Col;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.charset_cursor_index, 3);

        ui_state.sprites_cursor_index = 4;
        ui_state.right_pane = RightPane::Sprites2Col;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.sprites_cursor_index, 4);

        ui_state.bitmap_cursor_index = 1;
        ui_state.right_pane = RightPane::Bitmap;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.bitmap_cursor_index, 1);
    }

    #[test]
    fn jump_to_operand_dialog_close_click_dismisses_and_promotes_queued_dialog() {
        let mut core = Core::new();
        let mut ui_state = UIState::new(Theme::default());
        ui_state.dismiss_logo = true;
        let (tx, _rx) = mpsc::channel();
        let mut should_render = false;

        let targets = vec![
            OperandJumpTarget::new(
                Addr(0x1010),
                "target_a".to_string(),
                Some("nop".to_string()),
            ),
            OperandJumpTarget::new(
                Addr(0x1020),
                "target_b".to_string(),
                Some("rts".to_string()),
            ),
        ];
        ui_state.push_dialog(Box::new(JumpToOperandDialog::new(Addr(0x1000), targets)));

        // Render once so `active_dialog_area` matches the real centered modal bounds.
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal
            .draw(|f| ui(f, &core.state, &mut ui_state))
            .expect("draw jump-to-operand dialog");
        let dialog_area = ui_state.active_dialog_area;
        assert_ne!(dialog_area, Rect::default());

        // Queue a follow-up dialog behind `JumpToOperandDialog`.
        ui_state.push_dialog(Box::new(ShortcutsDialog::new()));
        assert_eq!(ui_state.dialog_queue.len(), 1);
        ui_state.set_status_message("Jump pending");

        // Clicking `[x]` on the top-right border resets status, drops the dialog, clears stale
        // modal bounds, and promotes the queued dialog.
        let close_click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: dialog_area.right().saturating_sub(2),
            row: dialog_area.y,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(
            close_click,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );

        assert!(should_render);
        assert_eq!(ui_state.status_message.as_deref(), Some("Ready"));
        assert!(ui_state.active_dialog.is_some());
        assert_eq!(ui_state.dialog_queue.len(), 0);
        assert_eq!(ui_state.active_dialog_area, Rect::default());

        // Clicking the previous dialog's `[x]` coordinate before the newly promoted dialog renders
        // must not dismiss it prematurely.
        handle_mouse_event(
            close_click,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert!(ui_state.active_dialog.is_some());
    }

    #[test]
    fn test_sync_views_before_render_direction_collapsed_wrapped_equates_and_hex() {
        // 1. Backward direction preservation across Tab (Sprites, Charset, Bitmap) + 1-item round-trip
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x2000);
        app_state.raw_data = vec![0xEA; 0x4200];
        app_state.block_types = vec![BlockType::Code; 0x4200];
        app_state.system_config.sync_sprites_view = true;
        app_state.system_config.sync_charset_view = true;
        app_state.system_config.sync_bitmap_view = true;
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Disassembly;

        // Sprites backward 2-item ($20BF -> $2000 -> Some(2), 0) and ($20BF -> $2040 -> Some(2), 1)
        let index_2000 = app_state
            .line_index_containing_address(Addr(0x2000))
            .expect("$2000");
        let index_2040 = app_state
            .line_index_containing_address(Addr(0x2040))
            .expect("$2040");
        let index_20bf = app_state
            .line_index_containing_address(Addr(0x20BF))
            .expect("$20BF");
        let index_203f = app_state
            .line_index_containing_address(Addr(0x203F))
            .expect("$203F");
        ui_state.right_pane = RightPane::Sprites2Col;
        ui_state.selection_start = Some(index_20bf);
        ui_state.cursor_index = index_2000;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(2));
        assert_eq!(ui_state.sprites_cursor_index, 0);

        ui_state.cursor_index = index_2040;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(2));
        assert_eq!(ui_state.sprites_cursor_index, 1);

        // Single-item Sprites round-trip ($203F -> $2000 => (Some(0), 0))
        ui_state.selection_start = Some(index_203f);
        ui_state.cursor_index = index_2000;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(0));
        assert_eq!(ui_state.sprites_cursor_index, 0);

        // Charset backward ($2017 Char 2 -> $2008 Char 1)
        let index_2008 = app_state
            .line_index_containing_address(Addr(0x2008))
            .expect("$2008");
        let index_2017 = app_state
            .line_index_containing_address(Addr(0x2017))
            .expect("$2017");
        ui_state.right_pane = RightPane::Charset8Col;
        ui_state.selection_start = Some(index_2017);
        ui_state.cursor_index = index_2008;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.charset_selection_start, Some(2));
        assert_eq!(ui_state.charset_cursor_index, 1);

        // Bitmap backward ($5F3F Bitmap 1 -> $2000 Bitmap 0) and 1-item ($3F3F -> $2000)
        let index_5f3f = app_state
            .line_index_containing_address(Addr(0x5F3F))
            .expect("$5F3F");
        let index_3f3f = app_state
            .line_index_containing_address(Addr(0x3F3F))
            .expect("$3F3F");
        ui_state.right_pane = RightPane::Bitmap;
        ui_state.selection_start = Some(index_5f3f);
        ui_state.cursor_index = index_2000;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.bitmap_selection_start, Some(1));
        assert_eq!(ui_state.bitmap_cursor_index, 0);

        ui_state.selection_start = Some(index_3f3f);
        ui_state.cursor_index = index_2000;
        sync_views_before_render(&app_state, &mut ui_state);
        assert_eq!(ui_state.bitmap_selection_start, Some(0));
        assert_eq!(ui_state.bitmap_cursor_index, 0);

        // 2. Collapsed block mirroring: (0, 127) ($2000..=$207F) maps to 2 sprites (0..=1)
        let mut collapsed_state = AppState::new();
        collapsed_state.origin = Addr(0x2000);
        collapsed_state.raw_data = vec![0xEA; 256];
        collapsed_state.block_types = vec![BlockType::Code; 256];
        collapsed_state.collapsed_blocks.push((0, 127));
        collapsed_state.system_config.sync_sprites_view = true;
        collapsed_state.disassemble();
        let collapsed_index = collapsed_state
            .disassembly
            .iter()
            .position(|l| l.is_collapsed && l.address == Addr(0x2000))
            .expect("collapsed block at $2000");
        ui_state.right_pane = RightPane::Sprites2Col;
        ui_state.selection_start = Some(collapsed_index);
        ui_state.cursor_index = collapsed_index;
        sync_views_before_render(&collapsed_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(0));
        assert_eq!(ui_state.sprites_cursor_index, 1);

        // 3. Wrapped binary $FFFE..=$0001 collapsed block
        let mut wrapped_state = AppState::new();
        wrapped_state.origin = Addr(0xFFFE);
        wrapped_state.raw_data = vec![0xEA; 4];
        wrapped_state.block_types = vec![BlockType::Undefined; 4];
        wrapped_state.collapsed_blocks.push((0, 3));
        wrapped_state.system_config.sync_sprites_view = true;
        wrapped_state.disassemble();
        let wrapped_index = wrapped_state
            .disassembly
            .iter()
            .position(|l| l.is_collapsed && l.address == Addr(0xFFFE))
            .expect("collapsed block at $FFFE");
        ui_state.right_pane = RightPane::Sprites2Col;
        ui_state.sprites_cursor_index = 5;
        ui_state.sprites_selection_start = Some(5);
        ui_state.selection_start = Some(wrapped_index);
        ui_state.cursor_index = wrapped_index;
        sync_views_before_render(&wrapped_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(0));
        assert_eq!(ui_state.sprites_cursor_index, 0);

        // 4. External equates $D020, $0000 preamble, and closing `}` exclusion + allow_cursor_sync
        let mut equate_state = AppState::new();
        equate_state.origin = Addr(0x2000);
        equate_state.raw_data = vec![0xEA; 0x200];
        equate_state.block_types = vec![BlockType::Code; 0x200];
        equate_state.settings.assembler = Assembler::Kick;
        equate_state.settings.all_labels = true;
        equate_state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ExternalJump,
            }],
        );
        equate_state.annotations.update(Addr(0x2000), |e| {
            e.scope = Some(Addr(0x203F));
        });
        equate_state.system_config.sync_blocks_view = true;
        equate_state.system_config.sync_hex_dump = true;
        equate_state.system_config.sync_charset_view = true;
        equate_state.system_config.sync_sprites_view = true;
        equate_state.system_config.sync_bitmap_view = true;
        equate_state.disassemble();

        let external_index = equate_state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0xD020)))
            .expect("$D020 external line");
        let preamble_index = equate_state
            .disassembly
            .iter()
            .position(|l| {
                l.address == Addr(0x0000)
                    && l.bytes.is_empty()
                    && !l.is_collapsed
                    && l.external_label_address.is_none()
            })
            .expect("$0000 preamble line");
        let close_brace_index = equate_state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x2040) && l.mnemonic == "}" && l.bytes.is_empty())
            .expect("closing } scope line at $2040");
        let equate_2000 = equate_state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x2000) && !l.bytes.is_empty())
            .expect("code line at $2000");
        let equate_207f = equate_state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x207F) && !l.bytes.is_empty())
            .expect("code line at $207F");

        // 4a. Cursor on $D020 with selection_start = None: keep cursors, clear stale selections, cache Blocks
        ui_state.selection_start = None;
        ui_state.cursor_index = external_index;
        ui_state.hex_cursor_index = 7;
        ui_state.hex_col_cursor = 3;
        ui_state.hex_selection_start = Some(1);
        ui_state.hex_selection_start_col = 4;
        ui_state.sprites_cursor_index = 4;
        ui_state.sprites_selection_start = Some(2);
        ui_state.charset_cursor_index = 9;
        ui_state.charset_selection_start = Some(3);
        ui_state.bitmap_cursor_index = 1;
        ui_state.bitmap_selection_start = Some(0);
        ui_state.blocks_list_state.select(Some(1));

        ui_state.right_pane = RightPane::Blocks;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert!(ui_state.cached_blocks_items.is_some());
        assert_eq!(ui_state.blocks_list_state.selected(), Some(1));

        ui_state.right_pane = RightPane::HexDump16;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.hex_cursor_index, 7);
        assert_eq!(ui_state.hex_col_cursor, 3);
        assert_eq!(ui_state.hex_selection_start, None);
        assert_eq!(ui_state.hex_selection_start_col, 0);

        ui_state.right_pane = RightPane::Sprites2Col;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.sprites_cursor_index, 4);
        assert_eq!(ui_state.sprites_selection_start, None);

        ui_state.right_pane = RightPane::Charset8Col;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.charset_cursor_index, 9);
        assert_eq!(ui_state.charset_selection_start, None);

        ui_state.right_pane = RightPane::Bitmap;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.bitmap_cursor_index, 1);
        assert_eq!(ui_state.bitmap_selection_start, None);

        // 4b. Selection from $D020 to $207F (and reversed $207F to $D020) mirrors only in-binary (0, 1) / (1, 0)
        ui_state.right_pane = RightPane::Sprites2Col;
        ui_state.selection_start = Some(external_index);
        ui_state.cursor_index = equate_207f;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(0));
        assert_eq!(ui_state.sprites_cursor_index, 1);

        ui_state.selection_start = Some(equate_207f);
        ui_state.cursor_index = external_index;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(1));
        assert_eq!(ui_state.sprites_cursor_index, 0);

        // 4c. Selection covering ONLY $0000 preamble + $D020 external equates clears stale selections
        ui_state.selection_start = Some(preamble_index);
        ui_state.cursor_index = external_index;
        ui_state.sprites_selection_start = Some(1);
        ui_state.right_pane = RightPane::Sprites2Col;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, None);

        ui_state.charset_selection_start = Some(1);
        ui_state.right_pane = RightPane::Charset8Col;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.charset_selection_start, None);

        ui_state.bitmap_selection_start = Some(0);
        ui_state.right_pane = RightPane::Bitmap;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.bitmap_selection_start, None);

        // 4d. Selection from $2000 to closing `}` at $2040 stays within Sprite 0 (no 1-byte spill)
        ui_state.right_pane = RightPane::Sprites2Col;
        ui_state.selection_start = Some(equate_2000);
        ui_state.cursor_index = close_brace_index;
        sync_views_before_render(&equate_state, &mut ui_state);
        assert_eq!(ui_state.sprites_selection_start, Some(0));
        assert_eq!(ui_state.sprites_cursor_index, 0);

        // 5. HexDump across Tab (sync_hex_dump = true)
        let mut hex_state = AppState::new();
        hex_state.origin = Addr(0x0800);
        // $0800..$0803: 4 NOPs, $0804..$0806: JSR $0810, $0807: RTS.
        hex_state.raw_data = vec![0xEA, 0xEA, 0xEA, 0xEA, 0x20, 0x10, 0x08, 0x60];
        hex_state.block_types = vec![BlockType::Code; 8];
        hex_state.system_config.sync_hex_dump = true;
        hex_state.disassemble();
        let line_0804 = hex_state
            .line_index_containing_address(Addr(0x0804))
            .expect("JSR at $0804");

        // 5a. selection_start.is_none() clears stale hex_selection_start + col and syncs cursor
        ui_state.active_pane = ActivePane::Disassembly;
        ui_state.right_pane = RightPane::HexDump16;
        ui_state.dismiss_logo = true;
        ui_state.selection_start = None;
        ui_state.cursor_index = line_0804;
        ui_state.hex_selection_start = Some(1);
        ui_state.hex_selection_start_col = 5;
        sync_views_before_render(&hex_state, &mut ui_state);
        assert_eq!(ui_state.hex_selection_start, None);
        assert_eq!(ui_state.hex_selection_start_col, 0);
        assert_eq!(ui_state.hex_cursor_index, 0);
        assert_eq!(ui_state.hex_col_cursor, 4);

        // 5b. selection_start.is_some() preserves sub-instruction hex anchors across sync & render
        ui_state.selection_start = Some(line_0804);
        ui_state.cursor_index = line_0804;
        ui_state.hex_selection_start = Some(0);
        ui_state.hex_selection_start_col = 4;
        ui_state.hex_cursor_index = 0;
        ui_state.hex_col_cursor = 5;
        sync_views_before_render(&hex_state, &mut ui_state);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("create test terminal for hex sync test");
        terminal
            .draw(|f| ui(f, &hex_state, &mut ui_state))
            .expect("draw UI for hex sync test");
        assert_eq!(ui_state.hex_selection_start, Some(0));
        assert_eq!(ui_state.hex_selection_start_col, 4);
        assert_eq!(ui_state.hex_cursor_index, 0);
        assert_eq!(ui_state.hex_col_cursor, 5);

        // 5c. Extending Disassembly selection past $0804 updates HexDump from sub-instruction bounds.
        let line_0807 = hex_state
            .line_index_containing_address(Addr(0x0807))
            .expect("RTS at $0807");
        ui_state.selection_start = Some(line_0804);
        ui_state.cursor_index = line_0807;
        sync_views_before_render(&hex_state, &mut ui_state);
        assert_eq!(ui_state.hex_selection_start, Some(0));
        assert_eq!(ui_state.hex_selection_start_col, 4);
        assert_eq!(ui_state.hex_cursor_index, 0);
        assert_eq!(ui_state.hex_col_cursor, 7);

        // 5d. Fresh Disassembly selection ($0800..=$0806) mirrors into HexDump when no hex anchor exists.
        let line_0800 = hex_state
            .line_index_containing_address(Addr(0x0800))
            .expect("NOP at $0800");
        ui_state.hex_selection_start = None;
        ui_state.hex_selection_start_col = 0;
        ui_state.selection_start = Some(line_0800);
        ui_state.cursor_index = line_0804;
        sync_views_before_render(&hex_state, &mut ui_state);
        assert_eq!(ui_state.hex_selection_start, Some(0));
        assert_eq!(ui_state.hex_selection_start_col, 0);
        assert_eq!(ui_state.hex_cursor_index, 0);
        assert_eq!(ui_state.hex_col_cursor, 6);
    }

    #[test]
    fn test_sprites_and_charset_mouse_shift_click_and_drag_sync() {
        let mut core = Core::new();
        core.state
            .load_binary(Addr(0x0800), vec![0xEA; 256])
            .expect("load binary fixture");
        core.state.block_types = vec![BlockType::Undefined; 256];
        core.state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.dismiss_logo = true;
        let (tx, _rx) = mpsc::channel();
        let mut should_render = false;

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("create test terminal for mouse sync");

        // --- Sprites2Col ---
        ui_state.right_pane = RightPane::Sprites2Col;
        terminal
            .draw(|f| ui(f, &core.state, &mut ui_state))
            .expect("draw sprites pane");
        let sprite_area = ui_state.right_pane_area;

        // 1. Plain Left-Click on Sprite 1 (column 2): Tier 1 passive focus on $0840
        let click_sprite_1 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: sprite_area.x + 1 + 30,
            row: sprite_area.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(
            click_sprite_1,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.active_pane, ActivePane::Sprites);
        assert_eq!(ui_state.sprites_cursor_index, 1);
        assert_eq!(ui_state.sprites_selection_start, None);
        assert_eq!(ui_state.selection_start, None);
        assert_eq!(
            core.state.disassembly[ui_state.cursor_index].address,
            Addr(0x0840)
        );

        // 2. Shift+Click on Sprite 1 itself: 1-item Tier 2 selection ($0840..=$087F, 64 bytes)
        let shift_click_sprite_1 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: sprite_area.x + 1 + 30,
            row: sprite_area.y + 2,
            modifiers: KeyModifiers::SHIFT,
        };
        handle_mouse_event(
            shift_click_sprite_1,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.sprites_selection_start, Some(1));
        assert_eq!(ui_state.sprites_cursor_index, 1);
        assert_eq!(
            core.state.disassembly[ui_state.cursor_index].address,
            Addr(0x0840)
        );
        assert_eq!(
            core.state.disassembly[ui_state
                .selection_start
                .expect("selection_start set for single-sprite Shift+Click")]
            .address,
            Addr(0x087F)
        );

        // 3. Mouse Down(Left) on Sprite 0 -> Drag(Left) to Sprite 1 ($0800..=$087F, 128 bytes)
        let down_sprite_0 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: sprite_area.x + 2,
            row: sprite_area.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(
            down_sprite_0,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.sprites_selection_start, None);
        assert_eq!(ui_state.sprites_cursor_index, 0);

        let drag_sprite_1 = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: sprite_area.x + 1 + 30,
            row: sprite_area.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(
            drag_sprite_1,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.sprites_selection_start, Some(0));
        assert_eq!(ui_state.sprites_cursor_index, 1);
        assert_eq!(
            core.state.disassembly[ui_state
                .selection_start
                .expect("selection_start set for sprite drag")]
            .address,
            Addr(0x0800)
        );
        assert_eq!(
            core.state.disassembly[ui_state.cursor_index].address,
            Addr(0x087F)
        );

        // --- Charset8Col ---
        ui_state.right_pane = RightPane::Charset8Col;
        ui_state.active_pane = ActivePane::Charset;
        ui_state.charset_cursor_index = 0;
        ui_state.charset_selection_start = None;
        terminal
            .draw(|f| ui(f, &core.state, &mut ui_state))
            .expect("draw charset pane");
        let charset_area = ui_state.right_pane_area;

        // 4. Plain Left-Click on Char 0 -> Shift+Click on Char 1 (y+2 skips bank header row)
        let click_char_0 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: charset_area.x + 2,
            row: charset_area.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(
            click_char_0,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.charset_cursor_index, 0);
        assert_eq!(ui_state.charset_selection_start, None);

        let shift_click_char_1 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: charset_area.x + 1 + 10,
            row: charset_area.y + 2,
            modifiers: KeyModifiers::SHIFT,
        };
        handle_mouse_event(
            shift_click_char_1,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.charset_selection_start, Some(0));
        assert_eq!(ui_state.charset_cursor_index, 1);
        assert_eq!(
            core.state.disassembly[ui_state
                .selection_start
                .expect("selection_start set for charset Shift+Click")]
            .address,
            Addr(0x0800)
        );
        assert_eq!(
            core.state.disassembly[ui_state.cursor_index].address,
            Addr(0x080F)
        );

        // 5. Plain Left-Click on Char 1 -> Drag(Left) to Char 0 (backward 2-char selection)
        let click_char_1 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: charset_area.x + 1 + 10,
            row: charset_area.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(
            click_char_1,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.charset_cursor_index, 1);
        assert_eq!(ui_state.charset_selection_start, None);

        // Shift+Click on Char 1 itself -> 1-item Tier 2 selection ($0808..=$080F, 8 bytes)
        handle_mouse_event(
            shift_click_char_1,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.charset_selection_start, Some(1));
        assert_eq!(ui_state.charset_cursor_index, 1);
        assert_eq!(
            core.state.disassembly[ui_state.cursor_index].address,
            Addr(0x0808)
        );
        assert_eq!(
            core.state.disassembly[ui_state
                .selection_start
                .expect("selection_start set for single-char Shift+Click")]
            .address,
            Addr(0x080F)
        );

        // Drag(Left) from Char 1 back to Char 0 -> backward 2-char ($0800..=$080F)
        handle_mouse_event(
            click_char_1,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        let drag_char_0 = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: charset_area.x + 2,
            row: charset_area.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse_event(
            drag_char_0,
            &mut core,
            &mut ui_state,
            &tx,
            &mut should_render,
        );
        assert_eq!(ui_state.charset_selection_start, Some(1));
        assert_eq!(ui_state.charset_cursor_index, 0);
        assert_eq!(
            core.state.disassembly[ui_state.cursor_index].address,
            Addr(0x0800)
        );
        assert_eq!(
            core.state.disassembly[ui_state
                .selection_start
                .expect("selection_start set for backward charset drag")]
            .address,
            Addr(0x080F)
        );
    }

    fn recv_vice_event(receiver: &mpsc::Receiver<AppEvent>) -> (ViceConnectionId, ViceEvent) {
        match receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("receive VICE event before timeout")
        {
            AppEvent::Vice {
                connection_id,
                event,
            } => (connection_id, event),
            _ => panic!("expected AppEvent::Vice"),
        }
    }

    #[test]
    fn vice_remote_disconnect_updates_menu_availability_and_allows_reconnect() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test TCP listener");
        let address = listener
            .local_addr()
            .expect("read listener local address")
            .to_string();

        let mut core = Core::new();
        core.state
            .load_binary(Addr(0x0801), vec![0xEA; 16])
            .expect("load test binary");
        let mut ui_state = UIState::new(Theme::default());
        let (tx, rx) = mpsc::channel::<AppEvent>();

        // 1. Connect to VICE and verify dump_address survives connection setup.
        core.state.vice_state.dump_address = Some(0xD000);
        dispatch_menu_action(
            AppAction::ViceConnectAddress(address.clone()),
            &mut core,
            &mut ui_state,
            &tx,
        );
        assert_eq!(core.state.vice_state.dump_address, Some(0xD000));
        let (server_stream, _) = listener.accept().expect("accept first VICE connection");
        let (connection_id, event) = recv_vice_event(&rx);
        assert_eq!(event, ViceEvent::Connected);
        handle_vice_event(connection_id, event, &mut core.state, &mut ui_state);
        sync_views_before_render(&core.state, &mut ui_state);

        let debugger_category_index = ui_state
            .menu
            .categories
            .iter()
            .position(|category| category.name == "Debugger")
            .expect("Debugger menu category must exist");
        let item_disabled = |ui_state: &UIState, action: AppAction| -> bool {
            ui_state.menu.categories[debugger_category_index]
                .items
                .iter()
                .find(|item| item.action.as_ref() == Some(&action))
                .unwrap_or_else(|| panic!("missing menu item for {action:?}"))
                .disabled
        };

        assert!(item_disabled(&ui_state, AppAction::ViceConnect));
        assert!(!item_disabled(&ui_state, AppAction::ViceDisconnect));
        assert!(!item_disabled(&ui_state, AppAction::ViceContinue));

        // 2. Resume execution (F9).
        dispatch_menu_action(AppAction::ViceContinue, &mut core, &mut ui_state, &tx);
        sync_views_before_render(&core.state, &mut ui_state);
        assert!(core.state.vice_state.running);
        assert!(item_disabled(&ui_state, AppAction::ViceConnect));
        assert!(!item_disabled(&ui_state, AppAction::ViceDisconnect));
        assert!(item_disabled(&ui_state, AppAction::ViceContinue));
        assert!(item_disabled(&ui_state, AppAction::ViceStep));

        // Open the Debugger menu on "Disconnect from VICE" before VICE exits.
        ui_state.menu.active = true;
        ui_state.menu.selected_category = debugger_category_index;
        ui_state.menu.select_first_enabled_item();
        assert_eq!(ui_state.menu.selected_item, Some(1));

        // 3. Close the server socket to simulate a VICE exit.
        drop(server_stream);

        let (disconnect_id, disconnect_event) = recv_vice_event(&rx);
        assert_eq!(disconnect_id, connection_id);
        assert!(matches!(disconnect_event, ViceEvent::Disconnected(_)));
        handle_vice_event(
            disconnect_id,
            disconnect_event,
            &mut core.state,
            &mut ui_state,
        );
        sync_views_before_render(&core.state, &mut ui_state);

        assert!(!core.state.is_vice_connected());
        assert!(core.state.vice_client.is_none());
        assert!(!core.state.vice_state.connected);
        assert!(!core.state.vice_state.running);

        assert!(!item_disabled(&ui_state, AppAction::ViceConnect));
        for item in &ui_state.menu.categories[debugger_category_index].items {
            if item.is_separator || item.action.as_ref() == Some(&AppAction::ViceConnect) {
                continue;
            }
            assert!(
                item.disabled,
                "menu item '{}' must be disabled after disconnect",
                item.name
            );
        }
        assert_eq!(ui_state.menu.selected_item, Some(0));

        // 4. Reconnect to VICE.
        dispatch_menu_action(
            AppAction::ViceConnectAddress(address),
            &mut core,
            &mut ui_state,
            &tx,
        );
        let (_second_server, _) = listener.accept().expect("accept second VICE connection");
        let (second_connection_id, second_event) = recv_vice_event(&rx);
        assert!(
            second_connection_id > connection_id,
            "expected second connection ID ({second_connection_id:?}) to exceed first ({connection_id:?})"
        );
        assert_eq!(second_event, ViceEvent::Connected);
        handle_vice_event(
            second_connection_id,
            second_event,
            &mut core.state,
            &mut ui_state,
        );
        sync_views_before_render(&core.state, &mut ui_state);

        assert!(core.state.is_vice_connected());
        assert!(item_disabled(&ui_state, AppAction::ViceConnect));
        assert!(!item_disabled(&ui_state, AppAction::ViceDisconnect));
    }

    #[test]
    fn vice_stale_connection_events_are_ignored_after_disconnect_and_reconnect() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test TCP listener");
        let address = listener
            .local_addr()
            .expect("read listener local address")
            .to_string();

        let mut core = Core::new();
        core.state
            .load_binary(Addr(0x0801), vec![0xEA; 16])
            .expect("load test binary");
        let mut ui_state = UIState::new(Theme::default());
        let (tx, rx) = mpsc::channel::<AppEvent>();

        // Connect session 1.
        dispatch_menu_action(
            AppAction::ViceConnectAddress(address.clone()),
            &mut core,
            &mut ui_state,
            &tx,
        );
        let (_first_server, _) = listener.accept().expect("accept session 1");
        let first_id = core
            .state
            .vice_client
            .as_ref()
            .expect("first client attached")
            .connection_id();
        let _ = recv_vice_event(&rx);

        // Disconnect session 1 manually.
        handle_menu_action(&mut core, &mut ui_state, AppAction::ViceDisconnect, &tx);
        assert!(!core.state.is_vice_connected());

        // Ignore a stale Connected event from session 1.
        handle_vice_event(
            first_id,
            ViceEvent::Connected,
            &mut core.state,
            &mut ui_state,
        );
        assert!(!core.state.is_vice_connected());
        assert!(!core.state.vice_state.connected);

        // Connect session 2.
        dispatch_menu_action(
            AppAction::ViceConnectAddress(address),
            &mut core,
            &mut ui_state,
            &tx,
        );
        let (_second_server, _) = listener.accept().expect("accept session 2");
        let second_id = core
            .state
            .vice_client
            .as_ref()
            .expect("second client attached")
            .connection_id();
        assert!(
            second_id > first_id,
            "expected second connection ID ({second_id:?}) to exceed first ({first_id:?})"
        );

        handle_vice_event(
            second_id,
            ViceEvent::Connected,
            &mut core.state,
            &mut ui_state,
        );
        assert_eq!(
            ui_state.status_message.as_deref(),
            Some("Connected to VICE Monitor")
        );

        // Ignore a stale Disconnected event from session 1 so session 2 and the status stay unchanged.
        handle_vice_event(
            first_id,
            ViceEvent::Disconnected("Connection closed".to_string()),
            &mut core.state,
            &mut ui_state,
        );
        assert!(core.state.is_vice_connected());
        assert!(core.state.is_active_vice_connection(second_id));
        assert_eq!(
            ui_state.status_message.as_deref(),
            Some("Connected to VICE Monitor")
        );
    }
}
