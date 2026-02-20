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
        // Step trace: 0 (one instruction) ? Actually doc says count = 2 bytes?
        // Let's send a basic command to advance 1 instruction.
        // The advance instructions payload is usually: Step mode (1 byte), Count (2 bytes little-endian)
        // Step mode 0 = single step
        self.send(ViceMessage::new(
            ViceCommand::ADVANCE_INSTRUCTION,
            vec![0, 1, 0], // Mode 0 (step), Count 1 (little endian)
        ));
    }
}
