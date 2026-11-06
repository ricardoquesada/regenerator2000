//! Asynchronous TCP client for the VICE Binary Monitor protocol.

use super::protocol::{ViceCommand, ViceCpuOp, ViceMessage};
use crate::error::ViceError;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{self, Sender};
use std::thread;

/// Lifecycle or message event emitted by the [`ViceClient`] reader thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViceEvent {
    /// The TCP socket connected to the VICE Binary Monitor.
    Connected,
    /// The TCP connection closed or encountered a framing error.
    Disconnected(String),
    /// A complete binary monitor message arrived from VICE.
    Message(ViceMessage),
}

/// Threaded TCP client that sends [`ViceMessage`] commands and streams [`ViceEvent`] notifications.
pub struct ViceClient {
    cmd_tx: Sender<ViceMessage>,
    event_rx: Option<mpsc::Receiver<ViceEvent>>,
}

impl ViceClient {
    /// Connects to the VICE Binary Monitor at `addr` and starts reader and writer threads.
    ///
    /// # Errors
    ///
    /// Returns [`ViceError::ConnectionFailed`] if the TCP connection fails, or
    /// [`ViceError::Io`] if socket configuration or stream cloning fails.
    pub fn connect(addr: &str, app_tx: Sender<ViceEvent>) -> Result<Self, ViceError> {
        let stream = TcpStream::connect(addr).map_err(|source| ViceError::ConnectionFailed {
            address: addr.to_string(),
            source,
        })?;
        stream.set_nonblocking(false)?;
        stream.set_nodelay(true)?;

        let mut read_stream = stream.try_clone()?;
        let mut write_stream = stream;

        let (cmd_tx, cmd_rx) = mpsc::channel::<ViceMessage>();

        let app_tx_read = app_tx;
        thread::spawn(move || {
            if app_tx_read.send(ViceEvent::Connected).is_err() {
                let _ = read_stream.shutdown(Shutdown::Both);
                return;
            }
            let mut buffer = [0u8; 8192];
            let mut read_buf = Vec::new();

            loop {
                match read_stream.read(&mut buffer) {
                    Ok(0) => {
                        let _ = app_tx_read
                            .send(ViceEvent::Disconnected("Connection closed".to_string()));
                        break;
                    }
                    Ok(n) => {
                        read_buf.extend_from_slice(&buffer[..n]);
                        let mut consumed = 0;

                        loop {
                            match ViceMessage::decode(&read_buf[consumed..]) {
                                Ok(Some((msg, size))) => {
                                    consumed += size;
                                    if app_tx_read.send(ViceEvent::Message(msg)).is_err() {
                                        let _ = read_stream.shutdown(Shutdown::Both);
                                        return;
                                    }
                                }
                                Ok(None) => {
                                    break;
                                }
                                Err(e) => {
                                    let _ = app_tx_read.send(ViceEvent::Disconnected(format!(
                                        "Protocol error: {e}"
                                    )));
                                    let _ = read_stream.shutdown(Shutdown::Both);
                                    return;
                                }
                            }
                        }

                        if consumed > 0 {
                            read_buf.drain(..consumed);
                        }
                    }
                    Err(e) => {
                        let _ = app_tx_read.send(ViceEvent::Disconnected(e.to_string()));
                        break;
                    }
                }
            }
            let _ = read_stream.shutdown(Shutdown::Both);
        });

        thread::spawn(move || {
            while let Ok(msg) = cmd_rx.recv() {
                let bytes = msg.encode();
                if write_stream.write_all(&bytes).is_err() {
                    break;
                }
            }
            let _ = write_stream.shutdown(Shutdown::Both);
        });

        Ok(Self {
            cmd_tx,
            event_rx: None,
        })
    }

    /// Stores the [`ViceEvent`] channel receiver inside this client so headless callers keep the reader channel open.
    pub fn set_event_receiver(&mut self, rx: mpsc::Receiver<ViceEvent>) {
        self.event_rx = Some(rx);
    }

    /// Returns a reference to the owned [`ViceEvent`] channel receiver, if set.
    #[must_use]
    pub fn event_receiver(&self) -> Option<&mpsc::Receiver<ViceEvent>> {
        self.event_rx.as_ref()
    }

    /// Queues a raw [`ViceMessage`] for transmission to VICE.
    pub fn send(&self, msg: ViceMessage) {
        let _ = self.cmd_tx.send(msg);
    }

    /// Sends a [`ViceCommand::PING`] request.
    pub fn send_ping(&self) {
        self.send(ViceMessage::new(ViceCommand::PING, vec![]));
    }

    /// Requests CPU registers for the main memory space (`0`).
    pub fn send_registers_get(&self) {
        self.send(ViceMessage::new(ViceCommand::REGISTERS_GET, vec![0]));
    }

    /// Steps the CPU forward by one instruction (step into).
    pub fn send_advance_instruction(&self) {
        self.send(ViceMessage::new(
            ViceCommand::ADVANCE_INSTRUCTION,
            vec![0, 1, 0],
        ));
    }

    /// Steps over a subroutine call without stopping inside the subroutine.
    pub fn send_step_over(&self) {
        self.send(ViceMessage::new(
            ViceCommand::ADVANCE_INSTRUCTION,
            vec![1, 1, 0],
        ));
    }

    /// Executes instructions until the current subroutine returns (step out).
    pub fn send_execute_until_return(&self) {
        self.send(ViceMessage::new(ViceCommand::EXECUTE_UNTIL_RETURN, vec![]));
    }

    /// Exits the monitor and resumes CPU execution.
    pub fn send_continue(&self) {
        self.send(ViceMessage::new(ViceCommand::EXIT_MONITOR, vec![]));
    }

    /// Requests the list of all active checkpoints from VICE.
    pub fn send_checkpoint_list(&self) {
        self.send(ViceMessage::new(ViceCommand::CHECKPOINT_LIST, vec![]));
    }

    /// Builds an 8-byte [`ViceCommand::CHECKPOINT_SET`] payload for `addr`.
    fn checkpoint_payload(addr: u16, cpu_op: u8, temporary: bool) -> Vec<u8> {
        let [lo, hi] = addr.to_le_bytes();
        vec![lo, hi, lo, hi, 1, 1, cpu_op, u8::from(temporary)]
    }

    /// Sets a persistent execution breakpoint at `addr`.
    pub fn send_checkpoint_set_exec(&self, addr: u16) {
        self.send(ViceMessage::new(
            ViceCommand::CHECKPOINT_SET,
            Self::checkpoint_payload(addr, ViceCpuOp::EXEC, false),
        ));
    }

    /// Sets a persistent read (load) watchpoint at `addr`.
    pub fn send_checkpoint_set_load(&self, addr: u16) {
        self.send(ViceMessage::new(
            ViceCommand::CHECKPOINT_SET,
            Self::checkpoint_payload(addr, ViceCpuOp::LOAD, false),
        ));
    }

    /// Sets a persistent write (store) watchpoint at `addr`.
    pub fn send_checkpoint_set_store(&self, addr: u16) {
        self.send(ViceMessage::new(
            ViceCommand::CHECKPOINT_SET,
            Self::checkpoint_payload(addr, ViceCpuOp::STORE, false),
        ));
    }

    /// Sets a persistent read and write watchpoint at `addr`.
    pub fn send_checkpoint_set_load_store(&self, addr: u16) {
        self.send(ViceMessage::new(
            ViceCommand::CHECKPOINT_SET,
            Self::checkpoint_payload(addr, ViceCpuOp::LOAD | ViceCpuOp::STORE, false),
        ));
    }

    /// Deletes a checkpoint by its `id`.
    pub fn send_checkpoint_delete(&self, id: u32) {
        self.send(ViceMessage::with_id(
            ViceCommand::CHECKPOINT_DELETE,
            id.to_le_bytes().to_vec(),
            id,
        ));
    }

    /// Sets a temporary execution breakpoint at `addr` that VICE deletes after the first hit.
    pub fn send_checkpoint_set_exec_temp(&self, addr: u16) {
        self.send(ViceMessage::new(
            ViceCommand::CHECKPOINT_SET,
            Self::checkpoint_payload(addr, ViceCpuOp::EXEC, true),
        ));
    }

    /// Requests emulated memory from `start` through `end` (inclusive) in main RAM using `req_id`.
    pub fn send_memory_get(&self, start: u16, end: u16, req_id: u32) {
        let mut payload = Vec::with_capacity(8);
        payload.push(0);
        payload.extend_from_slice(&start.to_le_bytes());
        payload.extend_from_slice(&end.to_le_bytes());
        payload.push(0);
        payload.extend_from_slice(&0u16.to_le_bytes());
        self.send(ViceMessage::with_id(
            ViceCommand::MEMORY_GET,
            payload,
            req_id,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vice::protocol::{API_VERSION, STX};
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn checkpoint_payload_encodes_persistent_and_temporary_flags() {
        assert_eq!(
            ViceClient::checkpoint_payload(0xC000, ViceCpuOp::EXEC, false),
            vec![0x00, 0xC0, 0x00, 0xC0, 1, 1, ViceCpuOp::EXEC, 0]
        );
        assert_eq!(
            ViceClient::checkpoint_payload(0xC000, ViceCpuOp::EXEC, true),
            vec![0x00, 0xC0, 0x00, 0xC0, 1, 1, ViceCpuOp::EXEC, 1]
        );
    }

    #[test]
    fn client_receives_coalesced_frames_and_reports_protocol_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let (tx, rx) = mpsc::channel();
        let _client = ViceClient::connect(&addr, tx).unwrap();
        let (mut server_stream, _) = listener.accept().unwrap();

        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            ViceEvent::Connected
        );

        // Send two valid response frames followed by an invalid STX byte in one write.
        let mut wire = Vec::new();
        for req_id in [1u32, 2u32] {
            wire.push(STX);
            wire.push(API_VERSION);
            wire.extend_from_slice(&1u32.to_le_bytes());
            wire.push(ViceCommand::PING);
            wire.push(0);
            wire.extend_from_slice(&req_id.to_le_bytes());
            wire.push(0xAA);
        }
        wire.extend_from_slice(&[0xFF; 12]);
        server_stream.write_all(&wire).unwrap();

        let ev1 = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let ev2 = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let ev3 = rx.recv_timeout(Duration::from_secs(1)).unwrap();

        assert!(matches!(
            ev1,
            ViceEvent::Message(ref m) if m.request_id == 1 && m.payload == vec![0xAA]
        ));
        assert!(matches!(
            ev2,
            ViceEvent::Message(ref m) if m.request_id == 2 && m.payload == vec![0xAA]
        ));
        assert!(matches!(
            ev3,
            ViceEvent::Disconnected(ref s) if s.contains("Protocol error")
        ));
    }
}
