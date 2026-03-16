use anyhow::{Result, anyhow};

pub const STX: u8 = 0x02;
pub const API_VERSION: u8 = 0x02; // Version 2

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViceMessage {
    pub command: u8,
    pub payload: Vec<u8>,
    pub request_id: u32,
    pub error_code: u8,
}

pub struct ViceCpuOp;

impl ViceCpuOp {
    pub const LOAD: u8 = 0x01; // Break on memory read
    pub const STORE: u8 = 0x02; // Break on memory write
    pub const EXEC: u8 = 0x04; // Break on execution
}

pub struct ViceCommand;

impl ViceCommand {
    pub const MEMORY_GET: u8 = 0x01;
    pub const MEMORY_SET: u8 = 0x02;
    pub const CHECKPOINT_GET: u8 = 0x11;
    pub const CHECKPOINT_SET: u8 = 0x12;
    pub const CHECKPOINT_DELETE: u8 = 0x13;
    pub const CHECKPOINT_LIST: u8 = 0x14;
    pub const REGISTERS_GET: u8 = 0x31;
    pub const ADVANCE_INSTRUCTION: u8 = 0x71;
    pub const EXECUTE_UNTIL_RETURN: u8 = 0x73;
    pub const EXIT_MONITOR: u8 = 0xaa; // Resume/continue execution (MON_CMD_EXIT)
    pub const PING: u8 = 0x81;

    // Push notifications from VICE (not request/response — VICE sends these unsolicited)
    pub const STOPPED: u8 = 0x62; // CPU stopped (checkpoint hit, step complete)
    pub const RESUMED: u8 = 0x65; // CPU resumed execution
}

impl ViceMessage {
    #[must_use]
    pub fn new(command: u8, payload: Vec<u8>) -> Self {
        Self {
            command,
            payload,
            request_id: 0,
            error_code: 0,
        }
    }

    #[must_use]
    pub fn with_id(command: u8, payload: Vec<u8>, request_id: u32) -> Self {
        Self {
            command,
            payload,
            request_id,
            error_code: 0,
        }
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let length = self.payload.len() as u32;
        let mut buf = Vec::with_capacity(11 + length as usize);
        buf.push(STX);
        buf.push(API_VERSION);
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&self.request_id.to_le_bytes());
        buf.push(self.command);
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn decode(buf: &[u8]) -> Result<Option<(Self, usize)>> {
        if buf.len() < 12 {
            return Ok(None);
        }
        if buf[0] != STX {
            return Err(anyhow!(
                "Invalid STX byte: expected 0x02, got 0x{:02x}",
                buf[0]
            ));
        }
        let length = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]) as usize;
        let total_size = 12 + length;

        if buf.len() < total_size {
            return Ok(None); // Need more data
        }

        let command = buf[6];
        let error_code = buf[7];
        let request_id = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let payload = buf[12..total_size].to_vec();

        Ok(Some((
            Self {
                command,
                payload,
                request_id,
                error_code,
            },
            total_size,
        )))
    }
}

// ---------------------------------------------------------------------------
// Typed response structs & parsing functions
// ---------------------------------------------------------------------------

/// Parsed register set from a REGISTERS_GET response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registers {
    pub a: Option<u8>,
    pub x: Option<u8>,
    pub y: Option<u8>,
    pub pc: Option<u16>,
    pub sp: Option<u8>,
    /// Status flags (processor flags register).
    pub p: Option<u8>,
}

/// Parse register items from a REGISTERS_GET response payload.
///
/// Payload format:
///   `ref_count` (2 LE) followed by `ref_count` register items, each:
///   `item_size` (1) `reg_id` (1) `value` (item_size - 1 bytes, LE)
///
/// Register IDs: 0x00=A, 0x01=X, 0x02=Y, 0x03=PC, 0x04=SP, 0x05=P.
#[must_use]
pub fn parse_registers(payload: &[u8]) -> Option<Registers> {
    if payload.len() < 2 {
        return None;
    }

    let ref_count = u16::from_le_bytes([payload[0], payload[1]]);
    let mut offset = 2;
    let mut regs = Registers::default();

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
                regs.a = Some(payload[offset + 2]);
            }
            0x01 if item_size >= 2 => {
                regs.x = Some(payload[offset + 2]);
            }
            0x02 if item_size >= 2 => {
                regs.y = Some(payload[offset + 2]);
            }
            0x03 if item_size >= 3 => {
                regs.pc = Some(u16::from_le_bytes([
                    payload[offset + 2],
                    payload[offset + 3],
                ]));
            }
            0x04 if item_size >= 2 => {
                regs.sp = Some(payload[offset + 2]);
            }
            0x05 if item_size >= 2 => {
                regs.p = Some(payload[offset + 2]);
            }
            _ => {}
        }

        offset += 1 + item_size;
    }

    Some(regs)
}

/// Parsed checkpoint info from a CHECKPOINT_SET / CHECKPOINT_GET response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointInfo {
    pub id: u32,
    /// True when this checkpoint was the one that just caused the CPU to stop.
    /// False for regular query/list responses.
    pub currently_hit: bool,
    pub address: u16,
    pub enabled: bool,
    pub cpu_op: u8,
    pub temporary: bool,
}

/// Parse checkpoint info from a CHECKPOINT_SET (0x12) or CHECKPOINT_GET (0x11) payload.
///
/// Payload format (at least 13 bytes):
///   `CN`(4 LE) `CH`(1) `SA`(2 LE) `EA`(2 LE) `ST`(1) `EN`(1) `OP`(1) `TM`(1) ...
///
/// Fields:
///   CN = checkpoint number (id), CH = currently hit flag,
///   SA/EA = start/end address, ST = stop when hit, EN = enabled,
///   OP = cpu operation (exec/load/store), TM = temporary.
#[must_use]
pub fn parse_checkpoint_info(payload: &[u8]) -> Option<CheckpointInfo> {
    if payload.len() < 13 {
        return None;
    }

    let id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let currently_hit = payload[4] != 0;
    let addr = u16::from_le_bytes([payload[5], payload[6]]);
    let enabled = payload[10] != 0;
    let cpu_op = payload[11];
    let temporary = payload[12] != 0;

    Some(CheckpointInfo {
        id,
        currently_hit,
        address: addr,
        enabled,
        cpu_op,
        temporary,
    })
}

/// Parsed response from a MEMORY_GET command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGetResponse {
    pub bytes: Vec<u8>,
}

/// Parse a MEMORY_GET response payload.
///
/// Payload format: `length` (2 LE) followed by `length` bytes of memory data.
#[must_use]
pub fn parse_memory_get(payload: &[u8]) -> Option<MemoryGetResponse> {
    if payload.len() < 2 {
        return None;
    }

    let mem_len = u16::from_le_bytes([payload[0], payload[1]]) as usize;
    if payload.len() < 2 + mem_len || mem_len == 0 {
        return None;
    }

    Some(MemoryGetResponse {
        bytes: payload[2..2 + mem_len].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // ViceMessage encode / decode round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn decode_valid_response() {
        // Build a valid response frame manually:
        // STX(1) + api_version(1) + length(4 LE) + command(1) + error_code(1)
        // + request_id(4 LE) + payload
        let payload = vec![1u8, 2, 3];
        let length = payload.len() as u32;
        let mut buf = Vec::new();
        buf.push(STX); // 0x02
        buf.push(API_VERSION); // api version
        buf.extend_from_slice(&length.to_le_bytes()); // 3, 0, 0, 0
        buf.push(ViceCommand::MEMORY_GET); // command
        buf.push(0x00); // error_code
        buf.extend_from_slice(&42u32.to_le_bytes()); // request_id
        buf.extend_from_slice(&payload);

        let (decoded, size) = ViceMessage::decode(&buf).unwrap().unwrap();
        assert_eq!(size, buf.len());
        assert_eq!(decoded.command, ViceCommand::MEMORY_GET);
        assert_eq!(decoded.request_id, 42);
        assert_eq!(decoded.error_code, 0);
        assert_eq!(decoded.payload, vec![1, 2, 3]);
    }

    #[test]
    fn encode_produces_valid_request_frame() {
        let msg = ViceMessage::with_id(ViceCommand::REGISTERS_GET, vec![0], 5);
        let encoded = msg.encode();
        // STX + API_VERSION + length(4) + request_id(4) + command + payload
        assert_eq!(encoded[0], STX);
        assert_eq!(encoded[1], API_VERSION);
        let length = u32::from_le_bytes([encoded[2], encoded[3], encoded[4], encoded[5]]);
        assert_eq!(length, 1); // payload is [0]
        let req_id = u32::from_le_bytes([encoded[6], encoded[7], encoded[8], encoded[9]]);
        assert_eq!(req_id, 5);
        assert_eq!(encoded[10], ViceCommand::REGISTERS_GET);
        assert_eq!(encoded[11], 0); // payload byte
    }

    #[test]
    fn decode_incomplete_returns_none() {
        assert!(ViceMessage::decode(&[0x02, 0x02]).unwrap().is_none());
    }

    #[test]
    fn decode_bad_stx_returns_error() {
        let mut buf = vec![0xFF; 12];
        buf[0] = 0xFF;
        assert!(ViceMessage::decode(&buf).is_err());
    }

    // -----------------------------------------------------------------------
    // parse_registers
    // -----------------------------------------------------------------------

    #[test]
    fn parse_registers_empty_payload() {
        assert!(parse_registers(&[]).is_none());
    }

    #[test]
    fn parse_registers_single_byte_payload() {
        assert!(parse_registers(&[0x01]).is_none());
    }

    #[test]
    fn parse_registers_all_regs() {
        // Build a register payload with 6 items (A, X, Y, PC, SP, P)
        let mut payload = Vec::new();
        payload.extend_from_slice(&6u16.to_le_bytes()); // ref_count = 6

        // A = 0x42 (reg_id=0x00, item_size=2)
        payload.push(2); // item_size
        payload.push(0x00); // reg_id A
        payload.push(0x42); // value

        // X = 0x10 (reg_id=0x01, item_size=2)
        payload.push(2);
        payload.push(0x01);
        payload.push(0x10);

        // Y = 0x20 (reg_id=0x02, item_size=2)
        payload.push(2);
        payload.push(0x02);
        payload.push(0x20);

        // PC = 0xC000 (reg_id=0x03, item_size=3)
        payload.push(3);
        payload.push(0x03);
        payload.extend_from_slice(&0xC000u16.to_le_bytes());

        // SP = 0xFF (reg_id=0x04, item_size=2)
        payload.push(2);
        payload.push(0x04);
        payload.push(0xFF);

        // P = 0x24 (reg_id=0x05, item_size=2)
        payload.push(2);
        payload.push(0x05);
        payload.push(0x24);

        let regs = parse_registers(&payload).unwrap();
        assert_eq!(regs.a, Some(0x42));
        assert_eq!(regs.x, Some(0x10));
        assert_eq!(regs.y, Some(0x20));
        assert_eq!(regs.pc, Some(0xC000));
        assert_eq!(regs.sp, Some(0xFF));
        assert_eq!(regs.p, Some(0x24));
    }

    #[test]
    fn parse_registers_partial() {
        // Only A and PC
        let mut payload = Vec::new();
        payload.extend_from_slice(&2u16.to_le_bytes());

        payload.push(2);
        payload.push(0x00);
        payload.push(0xAA);

        payload.push(3);
        payload.push(0x03);
        payload.extend_from_slice(&0x0800u16.to_le_bytes());

        let regs = parse_registers(&payload).unwrap();
        assert_eq!(regs.a, Some(0xAA));
        assert_eq!(regs.pc, Some(0x0800));
        assert_eq!(regs.x, None);
        assert_eq!(regs.y, None);
        assert_eq!(regs.sp, None);
        assert_eq!(regs.p, None);
    }

    #[test]
    fn parse_registers_truncated_item() {
        // ref_count says 2 but only 1 complete item
        let mut payload = Vec::new();
        payload.extend_from_slice(&2u16.to_le_bytes());

        // Complete item
        payload.push(2);
        payload.push(0x00);
        payload.push(0x42);

        // Truncated item (item_size says 3 but only 1 byte follows)
        payload.push(3);
        payload.push(0x03);
        // missing value bytes

        let regs = parse_registers(&payload).unwrap();
        assert_eq!(regs.a, Some(0x42));
        assert_eq!(regs.pc, None); // truncated, not parsed
    }

    #[test]
    fn parse_registers_unknown_reg_id() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u16.to_le_bytes());

        payload.push(2);
        payload.push(0xFF); // unknown reg id
        payload.push(0x42);

        let regs = parse_registers(&payload).unwrap();
        assert_eq!(regs, Registers::default());
    }

    // -----------------------------------------------------------------------
    // parse_checkpoint_info
    // -----------------------------------------------------------------------

    #[test]
    fn parse_checkpoint_info_empty() {
        assert!(parse_checkpoint_info(&[]).is_none());
    }

    #[test]
    fn parse_checkpoint_info_too_short() {
        assert!(parse_checkpoint_info(&[0; 12]).is_none());
    }

    #[test]
    fn parse_checkpoint_info_exec_breakpoint() {
        let mut p = Vec::new();
        p.extend_from_slice(&1u32.to_le_bytes()); // id = 1           [0..4]
        p.push(0); // currently_hit = false                            [4]
        p.extend_from_slice(&0xC000u16.to_le_bytes()); // start_addr   [5..7]
        p.extend_from_slice(&0xC000u16.to_le_bytes()); // end_addr     [7..9]
        p.push(1); // stop_when_hit                                    [9]
        p.push(1); // enabled                                          [10]
        p.push(0x04); // cpu_op = EXEC                                 [11]
        p.push(0); // temporary = false                                [12]

        let info = parse_checkpoint_info(&p).unwrap();
        assert_eq!(info.id, 1);
        assert!(!info.currently_hit);
        assert_eq!(info.address, 0xC000);
        assert!(info.enabled);
        assert_eq!(info.cpu_op, 0x04);
        assert!(!info.temporary);
    }

    #[test]
    fn parse_checkpoint_info_currently_hit() {
        let mut p = Vec::new();
        p.extend_from_slice(&3u32.to_le_bytes()); // id = 3           [0..4]
        p.push(1); // currently_hit = true                             [4]
        p.extend_from_slice(&0xD015u16.to_le_bytes()); // start_addr   [5..7]
        p.extend_from_slice(&0xD015u16.to_le_bytes()); // end_addr     [7..9]
        p.push(1); // stop_when_hit                                    [9]
        p.push(1); // enabled                                          [10]
        p.push(0x01); // cpu_op = LOAD (read watchpoint)               [11]
        p.push(0); // temporary = false                                [12]

        let info = parse_checkpoint_info(&p).unwrap();
        assert_eq!(info.id, 3);
        assert!(info.currently_hit);
        assert_eq!(info.address, 0xD015);
        assert_eq!(info.cpu_op, 0x01);
    }

    #[test]
    fn parse_checkpoint_info_temporary() {
        let mut p = vec![0u8; 13];
        p[0..4].copy_from_slice(&7u32.to_le_bytes());
        // p[4] = 0 (currently_hit = false)
        p[10] = 1; // enabled
        p[11] = 0x04; // cpu_op = EXEC
        p[12] = 1; // temporary

        let info = parse_checkpoint_info(&p).unwrap();
        assert_eq!(info.id, 7);
        assert!(!info.currently_hit);
        assert!(info.temporary);
    }

    // -----------------------------------------------------------------------
    // parse_memory_get
    // -----------------------------------------------------------------------

    #[test]
    fn parse_memory_get_empty() {
        assert!(parse_memory_get(&[]).is_none());
    }

    #[test]
    fn parse_memory_get_zero_length() {
        assert!(parse_memory_get(&[0, 0]).is_none());
    }

    #[test]
    fn parse_memory_get_valid() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&4u16.to_le_bytes());
        payload.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);

        let resp = parse_memory_get(&payload).unwrap();
        assert_eq!(resp.bytes, vec![0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn parse_memory_get_truncated() {
        // Length says 10 but only 3 bytes follow
        let mut payload = Vec::new();
        payload.extend_from_slice(&10u16.to_le_bytes());
        payload.extend_from_slice(&[0x01, 0x02, 0x03]);

        assert!(parse_memory_get(&payload).is_none());
    }
}
