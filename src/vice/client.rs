use super::protocol::{ViceCommand, ViceMessage};
use crate::events::AppEvent;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{self, Sender};
use std::thread;

pub enum ViceEvent {
    Connected,
    Disconnected(String),
    Message(ViceMessage),
}

pub struct ViceClient {
    cmd_tx: Sender<ViceMessage>,
}

impl ViceClient {
    pub fn connect(addr: &str, app_tx: Sender<AppEvent>) -> anyhow::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(false)?;
        stream.set_nodelay(true)?;

        let mut read_stream = stream.try_clone()?;
        let mut write_stream = stream;

        let (cmd_tx, cmd_rx) = mpsc::channel::<ViceMessage>();

        // Reading thread
        let app_tx_read = app_tx.clone();
        thread::spawn(move || {
            let _ = app_tx_read.send(AppEvent::Vice(ViceEvent::Connected));
            let mut buffer = [0u8; 8192];
            let mut read_buf = Vec::new();

            loop {
                match read_stream.read(&mut buffer) {
                    Ok(0) => {
                        let _ = app_tx_read.send(AppEvent::Vice(ViceEvent::Disconnected(
                            "Connection closed".to_string(),
                        )));
                        break;
                    }
                    Ok(n) => {
                        read_buf.extend_from_slice(&buffer[..n]);

                        // Decode all full messages
                        loop {
                            match ViceMessage::decode(&read_buf) {
                                Ok(Some((msg, size))) => {
                                    let _ =
                                        app_tx_read.send(AppEvent::Vice(ViceEvent::Message(msg)));
                                    read_buf.drain(..size);
                                }
                                Ok(None) => {
                                    break; // wait for more data
                                }
                                Err(e) => {
                                    // Corrupt data, clear buffer
                                    let _ = app_tx_read.send(AppEvent::Vice(
                                        ViceEvent::Disconnected(format!("Protocol error: {}", e)),
                                    ));
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = app_tx_read
                            .send(AppEvent::Vice(ViceEvent::Disconnected(e.to_string())));
                        break;
                    }
                }
            }
        });

        // Writing thread
        thread::spawn(move || {
            while let Ok(msg) = cmd_rx.recv() {
                let bytes = msg.encode();
                if write_stream.write_all(&bytes).is_err() {
                    break;
                }
            }
            let _ = write_stream.shutdown(Shutdown::Both);
        });

        Ok(Self { cmd_tx })
    }

    pub fn send(&self, msg: ViceMessage) {
        let _ = self.cmd_tx.send(msg);
    }

    pub fn send_ping(&self) {
        self.send(ViceMessage::new(ViceCommand::PING, vec![]));
    }

    pub fn send_registers_get(&self) {
        // For C64 main memory space is usually 0
        self.send(ViceMessage::new(ViceCommand::REGISTERS_GET, vec![0]));
    }

    pub fn send_advance_instruction(&self) {
        // Payload: step_mode (1 byte), count (2 bytes LE)
        // step_mode 0 = step-into (single step)
        self.send(ViceMessage::new(
            ViceCommand::ADVANCE_INSTRUCTION,
            vec![0, 1, 0],
        ));
    }

    /// Step over (next): execute through subroutine calls without stopping inside them.
    /// step_mode 1 = step-over
    pub fn send_step_over(&self) {
        self.send(ViceMessage::new(
            ViceCommand::ADVANCE_INSTRUCTION,
            vec![1, 1, 0],
        ));
    }

    /// Resume execution (exit the monitor / continue).
    pub fn send_continue(&self) {
        self.send(ViceMessage::new(ViceCommand::EXIT_MONITOR, vec![]));
    }

    /// Set a temporary exec-only breakpoint at `addr` and auto-delete it after it's hit.
    /// Used for Run-to-Cursor (F8): set, continue, VICE stops at addr, checkpoint is gone.
    /// CHECKPOINT_SET payload: start_addr (2 LE), end_addr (2 LE),
    ///   stop_when_hit (1), enabled (1), cpu_operation (1), temporary (1)
    /// cpu_operation 0x04 = exec; temporary 1 = auto-delete on hit.
    pub fn send_checkpoint_set_exec_temp(&self, addr: u16) {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&addr.to_le_bytes()); // start_addr
        payload.extend_from_slice(&addr.to_le_bytes()); // end_addr (same = exact address)
        payload.push(1); // stop_when_hit
        payload.push(1); // enabled
        payload.push(0x04); // cpu_operation: exec
        payload.push(1); // temporary: auto-delete after first hit
        self.send(ViceMessage::new(ViceCommand::CHECKPOINT_SET, payload));
    }

    /// Request a memory range from VICE.
    /// MEMORY_GET payload: start_addr (2 LE) + end_addr (2 LE) + memory_space (1)
    /// memory_space 0 = default (main C64 RAM)
    pub fn send_memory_get(&self, start: u16, end: u16) {
        let mut payload = Vec::with_capacity(5);
        payload.extend_from_slice(&start.to_le_bytes());
        payload.extend_from_slice(&end.to_le_bytes());
        payload.push(0); // memory_space 0 = main RAM
        self.send(ViceMessage::new(ViceCommand::MEMORY_GET, payload));
    }
}
