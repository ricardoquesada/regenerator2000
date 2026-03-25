#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// VICE binary protocol unit tests
///
/// Tests the encode/decode round-trip, edge cases, and error handling
/// for the `ViceMessage` protocol, as well as `ViceState` and `BreakpointKind` logic.
mod protocol_tests {
    use regenerator2000_core::vice::protocol::{
        API_VERSION, STX, ViceCommand, ViceCpuOp, ViceMessage,
    };

    #[test]
    fn encode_ping_message() {
        let msg = ViceMessage::new(ViceCommand::PING, vec![]);
        let buf = msg.encode();
        // Format: STX(1) + API_VERSION(1) + LEN(4) + REQ_ID(4) + CMD(1) + payload
        assert_eq!(buf.len(), 11); // 11 + 0 payload
        assert_eq!(buf[0], STX);
        assert_eq!(buf[1], API_VERSION);
        // Length = 0 (LE)
        assert_eq!(&buf[2..6], &[0, 0, 0, 0]);
        // Request ID = 0 (LE)
        assert_eq!(&buf[6..10], &[0, 0, 0, 0]);
        // Command
        assert_eq!(buf[10], ViceCommand::PING);
    }

    #[test]
    fn encode_message_with_payload() {
        let payload = vec![0x01, 0x02, 0x03, 0x04];
        let msg = ViceMessage::with_id(ViceCommand::MEMORY_GET, payload.clone(), 42);
        let buf = msg.encode();
        assert_eq!(buf.len(), 11 + 4);
        // Length = 4 (LE)
        assert_eq!(&buf[2..6], &[4, 0, 0, 0]);
        // Request ID = 42 (LE)
        assert_eq!(&buf[6..10], &[42, 0, 0, 0]);
        // Command
        assert_eq!(buf[10], ViceCommand::MEMORY_GET);
        // Payload
        assert_eq!(&buf[11..], &payload[..]);
    }

    #[test]
    fn decode_valid_message() {
        // Build a raw response buffer
        let mut buf = Vec::new();
        buf.push(STX); // [0]
        buf.push(API_VERSION); // [1]
        buf.extend_from_slice(&3u32.to_le_bytes()); // [2..6] length = 3
        buf.push(ViceCommand::PING); // [6] command
        buf.push(0x00); // [7] error_code
        buf.extend_from_slice(&99u32.to_le_bytes()); // [8..12] request_id
        buf.extend_from_slice(&[0xAA, 0xBB, 0xCC]); // [12..15] payload

        let result = ViceMessage::decode(&buf).unwrap();
        assert!(result.is_some());
        let (msg, consumed) = result.unwrap();
        assert_eq!(consumed, 15); // 12 header + 3 payload
        assert_eq!(msg.command, ViceCommand::PING);
        assert_eq!(msg.error_code, 0x00);
        assert_eq!(msg.request_id, 99);
        assert_eq!(msg.payload, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn decode_too_short_returns_none() {
        // Less than 12 bytes => not enough for header
        let buf = vec![STX, API_VERSION, 0, 0, 0, 0];
        let result = ViceMessage::decode(&buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn decode_incomplete_payload_returns_none() {
        // Header says 10 bytes of payload, but we only have 3
        let mut buf = Vec::new();
        buf.push(STX);
        buf.push(API_VERSION);
        buf.extend_from_slice(&10u32.to_le_bytes()); // claims 10 bytes payload
        buf.push(ViceCommand::PING);
        buf.push(0x00);
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&[0xAA, 0xBB, 0xCC]); // only 3 bytes

        let result = ViceMessage::decode(&buf).unwrap();
        assert!(result.is_none()); // need more data
    }

    #[test]
    fn decode_invalid_stx_returns_error() {
        let mut buf = vec![0xFF; 20]; // starts with 0xFF, not STX
        buf[1] = API_VERSION;
        // Make it long enough
        buf[2..6].copy_from_slice(&0u32.to_le_bytes());

        let result = ViceMessage::decode(&buf);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid STX"));
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = ViceMessage::with_id(
            ViceCommand::CHECKPOINT_SET,
            vec![0x00, 0x10, 0x00, 0x10, 1, 1, 4, 0],
            12345,
        );
        let encoded = original.encode();

        // The decode format has a different header layout:
        // encode: STX, API_VERSION, LEN(4), REQ_ID(4), CMD, PAYLOAD
        // decode: STX, ?, LEN(4), CMD, ERR, REQ_ID(4), PAYLOAD
        // So encode/decode are NOT direct inverses — they represent
        // request (encode) vs response (decode) formats.
        // This is by design in the VICE binary protocol.

        // Verify encode produces expected length
        assert_eq!(encoded.len(), 11 + 8);

        // Verify the encoded request_id at bytes 6-9
        let req_id = u32::from_le_bytes([encoded[6], encoded[7], encoded[8], encoded[9]]);
        assert_eq!(req_id, 12345);
    }

    #[test]
    fn decode_zero_length_payload() {
        let mut buf = Vec::new();
        buf.push(STX);
        buf.push(API_VERSION);
        buf.extend_from_slice(&0u32.to_le_bytes()); // length = 0
        buf.push(ViceCommand::EXIT_MONITOR);
        buf.push(0x00); // error
        buf.extend_from_slice(&77u32.to_le_bytes());

        let result = ViceMessage::decode(&buf).unwrap();
        assert!(result.is_some());
        let (msg, consumed) = result.unwrap();
        assert_eq!(consumed, 12);
        assert_eq!(msg.payload.len(), 0);
        assert_eq!(msg.command, ViceCommand::EXIT_MONITOR);
    }

    #[test]
    fn decode_multiple_messages_in_buffer() {
        // Two messages concatenated
        let mut buf = Vec::new();

        // Message 1: PING with empty payload
        buf.push(STX);
        buf.push(API_VERSION);
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.push(ViceCommand::PING);
        buf.push(0x00);
        buf.extend_from_slice(&1u32.to_le_bytes());

        // Message 2: STOPPED with 2-byte payload
        buf.push(STX);
        buf.push(API_VERSION);
        buf.extend_from_slice(&2u32.to_le_bytes());
        buf.push(ViceCommand::STOPPED);
        buf.push(0x00);
        buf.extend_from_slice(&2u32.to_le_bytes());
        buf.extend_from_slice(&[0xDE, 0xAD]);

        // Decode first
        let (msg1, size1) = ViceMessage::decode(&buf).unwrap().unwrap();
        assert_eq!(msg1.command, ViceCommand::PING);
        assert_eq!(size1, 12);

        // Decode second
        let (msg2, size2) = ViceMessage::decode(&buf[size1..]).unwrap().unwrap();
        assert_eq!(msg2.command, ViceCommand::STOPPED);
        assert_eq!(size2, 14);
        assert_eq!(msg2.payload, vec![0xDE, 0xAD]);
    }

    #[test]
    fn vice_command_constants() {
        // Verify protocol constants match expected values
        assert_eq!(ViceCommand::MEMORY_GET, 0x01);
        assert_eq!(ViceCommand::MEMORY_SET, 0x02);
        assert_eq!(ViceCommand::CHECKPOINT_SET, 0x12);
        assert_eq!(ViceCommand::CHECKPOINT_DELETE, 0x13);
        assert_eq!(ViceCommand::REGISTERS_GET, 0x31);
        assert_eq!(ViceCommand::ADVANCE_INSTRUCTION, 0x71);
        assert_eq!(ViceCommand::EXIT_MONITOR, 0xAA);
        assert_eq!(ViceCommand::PING, 0x81);
        assert_eq!(ViceCommand::STOPPED, 0x62);
        assert_eq!(ViceCommand::RESUMED, 0x65);
    }

    #[test]
    fn vice_cpu_op_constants() {
        assert_eq!(ViceCpuOp::LOAD, 0x01);
        assert_eq!(ViceCpuOp::STORE, 0x02);
        assert_eq!(ViceCpuOp::EXEC, 0x04);
        // Combined
        assert_eq!(ViceCpuOp::LOAD | ViceCpuOp::STORE, 0x03);
    }
}

mod vice_state_tests {
    use regenerator2000_core::vice::state::{BreakpointKind, ViceBreakpoint, ViceState};

    #[test]
    fn breakpoint_kind_from_cpu_op() {
        assert_eq!(BreakpointKind::from_cpu_op(0x01), BreakpointKind::Load);
        assert_eq!(BreakpointKind::from_cpu_op(0x02), BreakpointKind::Store);
        assert_eq!(BreakpointKind::from_cpu_op(0x03), BreakpointKind::LoadStore);
        assert_eq!(BreakpointKind::from_cpu_op(0x04), BreakpointKind::Exec);
        // Unknown values default to Exec
        assert_eq!(BreakpointKind::from_cpu_op(0x00), BreakpointKind::Exec);
        assert_eq!(BreakpointKind::from_cpu_op(0xFF), BreakpointKind::Exec);
    }

    #[test]
    fn breakpoint_kind_labels() {
        assert_eq!(BreakpointKind::Exec.label(), "X");
        assert_eq!(BreakpointKind::Load.label(), "R");
        assert_eq!(BreakpointKind::Store.label(), "W");
        assert_eq!(BreakpointKind::LoadStore.label(), "RW");
    }

    #[test]
    fn vice_state_new_defaults() {
        let state = ViceState::new();
        assert!(!state.connected);
        assert!(state.pc.is_none());
        assert!(state.a.is_none());
        assert!(state.x.is_none());
        assert!(state.y.is_none());
        assert!(state.sp.is_none());
        assert!(state.p.is_none());
        assert_eq!(state.status, "Disconnected");
        assert!(!state.running);
        assert!(state.live_memory.is_none());
        assert!(state.stack_memory.is_none());
        assert!(state.io_memory.is_none());
        assert!(state.breakpoints.is_empty());
        assert!(state.temporary_breakpoints.is_empty());
        assert!(state.stop_reason.is_none());
    }

    #[test]
    fn vice_state_reset_registers() {
        let mut state = ViceState::new();
        state.connected = true;
        state.pc = Some(0x1234);
        state.a = Some(0x42);
        state.x = Some(0x10);
        state.y = Some(0x20);
        state.sp = Some(0xFF);
        state.p = Some(0x30);
        state.running = true;
        state.live_memory = Some(vec![0; 256]);
        state.stack_memory = Some(vec![0; 256]);
        state.io_memory = Some(vec![0; 4096]);
        state.breakpoints.push(ViceBreakpoint {
            id: 1,
            address: 0x1000,
            enabled: true,
            kind: BreakpointKind::Exec,
        });
        state.temporary_breakpoints.push(99);
        state.stop_reason = Some("Test".to_string());

        state.reset_registers();

        // All registers should be None
        assert!(state.pc.is_none());
        assert!(state.a.is_none());
        assert!(state.x.is_none());
        assert!(state.y.is_none());
        assert!(state.sp.is_none());
        assert!(state.p.is_none());
        assert!(!state.running);
        assert!(state.live_memory.is_none());
        assert!(state.stack_memory.is_none());
        assert!(state.io_memory.is_none());
        assert!(state.breakpoints.is_empty());
        assert!(state.temporary_breakpoints.is_empty());
        assert!(state.stop_reason.is_none());
        // Note: `connected` is NOT reset by reset_registers
    }

    #[test]
    fn has_breakpoint_at() {
        let mut state = ViceState::new();
        assert!(!state.has_breakpoint_at(0x1000));

        state.breakpoints.push(ViceBreakpoint {
            id: 1,
            address: 0x1000,
            enabled: true,
            kind: BreakpointKind::Exec,
        });
        state.breakpoints.push(ViceBreakpoint {
            id: 2,
            address: 0x2000,
            enabled: false,
            kind: BreakpointKind::Store,
        });

        assert!(state.has_breakpoint_at(0x1000));
        assert!(state.has_breakpoint_at(0x2000));
        assert!(!state.has_breakpoint_at(0x3000));
    }

    #[test]
    fn vice_state_default_trait() {
        let state = ViceState::default();
        assert!(!state.connected);
        assert!(state.pc.is_none());
    }

    #[test]
    fn vice_state_snapshot_behavior() {
        // simulate the sequence of events
        // 1. start
        let mut state = ViceState::new();
        state.a = Some(0x20);

        // 2. STOPPED occurs
        state.previous = Some(state.snapshot());
        assert_eq!(state.previous.as_ref().unwrap().a, Some(0x20));
        assert!(state.previous.as_ref().unwrap().previous.is_none());

        // 3. REGISTERS_GET occurs
        state.a = Some(0x30);
        assert_eq!(state.previous.as_ref().unwrap().a, Some(0x20)); // previous stays old

        // 4. RESUMED occurs
        // (no-op affecting previous)
        assert_eq!(state.previous.as_ref().unwrap().a, Some(0x20)); // previous still old

        // 5. STOPPED occurs again
        state.previous = Some(state.snapshot());
        assert_eq!(state.previous.as_ref().unwrap().a, Some(0x30)); // previous becomes the new old

        // 6. REGISTERS_GET occurs again
        state.a = Some(0x40);
        assert_eq!(state.previous.as_ref().unwrap().a, Some(0x30)); // previous stays old
    }
}

