//! Decoders for Commodore 64 and Commodore 128 I/O hardware registers.

/// Decoded snapshot of a MOS 6526 Complex Interface Adapter (CIA) register block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CiaState {
    /// Data Port A (`$DC00` on CIA 1, `$DD00` on CIA 2).
    pub pra: u8,
    /// Data Port B (`$DC01` on CIA 1, `$DD01` on CIA 2).
    pub prb: u8,
    /// Data Direction Register A (`$DC02` on CIA 1, `$DD02` on CIA 2).
    pub ddra: u8,
    /// Data Direction Register B (`$DC03` on CIA 1, `$DD03` on CIA 2).
    pub ddrb: u8,
    /// Timer A 16-bit counter value (`$DC04`–`$DC05` or `$DD04`–`$DD05`).
    pub timer_a: u16,
    /// Timer B 16-bit counter value (`$DC06`–`$DC07` or `$DD06`–`$DD07`).
    pub timer_b: u16,
}

impl CiaState {
    /// Decodes a 16-byte CIA register window from `io_mem` at `offset`.
    ///
    /// Returns [`CiaState::default`] if `io_mem` does not contain 16 bytes starting at `offset`.
    #[must_use]
    pub fn decode(io_mem: &[u8], offset: usize) -> Self {
        let Some(regs) = io_mem.get(offset..).and_then(|s| s.get(..16)) else {
            return Self::default();
        };

        Self {
            pra: regs[0x00],
            prb: regs[0x01],
            ddra: regs[0x02],
            ddrb: regs[0x03],
            timer_a: u16::from_le_bytes([regs[0x04], regs[0x05]]),
            timer_b: u16::from_le_bytes([regs[0x06], regs[0x07]]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_cia_state_valid_window() {
        let mut io_mem = vec![0u8; 32];
        let offset = 16;
        io_mem[offset] = 0x7F;
        io_mem[offset + 1] = 0xFF;
        io_mem[offset + 2] = 0xFF;
        io_mem[offset + 3] = 0x00;
        io_mem[offset + 4] = 0x25;
        io_mem[offset + 5] = 0x40;
        io_mem[offset + 6] = 0xCD;
        io_mem[offset + 7] = 0xAB;

        let state = CiaState::decode(&io_mem, offset);
        assert_eq!(
            state,
            CiaState {
                pra: 0x7F,
                prb: 0xFF,
                ddra: 0xFF,
                ddrb: 0x00,
                timer_a: 0x4025,
                timer_b: 0xABCD,
            }
        );
    }

    #[test]
    fn decode_cia_state_short_slice_or_overflow_returns_default() {
        let io_mem = [0xFFu8; 15];
        assert_eq!(CiaState::decode(&io_mem, 0), CiaState::default());
        assert_eq!(CiaState::decode(&io_mem, usize::MAX), CiaState::default());
    }
}
