use crate::disassembler::{DisassemblyLine, Disassembler};
use std::path::PathBuf;

pub struct AppState {
    pub file_path: Option<PathBuf>,
    pub raw_data: Vec<u8>,
    pub disassembly: Vec<DisassemblyLine>,
    pub disassembler: Disassembler,
    pub origin: u16,
    
    // UI State
    pub cursor_index: usize,
    pub scroll_index: usize,
    pub should_quit: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            file_path: None,
            raw_data: Vec::new(),
            disassembly: Vec::new(),
            disassembler: Disassembler::new(),
            origin: 0,
            cursor_index: 0,
            scroll_index: 0,
            should_quit: false,
        }
    }

    pub fn load_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let data = std::fs::read(&path)?;
        self.file_path = Some(path);
        
        // Simple heuristic for .prg: first 2 bytes are load address
        if let Some(ext) = self.file_path.as_ref().and_then(|p| p.extension()).and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("prg") && data.len() >= 2 {
                self.origin = (data[1] as u16) << 8 | (data[0] as u16);
                self.raw_data = data[2..].to_vec();
            } else {
                self.origin = 0; // Default for .bin, or user can change later
                self.raw_data = data;
            }
        } else {
             self.origin = 0;
             self.raw_data = data;
        }

        self.disassemble();
        Ok(())
    }

    pub fn disassemble(&mut self) {
        self.disassembly = self.disassembler.disassemble(&self.raw_data, self.origin);
    }
}
