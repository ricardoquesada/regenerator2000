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

pub struct ViceCommand;

impl ViceCommand {
    pub const MEMORY_GET: u8 = 0x01;
    pub const MEMORY_SET: u8 = 0x02;
    pub const CHECKPOINT_GET: u8 = 0x11;
    pub const CHECKPOINT_SET: u8 = 0x12;
    pub const CHECKPOINT_DELETE: u8 = 0x13;
    pub const REGISTERS_GET: u8 = 0x31;
    pub const ADVANCE_INSTRUCTION: u8 = 0x71;
    pub const EXIT_MONITOR: u8 = 0x77; // Resume/continue execution
    pub const PING: u8 = 0x81;

    // Push notifications from VICE (not request/response — VICE sends these unsolicited)
    pub const STOPPED: u8 = 0x62; // CPU stopped (checkpoint hit, step complete)
    pub const RESUMED: u8 = 0x65; // CPU resumed execution
}

impl ViceMessage {
    pub fn new(command: u8, payload: Vec<u8>) -> Self {
        Self {
            command,
            payload,
            request_id: 0,
            error_code: 0,
        }
    }

    pub fn with_id(command: u8, payload: Vec<u8>, request_id: u32) -> Self {
        Self {
            command,
            payload,
            request_id,
            error_code: 0,
        }
    }

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
