//! Debug domain action handler for the VICE monitor protocol, checkpoints, and step execution.

use super::{ActionContext, CoreError, DomainActionHandler};
use crate::event::CoreEvent;
use crate::state::actions::AppAction;

/// Handler for live VICE debugging actions.
#[derive(Debug, Default)]
pub struct DebugActionHandler;

impl DebugActionHandler {
    /// Creates a new [`DebugActionHandler`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Toggles an execution breakpoint at `address` on the active VICE connection.
fn handle_toggle_exec_breakpoint(ctx: &mut ActionContext<'_>, address: u16) {
    let Some(client) = &ctx.state.vice_client else {
        ctx.events.push(CoreEvent::StatusMessage(
            "Not connected to VICE".to_string(),
        ));
        return;
    };

    let existing_id = ctx
        .state
        .vice_state
        .breakpoints
        .iter()
        .find(|bp| bp.address == address && bp.kind == crate::vice::state::BreakpointKind::Exec)
        .map(|bp| bp.id);

    if let Some(id) = existing_id {
        client.send_checkpoint_delete(id);
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Deleting breakpoint #{id} at ${address:04X}"
        )));
    } else {
        client.send_checkpoint_set_exec(address);
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Creating breakpoint at ${address:04X}"
        )));
    }
    ctx.events.push(CoreEvent::StateChanged);
}

impl DomainActionHandler for DebugActionHandler {
    fn handle_action(
        &self,
        action: &AppAction,
        ctx: &mut ActionContext<'_>,
    ) -> Result<bool, CoreError> {
        match action {
            AppAction::ViceConnect => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ViceConnect,
                ));
                ctx.events.push(CoreEvent::StatusMessage(
                    "Enter VICE hostname and port".to_string(),
                ));
                Ok(true)
            }
            AppAction::ViceConnectAddress(addr) => {
                let (tx, rx) = std::sync::mpsc::channel();
                match crate::vice::ViceClient::connect(addr, tx) {
                    Ok(mut client) => {
                        client.set_event_receiver(rx);
                        ctx.state.vice_state.previous = Some(ctx.state.vice_state.snapshot());
                        ctx.state.vice_state.connected = true;
                        client.send_registers_get();
                        client.send_checkpoint_list();
                        ctx.state.vice_client = Some(client);
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Connected to VICE at {addr}"
                        )));
                        ctx.events.push(CoreEvent::StateChanged);
                    }
                    Err(err) => {
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Failed to connect to VICE: {err}"
                        )));
                    }
                }
                Ok(true)
            }
            AppAction::ViceDisconnect => {
                ctx.state.vice_client = None;
                ctx.state.vice_state.connected = false;
                ctx.events.push(CoreEvent::StatusMessage(
                    "Disconnected from VICE".to_string(),
                ));
                ctx.events.push(CoreEvent::StateChanged);
                Ok(true)
            }
            AppAction::ViceStep => {
                if let Some(client) = &ctx.state.vice_client {
                    ctx.state.vice_state.previous = Some(ctx.state.vice_state.snapshot());
                    client.send_advance_instruction();
                    ctx.state.vice_state.running = true;
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
                Ok(true)
            }
            AppAction::ViceContinue => {
                if let Some(client) = &ctx.state.vice_client {
                    ctx.state.vice_state.previous = Some(ctx.state.vice_state.snapshot());
                    client.send_continue();
                    ctx.state.vice_state.running = true;
                    ctx.events
                        .push(CoreEvent::StatusMessage("VICE: Running...".to_string()));
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
                Ok(true)
            }
            AppAction::ViceStepOver => {
                if let Some(client) = &ctx.state.vice_client {
                    ctx.state.vice_state.previous = Some(ctx.state.vice_state.snapshot());
                    client.send_step_over();
                    ctx.state.vice_state.running = true;
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
                Ok(true)
            }
            AppAction::ViceStepOut => {
                if let Some(client) = &ctx.state.vice_client {
                    ctx.state.vice_state.previous = Some(ctx.state.vice_state.snapshot());
                    client.send_execute_until_return();
                    ctx.state.vice_state.running = true;
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
                Ok(true)
            }
            AppAction::ViceRunToCursor => {
                if let Some(client) = &ctx.state.vice_client {
                    if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                        ctx.state.vice_state.previous = Some(ctx.state.vice_state.snapshot());
                        client.send_checkpoint_set_exec_temp(line.address.0);
                        client.send_continue();
                        ctx.state.vice_state.running = true;
                    }
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
                Ok(true)
            }
            AppAction::ViceToggleBreakpoint => {
                if ctx.state.vice_client.is_none() {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                } else if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                    handle_toggle_exec_breakpoint(ctx, line.address.0);
                }
                Ok(true)
            }
            AppAction::ViceBreakpointDialog => {
                let prefill = ctx
                    .state
                    .disassembly
                    .get(ctx.view.cursor_index)
                    .map(|l| l.address.0);
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::BreakpointAddress(prefill),
                ));
                Ok(true)
            }
            AppAction::ViceSetBreakpointAt { address } => {
                handle_toggle_exec_breakpoint(ctx, address.0);
                Ok(true)
            }
            AppAction::ViceToggleWatchpoint => {
                let prefill = ctx
                    .state
                    .disassembly
                    .get(ctx.view.cursor_index)
                    .map(|l| l.address.0);
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::WatchpointAddress(prefill),
                ));
                Ok(true)
            }
            AppAction::ViceMemoryDumpDialog => {
                let prefill = ctx.state.vice_state.dump_address;
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::MemoryDumpAddress(prefill),
                ));
                Ok(true)
            }
            AppAction::ViceSetMemoryDumpAddress { address } => {
                ctx.state.vice_state.dump_address = Some(address.0);
                if let Some(client) = &ctx.state.vice_client
                    && !ctx.state.vice_state.running
                {
                    client.send_memory_get(address.0, address.0.saturating_add(63), 6);
                }
                ctx.events.push(CoreEvent::StatusMessage(format!(
                    "Memory dump set to ${:04X}",
                    address.0
                )));
                ctx.events.push(CoreEvent::StateChanged);
                Ok(true)
            }
            AppAction::ViceSetWatchpoint { address, kind } => {
                let Some(client) = &ctx.state.vice_client else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                    return Ok(true);
                };

                let existing_id = ctx
                    .state
                    .vice_state
                    .breakpoints
                    .iter()
                    .find(|bp| {
                        bp.address == address.0
                            && bp.kind != crate::vice::state::BreakpointKind::Exec
                    })
                    .map(|bp| bp.id);

                if let Some(id) = existing_id {
                    client.send_checkpoint_delete(id);
                    ctx.events.push(CoreEvent::StatusMessage(format!(
                        "Deleting watchpoint #{id} at ${:04X}",
                        address.0
                    )));
                } else {
                    match kind {
                        crate::vice::state::BreakpointKind::Load => {
                            client.send_checkpoint_set_load(address.0);
                        }
                        crate::vice::state::BreakpointKind::Store => {
                            client.send_checkpoint_set_store(address.0);
                        }
                        crate::vice::state::BreakpointKind::LoadStore
                        | crate::vice::state::BreakpointKind::Exec => {
                            client.send_checkpoint_set_load_store(address.0);
                        }
                    }
                    ctx.events.push(CoreEvent::StatusMessage(format!(
                        "Creating watchpoint at ${:04X}",
                        address.0
                    )));
                }
                ctx.events.push(CoreEvent::StateChanged);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::state::types::Addr;
    use crate::view_state::CoreViewState;

    #[test]
    fn test_vice_connect_address_retains_event_receiver() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr_str = listener.local_addr().unwrap().to_string();

        let mut state = AppState::new();
        let mut view = CoreViewState::new();
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = DebugActionHandler::new();
        assert!(
            handler
                .handle_action(&AppAction::ViceConnectAddress(addr_str), &mut ctx)
                .unwrap()
        );
        assert!(ctx.state.vice_state.connected);
        let client = ctx
            .state
            .vice_client
            .as_ref()
            .expect("Expected connected ViceClient");
        let rx = client
            .event_receiver()
            .expect("Expected event_receiver to remain attached");
        let first_event = rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("Expected ViceEvent::Connected");
        assert!(matches!(first_event, crate::vice::ViceEvent::Connected));
    }

    #[test]
    fn test_disconnected_breakpoint_and_watchpoint_actions_report_status() {
        let mut state = AppState::new();
        state.load_binary(Addr(0x1000), vec![0xEA]).unwrap();
        let mut view = CoreViewState::new();
        let mut events = Vec::new();
        let mut ctx = ActionContext {
            state: &mut state,
            view: &mut view,
            events: &mut events,
        };

        let handler = DebugActionHandler::new();
        handler
            .handle_action(&AppAction::ViceToggleBreakpoint, &mut ctx)
            .unwrap();
        assert_eq!(
            *ctx.events,
            vec![CoreEvent::StatusMessage(
                "Not connected to VICE".to_string()
            )]
        );

        ctx.events.clear();
        handler
            .handle_action(
                &AppAction::ViceSetBreakpointAt {
                    address: Addr(0x1000),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            *ctx.events,
            vec![CoreEvent::StatusMessage(
                "Not connected to VICE".to_string()
            )]
        );

        ctx.events.clear();
        handler
            .handle_action(
                &AppAction::ViceSetWatchpoint {
                    address: Addr(0x1000),
                    kind: crate::vice::state::BreakpointKind::Store,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            *ctx.events,
            vec![CoreEvent::StatusMessage(
                "Not connected to VICE".to_string()
            )]
        );
    }
}
