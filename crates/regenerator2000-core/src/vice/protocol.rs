//! VICE Binary Monitor v2 frame encoding, frame decoding, and response parsers.

use crate::error::ViceError;

/// Start-of-transmission marker byte (`0x02`) for VICE Binary Monitor frames.
pub const STX: u8 = 0x02;

/// Binary monitor protocol version byte (`0x02`).
pub const API_VERSION: u8 = 0x02;

/// Maximum accepted response payload size (1 MiB) before [`ViceMessage::decode`] rejects the frame.
pub const MAX_PAYLOAD_LEN: usize = 1024 * 1024;

/// Binary monitor command or response frame exchanged with VICE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViceMessage {
    /// Command or response type code (see [`ViceCommand`]).
    pub command: u8,
    /// Raw command arguments or response body bytes.
    pub payload: Vec<u8>,
    /// Client-assigned correlation identifier echoed by VICE in responses.
    pub request_id: u32,
    /// Response status code (`0x00` indicates success).
    pub error_code: u8,
}

/// Bitmask constants for the checkpoint `cpu_operation` field.
pub struct ViceCpuOp;

impl ViceCpuOp {
    /// Stops when the CPU reads from the watched address.
    pub const LOAD: u8 = 0x01;
    /// Stops when the CPU writes to the watched address.
    pub const STORE: u8 = 0x02;
    /// Stops when the CPU executes an instruction at the address.
    pub const EXEC: u8 = 0x04;
}

/// Command and push-notification opcodes for the VICE Binary Monitor v2 protocol.
pub struct ViceCommand;

impl ViceCommand {
    /// Reads a range of bytes from emulated memory.
    pub const MEMORY_GET: u8 = 0x01;
    /// Writes a range of bytes to emulated memory.
    pub const MEMORY_SET: u8 = 0x02;
    /// Queries or reports a single checkpoint.
    pub const CHECKPOINT_GET: u8 = 0x11;
    /// Creates a new breakpoint or watchpoint.
    pub const CHECKPOINT_SET: u8 = 0x12;
    /// Deletes an existing checkpoint by ID.
    pub const CHECKPOINT_DELETE: u8 = 0x13;
    /// Lists all registered checkpoints.
    pub const CHECKPOINT_LIST: u8 = 0x14;
    /// Reads CPU register values.
    pub const REGISTERS_GET: u8 = 0x31;
    /// Steps the CPU by a given number of instructions.
    pub const ADVANCE_INSTRUCTION: u8 = 0x71;
    /// Executes instructions until the current subroutine returns.
    pub const EXECUTE_UNTIL_RETURN: u8 = 0x73;
    /// Exits the monitor and resumes normal emulation (`MON_CMD_EXIT`).
    pub const EXIT_MONITOR: u8 = 0xaa;
    /// Pings the monitor connection.
    pub const PING: u8 = 0x81;

    /// Unsolicited notification sent when the emulated CPU stops.
    pub const STOPPED: u8 = 0x62;
    /// Unsolicited notification sent when the emulated CPU resumes execution.
    pub const RESUMED: u8 = 0x65;
}

impl ViceMessage {
    /// Creates a request message with `request_id` and `error_code` set to `0`.
    #[must_use]
    pub fn new(command: u8, payload: Vec<u8>) -> Self {
        Self {
            command,
            payload,
            request_id: 0,
            error_code: 0,
        }
    }

    /// Creates a request message with an explicit `request_id`.
    #[must_use]
    pub fn with_id(command: u8, payload: Vec<u8>, request_id: u32) -> Self {
        Self {
            command,
            payload,
            request_id,
            error_code: 0,
        }
    }

    /// Encodes this message into an 11-byte VICE request header followed by `payload`.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let length = self.payload.len() as u32;
        let mut buf = Vec::with_capacity(11 + self.payload.len());
        buf.push(STX);
        buf.push(API_VERSION);
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&self.request_id.to_le_bytes());
        buf.push(self.command);
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Decodes the next complete VICE response frame from `buf`.
    ///
    /// Returns `Ok(None)` if `buf` holds fewer than 12 header bytes or fewer than
    /// `12 + payload_length` total bytes. Returns `Ok(Some((message, consumed_bytes)))`
    /// when a complete frame is available.
    ///
    /// # Errors
    ///
    /// Returns [`ViceError::ProtocolFraming`] if `buf[0]` is not [`STX`] or if the
    /// declared payload length exceeds [`MAX_PAYLOAD_LEN`].
    pub fn decode(buf: &[u8]) -> Result<Option<(Self, usize)>, ViceError> {
        if buf.len() < 12 {
            return Ok(None);
        }
        if buf[0] != STX {
            return Err(ViceError::ProtocolFraming(format!(
                "Invalid STX byte: expected 0x02, got 0x{:02x}",
                buf[0]
            )));
        }
        let length = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]) as usize;
        if length > MAX_PAYLOAD_LEN {
            return Err(ViceError::ProtocolFraming(format!(
                "Payload length {length} exceeds maximum {MAX_PAYLOAD_LEN} bytes"
            )));
        }
        let total_size = 12 + length;

        if buf.len() < total_size {
            return Ok(None);
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

/// CPU register values parsed from a [`ViceCommand::REGISTERS_GET`] response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registers {
    /// Accumulator (`A`, register ID `0x00`).
    pub a: Option<u8>,
    /// X index (`X`, register ID `0x01`).
    pub x: Option<u8>,
    /// Y index (`Y`, register ID `0x02`).
    pub y: Option<u8>,
    /// Program Counter (`PC`, register ID `0x03`).
    pub pc: Option<u16>,
    /// Stack Pointer (`SP`, register ID `0x04`).
    pub sp: Option<u8>,
    /// Processor status flags (`P`, register ID `0x05`).
    pub p: Option<u8>,
}

/// Parses CPU register items from a [`ViceCommand::REGISTERS_GET`] response payload.
///
/// The payload begins with a 16-bit little-endian item count, followed by items of the form
/// `[item_size, reg_id, value_bytes...]`, where `item_size` counts `reg_id` plus the value bytes.
/// Returns `None` if `payload` is shorter than 2 bytes.
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
        if item_size == 0 || offset + 1 + item_size > payload.len() {
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

/// Checkpoint descriptor parsed from a [`ViceCommand::CHECKPOINT_SET`] or [`ViceCommand::CHECKPOINT_GET`] response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointInfo {
    /// Checkpoint number assigned by VICE.
    pub id: u32,
    /// True when this checkpoint caused the current CPU stop.
    pub currently_hit: bool,
    /// Start address of the checkpoint.
    pub address: u16,
    /// True when the checkpoint is enabled.
    pub enabled: bool,
    /// Bitmask of [`ViceCpuOp`] flags (`EXEC`, `LOAD`, `STORE`).
    pub cpu_op: u8,
    /// True when VICE deletes this checkpoint automatically after the first hit.
    pub temporary: bool,
}

/// Parses checkpoint metadata from a [`ViceCommand::CHECKPOINT_SET`] or [`ViceCommand::CHECKPOINT_GET`] payload.
///
/// Expects at least 13 bytes: `id` (4 LE), `currently_hit` (1), `start_addr` (2 LE),
/// `end_addr` (2 LE), `stop_when_hit` (1), `enabled` (1), `cpu_op` (1), and `temporary` (1).
/// Returns `None` if `payload` is shorter than 13 bytes.
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

/// Memory bytes parsed from a [`ViceCommand::MEMORY_GET`] response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGetResponse {
    /// Raw memory bytes returned by VICE.
    pub bytes: Vec<u8>,
}

/// Parses a [`ViceCommand::MEMORY_GET`] response payload.
///
/// The payload begins with a 16-bit little-endian byte length followed by that many memory bytes.
/// Returns `None` if the declared length is `0` or if `payload` is truncated.
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

    #[test]
    fn decode_valid_response() {
        let payload = vec![1u8, 2, 3];
        let length = payload.len() as u32;
        let mut buf = Vec::new();
        buf.push(STX);
        buf.push(API_VERSION);
        buf.extend_from_slice(&length.to_le_bytes());
        buf.push(ViceCommand::MEMORY_GET);
        buf.push(0x00);
        buf.extend_from_slice(&42u32.to_le_bytes());
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
        assert_eq!(encoded[0], STX);
        assert_eq!(encoded[1], API_VERSION);
        let length = u32::from_le_bytes([encoded[2], encoded[3], encoded[4], encoded[5]]);
        assert_eq!(length, 1);
        let req_id = u32::from_le_bytes([encoded[6], encoded[7], encoded[8], encoded[9]]);
        assert_eq!(req_id, 5);
        assert_eq!(encoded[10], ViceCommand::REGISTERS_GET);
        assert_eq!(encoded[11], 0);
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

    #[test]
    fn decode_oversized_length_returns_error() {
        let mut buf = vec![0u8; 12];
        buf[0] = STX;
        buf[1] = API_VERSION;
        let oversized = (MAX_PAYLOAD_LEN as u32) + 1;
        buf[2..6].copy_from_slice(&oversized.to_le_bytes());
        assert!(ViceMessage::decode(&buf).is_err());
    }

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
        let mut payload = Vec::new();
        payload.extend_from_slice(&6u16.to_le_bytes());

        payload.extend_from_slice(&[2, 0x00, 0x42]);
        payload.extend_from_slice(&[2, 0x01, 0x10]);
        payload.extend_from_slice(&[2, 0x02, 0x20]);
        payload.extend_from_slice(&[3, 0x03]);
        payload.extend_from_slice(&0xC000u16.to_le_bytes());
        payload.extend_from_slice(&[2, 0x04, 0xFF]);
        payload.extend_from_slice(&[2, 0x05, 0x24]);

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
        let mut payload = Vec::new();
        payload.extend_from_slice(&2u16.to_le_bytes());
        payload.extend_from_slice(&[2, 0x00, 0xAA]);
        payload.extend_from_slice(&[3, 0x03]);
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
        let mut payload = Vec::new();
        payload.extend_from_slice(&2u16.to_le_bytes());
        payload.extend_from_slice(&[2, 0x00, 0x42]);
        payload.extend_from_slice(&[3, 0x03]);

        let regs = parse_registers(&payload).unwrap();
        assert_eq!(regs.a, Some(0x42));
        assert_eq!(regs.pc, None);
    }

    #[test]
    fn parse_registers_zero_item_size_does_not_panic() {
        // item_size == 0 at end of slice previously caused an out-of-bounds panic at payload[offset + 1].
        let payload = [1u8, 0, 0];
        let regs = parse_registers(&payload).unwrap();
        assert_eq!(regs, Registers::default());

        // item_size == 0 after a valid item stops parsing cleanly without corrupting prior items.
        let payload_with_valid = [2u8, 0, 2, 0x00, 0x42, 0, 2, 0x01, 0x99];
        let regs2 = parse_registers(&payload_with_valid).unwrap();
        assert_eq!(regs2.a, Some(0x42));
        assert_eq!(regs2.x, None);
    }

    #[test]
    fn parse_registers_unknown_reg_id() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u16.to_le_bytes());
        payload.extend_from_slice(&[2, 0xFF, 0x42]);

        let regs = parse_registers(&payload).unwrap();
        assert_eq!(regs, Registers::default());
    }

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
        p.extend_from_slice(&1u32.to_le_bytes());
        p.push(0);
        p.extend_from_slice(&0xC000u16.to_le_bytes());
        p.extend_from_slice(&0xC000u16.to_le_bytes());
        p.push(1);
        p.push(1);
        p.push(0x04);
        p.push(0);

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
        p.extend_from_slice(&3u32.to_le_bytes());
        p.push(1);
        p.extend_from_slice(&0xD015u16.to_le_bytes());
        p.extend_from_slice(&0xD015u16.to_le_bytes());
        p.push(1);
        p.push(1);
        p.push(0x01);
        p.push(0);

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
        p[10] = 1;
        p[11] = 0x04;
        p[12] = 1;

        let info = parse_checkpoint_info(&p).unwrap();
        assert_eq!(info.id, 7);
        assert!(!info.currently_hit);
        assert!(info.temporary);
    }

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
        let mut payload = Vec::new();
        payload.extend_from_slice(&10u16.to_le_bytes());
        payload.extend_from_slice(&[0x01, 0x02, 0x03]);

        assert!(parse_memory_get(&payload).is_none());
    }
}
