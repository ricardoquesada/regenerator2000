//! TSCrunch strategy implementation.

use super::{Packer, PackerInfo};
use crate::state::types::System;
use crate::unpacker::UnpackerMemory;
use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;

/// TSCrunch packer strategy with high-stream relocation clearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TScrunchPacker {
    /// Information describing the packer.
    pub info: PackerInfo,
    /// Start address of high-memory compressed stream relocation.
    pub reloc_start: Option<u16>,
    /// Whether the initial stream relocation write flags have been cleared.
    pub reloc_cleared: bool,
}

impl TScrunchPacker {
    /// Creates a new [`TScrunchPacker`].
    #[must_use]
    pub fn new(info: PackerInfo, reloc_start: Option<u16>) -> Self {
        Self {
            info,
            reloc_start,
            reloc_cleared: false,
        }
    }
}

impl Packer for TScrunchPacker {
    fn info(&self) -> PackerInfo {
        self.info.clone()
    }

    fn on_step(&mut self, cpu: &mut CPU<UnpackerMemory, Nmos6502>, phase: u8) {
        if phase != 2 || self.reloc_cleared {
            return;
        }
        let pc = cpu.registers.program_counter as usize;
        if (0x0021..=0x0090).contains(&pc) || (0x0121..=0x0190).contains(&pc) {
            if let Some(reloc) = self.reloc_start {
                let reloc_idx = reloc as usize;
                if reloc_idx < cpu.memory.mem.len() {
                    for a in reloc_idx..cpu.memory.mem.len() {
                        cpu.memory.written[a] = false;
                    }
                }
            }
            self.reloc_cleared = true;
        }
    }

    fn post_emulate(
        &self,
        _mem: &[u8],
        _snapshot: &[u8],
        written: &[bool],
        range: &mut (u16, u16),
        _entry_point: &mut u16,
        _system: &System,
        _y_reg: u8,
    ) {
        let mut last_written = range.0;
        for a in (range.0 as usize..=range.1 as usize).rev() {
            if written.get(a).copied().unwrap_or(false) {
                last_written = a as u16;
                break;
            }
        }
        if last_written < range.1 {
            range.1 = last_written;
        }
    }
}

fn detect_at(mem: &[u8], q: usize) -> Option<Box<dyn Packer>> {
    if mem.len() <= q + 18 {
        return None;
    }

    // Variant 1: TSCrunch v1.3+ (Zero-Page depacker at $0002)
    // Code at $080D: SEI; LDX #$CC; LDA $081A,X; STA $00,X; DEX; BNE; JMP $0002
    if mem[q] == 0x78
        && mem[q + 1] == 0xA2
        && mem[q + 3] == 0xBD
        && mem[q + 6] == 0x95
        && mem[q + 7] == 0x00
        && mem[q + 8] == 0xCA
        && mem[q + 9] == 0xD0
        && mem[q + 11] == 0x4C
        && mem[q + 12] == 0x02
        && mem[q + 13] == 0x00
    {
        let mut entry_point = None;
        if mem.len() >= q + 0x70 {
            for p in q + 14..mem.len().min(q + 0x80) {
                if mem.len() >= p + 8
                    && mem[p] == 0xA9
                    && mem[p + 1] == 0x37
                    && mem[p + 2] == 0x85
                    && mem[p + 3] == 0x01
                    && mem[p + 4] == 0x58
                    && mem[p + 5] == 0x4C
                {
                    entry_point = Some(u16::from_le_bytes([mem[p + 6], mem[p + 7]]));
                    break;
                }
            }
        }

        let mut reloc_start = None;
        if mem.len() > q + 0x11 {
            let addr = u16::from_le_bytes([mem[q + 0x10], mem[q + 0x11]]);
            if addr > 0x0800 {
                reloc_start = Some(addr);
            }
        }

        return Some(Box::new(TScrunchPacker::new(
            PackerInfo {
                name: "TSCrunch v1.3+",
                dep_addr: Some(0x0002),
                start_addr: Some(0x0800),
                end_addr: None,
                entry_point,
                end_addr_ptr: None,
            },
            reloc_start,
        )));
    }

    // Variant 2: TSCrunch v1.3+-X2 (Stack depacker at $0100)
    // Code at $080D: SEI; LDA #$34; STA $01; LDX #$D0; LDA $081F,X; STA $00FB,X; DEX; BNE; JMP $0100
    if mem[q] == 0x78
        && mem[q + 1] == 0xA9
        && mem[q + 2] == 0x34
        && mem[q + 3] == 0x85
        && mem[q + 4] == 0x01
        && mem[q + 5] == 0xA2
        && mem[q + 7] == 0xBD
        && mem[q + 10] == 0x9D
        && mem[q + 13] == 0xCA
        && mem[q + 14] == 0xD0
        && mem[q + 16] == 0x4C
        && mem[q + 17] == 0x00
        && mem[q + 18] == 0x01
    {
        let mut entry_point = None;
        if mem.len() >= q + 0x70 {
            for p in q + 19..mem.len().min(q + 0x80) {
                if mem.len() >= p + 8
                    && mem[p] == 0xA9
                    && mem[p + 1] == 0x37
                    && mem[p + 2] == 0x85
                    && mem[p + 3] == 0x01
                    && mem[p + 4] == 0x58
                    && mem[p + 5] == 0x4C
                {
                    entry_point = Some(u16::from_le_bytes([mem[p + 6], mem[p + 7]]));
                    break;
                }
            }
        }

        let mut reloc_start = None;
        if mem.len() > q + 0x19 {
            let addr = u16::from_le_bytes([mem[q + 0x18], mem[q + 0x19]]);
            if addr > 0x0800 {
                reloc_start = Some(addr);
            }
        }

        return Some(Box::new(TScrunchPacker::new(
            PackerInfo {
                name: "TSCrunch v1.3+-X2",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0800),
                end_addr: None,
                entry_point,
                end_addr_ptr: None,
            },
            reloc_start,
        )));
    }

    None
}

/// Detects TSCrunch signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16, _load_end: u16) -> Option<Box<dyn Packer>> {
    if load_addr <= 0x0801 {
        let q = if mem.len() == 65536 {
            0x080D
        } else {
            (0x080D_u16.saturating_sub(load_addr)) as usize
        };
        if let Some(p) = detect_at(mem, q) {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mule_tscrunch_x() {
        let path = std::path::Path::new("tests/6502/c64_mule.tscrunch_x.prg");
        let path = if path.exists() {
            path.to_path_buf()
        } else {
            std::path::PathBuf::from("../../tests/6502/c64_mule.tscrunch_x.prg")
        };
        if !path.exists() {
            return;
        }
        let prg = std::fs::read(path).unwrap();
        let load_addr = u16::from_le_bytes([prg[0], prg[1]]);
        let raw = &prg[2..];
        let mut mem = vec![0u8; 65536];
        mem[load_addr as usize..load_addr as usize + raw.len()].copy_from_slice(raw);
        let packer =
            detect(&mem, load_addr, load_addr + raw.len() as u16).expect("packer detected");
        println!("Packer info: {:?}", packer.info());
        let config = crate::unpacker::UnpackConfig {
            max_instructions: 350_000_000,
            ..Default::default()
        };
        let res = crate::unpacker::unpack(raw, load_addr, &config, None).unwrap();
        println!(
            "Unpacked range: ${:04X}-${:04X} (${:04X})",
            res.start_addr, res.end_addr, res.entry_point
        );
        assert_eq!(res.start_addr, 0x0800);
        assert_eq!(res.end_addr, 0x9D19);
        assert_eq!(res.entry_point, 0x1100);
    }
}
