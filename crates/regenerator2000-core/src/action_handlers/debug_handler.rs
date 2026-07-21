//! Debug domain action handler for VICE monitor protocol, breakpoints, watchpoints, and step execution.

use super::{ActionContext, CoreError, DomainActionHandler};
use crate::event::CoreEvent;
use crate::state::actions::AppAction;

/// Handler for VICE live debugging actions.
#[derive(Debug, Default)]
pub struct DebugActionHandler;

impl DebugActionHandler {
    /// Creates a new [`DebugActionHandler`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
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
                let (tx, _rx) = std::sync::mpsc::channel();
                match crate::vice::ViceClient::connect(addr, tx) {
                    Ok(client) => {
                        ctx.state.vice_state.previous = Some(ctx.state.vice_state.snapshot());
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
                if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                    let checkpoint_id = ctx
                        .state
                        .vice_state
                        .breakpoints
                        .iter()
                        .find(|bp| {
                            bp.address == line.address.0
                                && bp.kind == crate::vice::state::BreakpointKind::Exec
                        })
                        .map(|bp| bp.id);

                    if let Some(id) = checkpoint_id {
                        if let Some(client) = &ctx.state.vice_client {
                            client.send_checkpoint_delete(id);
                            ctx.events.push(CoreEvent::StatusMessage(format!(
                                "Deleting breakpoint #{id} at ${:04X}",
                                line.address.0
                            )));
                        }
                    } else if let Some(client) = &ctx.state.vice_client {
                        client.send_checkpoint_set_exec(line.address.0);
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Creating breakpoint at ${:04X}",
                            line.address.0
                        )));
                    }
                    ctx.events.push(CoreEvent::StateChanged);
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
                if let Some(client) = &ctx.state.vice_client {
                    let existing_id = ctx
                        .state
                        .vice_state
                        .breakpoints
                        .iter()
                        .find(|bp| {
                            bp.address == address.0
                                && bp.kind == crate::vice::state::BreakpointKind::Exec
                        })
                        .map(|bp| bp.id);

                    if let Some(id) = existing_id {
                        client.send_checkpoint_delete(id);
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Deleting breakpoint #{id} at ${:04X}",
                            address.0
                        )));
                    } else {
                        client.send_checkpoint_set_exec(address.0);
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Creating breakpoint at ${:04X}",
                            address.0
                        )));
                    }
                    ctx.events.push(CoreEvent::StateChanged);
                }
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
                if let Some(client) = &ctx.state.vice_client {
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
                            crate::vice::state::BreakpointKind::LoadStore => {
                                client.send_checkpoint_set_load_store(address.0);
                            }
                            _ => {
                                client.send_checkpoint_set_load_store(address.0);
                            }
                        }
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Creating watchpoint at ${:04X}",
                            address.0
                        )));
                    }
                    ctx.events.push(CoreEvent::StateChanged);
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
