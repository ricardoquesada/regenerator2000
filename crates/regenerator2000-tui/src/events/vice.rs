//! VICE binary monitor client connection and protocol event handling.

use super::app_event::AppEvent;
use crate::ui::view_disassembly::DisassemblyView;
use crate::ui_state::{ActivePane, RightPane, UIState};
use regenerator2000_core::error::ViceError;
use regenerator2000_core::navigation;
use regenerator2000_core::state::types::System;
use regenerator2000_core::state::{Addr, AppState};
use regenerator2000_core::vice::state::{BreakpointKind, ViceBreakpoint};
use regenerator2000_core::vice::{
    self, ViceClient, ViceCommand, ViceConnectionId, ViceEvent, ViceMessage,
};
use std::sync::mpsc::{self, Sender};
use std::thread;

/// Connects to the VICE Binary Monitor at `addr` and attaches the client to `app_state`.
///
/// Starts a background thread to forward [`ViceEvent`] messages to `event_tx`.
///
/// # Errors
///
/// Returns [`ViceError`] if the TCP connection or socket configuration fails.
pub fn connect_vice_client(
    app_state: &mut AppState,
    addr: &str,
    event_tx: &Sender<AppEvent>,
) -> Result<(), ViceError> {
    let (vice_tx, vice_rx) = mpsc::channel();
    let client = ViceClient::connect(addr, vice_tx)?;
    let connection_id = client.connection_id();
    app_state.attach_vice_client(client);

    let app_tx = event_tx.clone();
    thread::spawn(move || {
        while let Ok(event) = vice_rx.recv() {
            if app_tx
                .send(AppEvent::Vice {
                    connection_id,
                    event,
                })
                .is_err()
            {
                break;
            }
        }
    });
    Ok(())
}

/// Processes a lifecycle or protocol event emitted by a VICE client session.
pub(super) fn handle_vice_event(
    connection_id: ViceConnectionId,
    vice_event: ViceEvent,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) {
    if !app_state.is_active_vice_connection(connection_id) {
        return;
    }
    match vice_event {
        ViceEvent::Connected => {
            app_state.vice_state.connected = true;
            ui_state.right_pane = RightPane::Debugger;
            ui_state.active_pane = ActivePane::Debugger;
            // Sync existing breakpoints from VICE (e.g. set in a previous session)
            if let Some(client) = &app_state.vice_client {
                client.send_checkpoint_list();
            }
            ui_state.set_status_message("Connected to VICE Monitor");
        }
        ViceEvent::Disconnected(reason) => {
            app_state.disconnect_vice();
            ui_state.clear_debugger_state();
            ui_state.set_status_message(format!("Disconnected from VICE: {reason}"));
        }
        ViceEvent::Message(msg) => {
            handle_vice_message(&msg, app_state, ui_state);
        }
    }
}

fn handle_vice_message(msg: &ViceMessage, app_state: &mut AppState, ui_state: &mut UIState) {
    if msg.command == ViceCommand::REGISTERS_GET {
        handle_vice_registers_get(msg, app_state, ui_state);
    } else if msg.command == ViceCommand::MEMORY_GET && msg.error_code == 0 {
        handle_vice_memory_get(msg, app_state);
    } else if (msg.command == ViceCommand::CHECKPOINT_SET
        || msg.command == ViceCommand::CHECKPOINT_GET)
        && msg.error_code == 0
    {
        handle_vice_checkpoint(msg, app_state);
    } else if msg.command == ViceCommand::CHECKPOINT_LIST && msg.error_code == 0 {
        // CHECKPOINT_LIST response: just a count (4 bytes).
        // VICE then sends individual CHECKPOINT_GET (0x11) responses for each
        // checkpoint — those are handled above. Clear the list here so the
        // incoming 0x11 responses repopulate it cleanly.
        app_state.vice_state.breakpoints.clear();
    } else if msg.command == ViceCommand::CHECKPOINT_DELETE && msg.error_code == 0 {
        let id = msg.request_id;
        app_state.vice_state.breakpoints.retain(|bp| bp.id != id);
    } else if msg.command == ViceCommand::RESUMED {
        app_state.vice_state.running = true;
        app_state.vice_state.stop_reason = None;
        app_state.vice_state.last_hit_checkpoint_id = None;
        ui_state.debugger_flash_remaining = 0;
    } else if msg.command == ViceCommand::STOPPED {
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
            ViceCommand::RESUMED | ViceCommand::EXIT_MONITOR
        );
        if !silent {
            ui_state.set_status_message(format!("VICE Msg: {:02x}", msg.command));
        }
    }
}

fn handle_vice_registers_get(msg: &ViceMessage, app_state: &mut AppState, ui_state: &mut UIState) {
    let Some(regs) = vice::parse_registers(&msg.payload) else {
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
    let pc_addr = Addr(pc);
    if !app_state.is_external(pc_addr)
        && let Some(idx) = navigation::resolve_jump_content_line_index(app_state, pc_addr)
    {
        let sub_idx =
            DisassemblyView::sub_index_for_address(&app_state.disassembly[idx], app_state, pc);
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
                    || (bp.kind == BreakpointKind::Exec && bp.address == pc)
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

fn format_checkpoint_stop_reason(id: u32, address: u16, kind: BreakpointKind) -> String {
    if kind == BreakpointKind::Exec {
        format!("Breakpoint #{id} at ${address:04X}")
    } else {
        format!("Watchpoint #{id} at ${address:04X} [{}]", kind.label())
    }
}

fn handle_vice_memory_get(msg: &ViceMessage, app_state: &mut AppState) {
    let Some(resp) = vice::parse_memory_get(&msg.payload) else {
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

fn handle_vice_checkpoint(msg: &ViceMessage, app_state: &mut AppState) {
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
    let Some(info) = vice::parse_checkpoint_info(&msg.payload) else {
        return;
    };

    let kind = BreakpointKind::from_cpu_op(info.cpu_op);

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
            app_state.vice_state.breakpoints.push(ViceBreakpoint {
                id: info.id,
                address: info.address,
                enabled: info.enabled,
                kind,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use regenerator2000_core::state::{BlockType, EnumDefinition, Label, LabelKind, LabelType};
    use std::collections::BTreeMap;

    #[test]
    fn vice_registers_get_locates_pc_with_external_labels_and_ignores_external_rom_pc() {
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

        let mut ui_state = UIState::new(Theme::default());
        let build_pc_msg = |pc: u16| {
            let mut payload = Vec::new();
            payload.extend_from_slice(&1u16.to_le_bytes());
            payload.extend_from_slice(&[3, 0x03]);
            payload.extend_from_slice(&pc.to_le_bytes());
            ViceMessage {
                command: ViceCommand::REGISTERS_GET,
                error_code: 0,
                request_id: 1,
                payload,
            }
        };

        // 1. In-range PC ($0802) must land on the JSR $FFD2 line despite prepended external labels.
        let expected_idx = app_state
            .line_index_containing_address(Addr(0x0802))
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

        // 3. Origin $0000 collapsed block preceded by enum preamble headers at $0000 must land on
        // the collapsed summary row and never step onto the preamble or past `!is_collapsed`.
        let mut collapsed_zero_state = AppState::new();
        collapsed_zero_state.origin = Addr::ZERO;
        // $0000: collapsed NOP, $0001: LDA #Colors.BLACK
        collapsed_zero_state.raw_data = vec![0xEA, 0xA9, 0x00];
        collapsed_zero_state.block_types = vec![BlockType::Code; 3];
        collapsed_zero_state.collapsed_blocks.push((0, 0));
        let mut variants = BTreeMap::new();
        variants.insert(0, "BLACK".to_string());
        collapsed_zero_state.enums.insert(
            "Colors".to_string(),
            EnumDefinition {
                name: "Colors".to_string(),
                description: None,
                variants,
                source_file: None,
            },
        );
        collapsed_zero_state.annotations.update(Addr(0x0001), |e| {
            e.enum_usage = Some("Colors".to_string());
        });
        collapsed_zero_state.settings.all_labels = true;
        collapsed_zero_state.disassemble();

        let mut collapsed_ui = UIState::new(Theme::default());
        handle_vice_registers_get(
            &build_pc_msg(0x0000),
            &mut collapsed_zero_state,
            &mut collapsed_ui,
        );
        assert!(collapsed_zero_state.disassembly[collapsed_ui.cursor_index].is_collapsed);
        assert_eq!(
            collapsed_zero_state.disassembly[collapsed_ui.cursor_index].address,
            Addr::ZERO
        );
    }
}
