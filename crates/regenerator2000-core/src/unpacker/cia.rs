//! MOS 6526 Complex Interface Adapter (CIA) timer state emulation.
//!
//! Emulates interval Timer A, Timer B, control registers (CRA/CRB), and Interrupt
//! Control Register (ICR) flags/masks for CIA 1 (`$DC00`–`$DC0F`) and CIA 2 (`$DD00`–`$DD0F`).
//! Many Commodore 8-bit depackers rely on CIA timers to pace decompression or poll
//! `$DC0D` for underflow flags.

/// MOS 6526 Complex Interface Adapter (CIA) timer state emulation.
#[derive(Debug, Clone)]
pub(crate) struct CiaState {
    ta_latch: u16,
    ta_counter: u16,
    ta_control: u8,
    ta_read_latch: u8,

    tb_latch: u16,
    tb_counter: u16,
    tb_control: u8,
    tb_read_latch: u8,

    icr_mask: u8,
    icr_data: u8,
}

impl Default for CiaState {
    fn default() -> Self {
        Self {
            ta_latch: 0,
            ta_counter: 0xFFFF,
            ta_control: 0,
            ta_read_latch: 0,
            tb_latch: 0,
            tb_counter: 0xFFFF,
            tb_control: 0,
            tb_read_latch: 0,
            icr_mask: 0,
            icr_data: 0,
        }
    }
}

impl CiaState {
    /// Returns `true` if Timer A is running (CRA bit 0 is set).
    #[must_use]
    #[inline]
    pub fn is_ta_running(&self) -> bool {
        (self.ta_control & 0x01) != 0
    }

    /// Returns `true` if Timer B is running (CRB bit 0 is set).
    #[must_use]
    #[inline]
    pub fn is_tb_running(&self) -> bool {
        (self.tb_control & 0x01) != 0
    }

    /// Steps active CIA timers by the specified number of CPU clock cycles.
    ///
    /// Decrements active counters, reloads latches on underflow, sets ICR underflow flags
    /// (bit 0 for Timer A, bit 1 for Timer B), clears the `START` bit in one-shot mode,
    /// and steps Timer B when chained (`INMODE` = `%10` or `%11`).
    pub fn step(&mut self, cycles: u32) {
        if self.is_ta_running() {
            for _ in 0..cycles {
                if !self.is_ta_running() {
                    break;
                }
                if self.ta_counter == 0 {
                    self.ta_counter = self.ta_latch;
                    self.icr_data |= 0x01; // Timer A underflow flag
                    if (self.ta_control & 0x08) != 0 {
                        self.ta_control &= !0x01; // Hardware clears START bit in CRA for one-shot mode
                    }
                    // Check Timer B chaining mode (INMODE %10 or %11)
                    if self.is_tb_running() && (self.tb_control & 0x40) != 0 {
                        self.step_tb_underflow();
                    }
                } else {
                    self.ta_counter = self.ta_counter.saturating_sub(1);
                }
            }
        }
        if self.is_tb_running() && (self.tb_control & 0x60) == 0x00 {
            for _ in 0..cycles {
                if !self.is_tb_running() {
                    break;
                }
                if self.tb_counter == 0 {
                    self.tb_counter = self.tb_latch;
                    self.icr_data |= 0x02; // Timer B underflow flag
                    if (self.tb_control & 0x08) != 0 {
                        self.tb_control &= !0x01; // Hardware clears START bit in CRB for one-shot mode
                    }
                } else {
                    self.tb_counter = self.tb_counter.saturating_sub(1);
                }
            }
        }
    }

    /// Steps active CIA timers by the specified number of CPU clock cycles.
    ///
    /// Alias for [`step`](Self::step) providing standard cycle stepping naming.
    #[inline]
    pub fn step_cycles(&mut self, cycles: u32) {
        self.step(cycles);
    }

    fn step_tb_underflow(&mut self) {
        if self.tb_counter == 0 {
            self.tb_counter = self.tb_latch;
            self.icr_data |= 0x02;
            if (self.tb_control & 0x08) != 0 {
                self.tb_control &= !0x01;
            }
        } else {
            self.tb_counter = self.tb_counter.saturating_sub(1);
        }
    }

    /// Reads a CIA register offset (`$00`–`$0F`).
    ///
    /// - `$04` / `$05`: Reads Timer A Counter low/high bytes (reading low byte latches high byte).
    /// - `$06` / `$07`: Reads Timer B Counter low/high bytes (reading low byte latches high byte).
    /// - `$0D`: Reads Interrupt Control Register (ICR) with clear-on-read semantics and IR summary bit 7.
    /// - `$0E` / `$0F`: Reads CRA / CRB control register bytes.
    pub fn read_reg(&mut self, reg: u8) -> u8 {
        match reg & 0x0F {
            0x04 => {
                self.ta_read_latch = ((self.ta_counter >> 8) & 0xFF) as u8;
                (self.ta_counter & 0xFF) as u8
            }
            0x05 => self.ta_read_latch,
            0x06 => {
                self.tb_read_latch = ((self.tb_counter >> 8) & 0xFF) as u8;
                (self.tb_counter & 0xFF) as u8
            }
            0x07 => self.tb_read_latch,
            0x0D => {
                let mut val = self.icr_data;
                if (self.icr_data & self.icr_mask & 0x1F) != 0 {
                    val |= 0x80; // IR summary flag
                }
                self.icr_data = 0; // Clear-on-read
                val
            }
            0x0E => self.ta_control,
            0x0F => self.tb_control,
            _ => 0x00,
        }
    }

    /// Writes a CIA register offset (`$00`–`$0F`).
    ///
    /// - `$04` / `$05`: Updates Timer A latch low/high bytes (high byte write reloads counter if stopped).
    /// - `$06` / `$07`: Updates Timer B latch low/high bytes (high byte write reloads counter if stopped).
    /// - `$0D`: Configures ICR interrupt enable mask (bit 7 sets or clears mask bits 0–4).
    /// - `$0E` / `$0F`: Updates CRA / CRB control registers (bit 4 force-load strobe reloads counter).
    pub fn write_reg(&mut self, reg: u8, val: u8) {
        match reg & 0x0F {
            0x04 => self.ta_latch = (self.ta_latch & 0xFF00) | u16::from(val),
            0x05 => {
                self.ta_latch = (self.ta_latch & 0x00FF) | (u16::from(val) << 8);
                if !self.is_ta_running() {
                    self.ta_counter = self.ta_latch;
                }
            }
            0x06 => self.tb_latch = (self.tb_latch & 0xFF00) | u16::from(val),
            0x07 => {
                self.tb_latch = (self.tb_latch & 0x00FF) | (u16::from(val) << 8);
                if !self.is_tb_running() {
                    self.tb_counter = self.tb_latch;
                }
            }
            0x0D => {
                if (val & 0x80) != 0 {
                    self.icr_mask |= val & 0x1F;
                } else {
                    self.icr_mask &= !(val & 0x1F);
                }
            }
            0x0E => {
                self.ta_control = val & !0x10; // Mask out force-load strobe bit 4
                if (val & 0x10) != 0 {
                    self.ta_counter = self.ta_latch; // Force load
                }
            }
            0x0F => {
                self.tb_control = val & !0x10; // Mask out force-load strobe bit 4
                if (val & 0x10) != 0 {
                    self.tb_counter = self.tb_latch; // Force load
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cia_timer_a_countdown_and_icr_flag() {
        let mut cia = CiaState::default();
        cia.write_reg(0x04, 0x05);
        cia.write_reg(0x05, 0x00);
        cia.write_reg(0x0E, 0x01);
        assert!(cia.is_ta_running());

        cia.step(6);
        let icr = cia.read_reg(0x0D);
        assert_eq!(
            icr & 0x01,
            0x01,
            "Timer A underflow bit 0 should be set in ICR"
        );
    }

    #[test]
    fn test_cia_icr_clear_on_read() {
        let mut cia = CiaState::default();
        cia.write_reg(0x04, 0x02);
        cia.write_reg(0x05, 0x00);
        cia.write_reg(0x0E, 0x01);
        cia.step(3);

        let first_read = cia.read_reg(0x0D);
        assert_eq!(first_read & 0x01, 0x01, "First read returns underflow flag");

        let second_read = cia.read_reg(0x0D);
        assert_eq!(second_read, 0x00, "Second read should be cleared to 0");
    }

    #[test]
    fn test_cia_timer_oneshot_clears_start_bit() {
        let mut cia = CiaState::default();
        cia.write_reg(0x04, 0x02);
        cia.write_reg(0x05, 0x00);
        cia.write_reg(0x0E, 0x09);
        assert!(cia.is_ta_running());

        cia.step(3);
        assert!(
            !cia.is_ta_running(),
            "START bit should be cleared after one-shot underflow"
        );
        assert_eq!(cia.read_reg(0x0E) & 0x01, 0);
    }

    #[test]
    fn test_cia_timer_b_chaining_mode() {
        let mut cia = CiaState::default();
        cia.write_reg(0x04, 0x02);
        cia.write_reg(0x05, 0x00);

        cia.write_reg(0x06, 0x01);
        cia.write_reg(0x07, 0x00);

        cia.write_reg(0x0F, 0x41);
        cia.write_reg(0x0E, 0x01);

        assert!(cia.is_ta_running());
        assert!(cia.is_tb_running());

        cia.step(3);
        let icr1 = cia.read_reg(0x0D);
        assert_eq!(icr1 & 0x01, 0x01, "Timer A underflow flag");
        assert_eq!(icr1 & 0x02, 0x00, "Timer B should not have underflowed yet");

        cia.step(3);
        let icr2 = cia.read_reg(0x0D);
        assert_eq!(
            icr2 & 0x02,
            0x02,
            "Timer B should underflow on second Timer A underflow pulse"
        );
    }
}
