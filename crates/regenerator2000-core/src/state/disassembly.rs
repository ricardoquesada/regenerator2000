use super::app_state::AppState;
use super::types::{Addr, CachedArrow};

impl AppState {
    pub fn disassemble(&mut self) {
        let ctx = crate::disassembler::context::DisassemblyContext {
            data: &self.raw_data,
            block_types: &self.block_types,
            labels: &self.labels,
            origin: self.origin,
            settings: &self.settings,
            platform_comments: &self.platform_comments,
            user_side_comments: &self.user_side_comments,
            user_line_comments: &self.user_line_comments,
            immediate_value_formats: &self.immediate_value_formats,
            cross_refs: &self.cross_refs,
            collapsed_blocks: &self.collapsed_blocks,
            splitters: &self.splitters,
            scopes: &self.scopes,
        };
        let mut lines = self.disassembler.disassemble_ctx(&ctx);

        // Add external label definitions at the top if enabled
        if self.settings.all_labels {
            let external_lines = self.get_external_label_definitions(true);
            // Prepend external lines
            lines.splice(0..0, external_lines);
        }

        self.disassembly = lines;
        self.compute_cached_arrows();
    }

    pub fn compute_cached_arrows(&mut self) {
        let mut arrows = Vec::new();

        if self.disassembly.is_empty() {
            self.cached_arrows = arrows;
            return;
        }

        // Build a temporary index for efficient lookup.
        // We prioritize lines with bytes (instructions/data) over label-only lines.
        // Map: Address -> Line Index
        let mut addr_to_idx = std::collections::BTreeMap::new();
        for (i, line) in self.disassembly.iter().enumerate() {
            if !line.bytes.is_empty() || line.is_collapsed {
                addr_to_idx.insert(line.address, i);
            } else {
                addr_to_idx.entry(line.address).or_insert(i);
            }
        }

        for (src_idx, line) in self.disassembly.iter().enumerate() {
            if let Some(target_addr) = line.target_address {
                // Determine if we should draw an arrow
                let should_draw = if let Some(opcode) = &line.opcode {
                    opcode.is_flow_control_with_target()
                } else {
                    line.mnemonic.eq_ignore_ascii_case("JMP") && line.operand.contains('(')
                };

                if should_draw {
                    // 1. Try exact address match using our index
                    let mut dst_idx_opt = addr_to_idx.get(&target_addr).copied();

                    // 2. If no exact match, it might be a jump into the middle of an instruction
                    if dst_idx_opt.is_none() {
                        // Find the last line with address <= target_addr
                        if let Some((&base_addr, &base_idx)) =
                            addr_to_idx.range(..=target_addr).next_back()
                        {
                            let base_line = &self.disassembly[base_idx];
                            let len = base_line.bytes.len() as u16;
                            if target_addr.0 >= base_addr.0
                                && target_addr.0 < base_addr.0.wrapping_add(len)
                            {
                                dst_idx_opt = Some(base_idx);
                            }
                        }
                    }

                    if let Some(dst_idx) = dst_idx_opt {
                        arrows.push(CachedArrow {
                            start: src_idx,
                            end: dst_idx,
                            target_addr: Some(target_addr),
                        });
                    }
                }
            }
        }
        self.cached_arrows = arrows;
    }

    #[must_use]
    pub fn get_line_index_for_address(&self, address: Addr) -> Option<usize> {
        // First pass: try to find exact match with content (bytes not empty)
        // This prioritizes real instructions/data over label-only or header lines
        if let Some(idx) = self
            .disassembly
            .iter()
            .position(|line| line.address == address && !line.bytes.is_empty())
        {
            return Some(idx);
        }

        // Second pass: try to find any exact match
        if let Some(idx) = self
            .disassembly
            .iter()
            .position(|line| line.address == address)
        {
            return Some(idx);
        }

        // Check for external label definitions (external_label_address matches target)
        if let Some(idx) = self
            .disassembly
            .iter()
            .position(|line| line.external_label_address == Some(address))
        {
            return Some(idx);
        }
        // Third pass: find first address >= target
        self.disassembly
            .iter()
            .position(|line| line.address >= address)
    }

    #[must_use]
    pub fn get_line_index_containing_address(&self, address: Addr) -> Option<usize> {
        // Check if address is in a collapsed block
        for (start_idx, end_idx) in &self.collapsed_blocks {
            let start_addr = self.origin.wrapping_add(*start_idx as u16);
            let end_addr = self.origin.wrapping_add(*end_idx as u16);

            // Check if address is within this collapsed block [start, end]
            // Handle wrap-around if necessary
            let in_range = if start_addr <= end_addr {
                address >= start_addr && address <= end_addr
            } else {
                address >= start_addr || address <= end_addr
            };

            if in_range {
                // Return the index of the line that represents this collapsed block
                // This line starts at start_addr and has is_collapsed=true
                return self.get_line_index_for_address(start_addr);
            }
        }

        self.disassembly.iter().position(|line| {
            let start = line.address;
            let len = line.bytes.len() as u16;

            // For collapsed blocks or special lines with no bytes, we match if address is exact
            if len == 0 {
                return start == address;
            }

            let end = start.wrapping_add(len);

            if start < end {
                address >= start && address < end
            } else {
                // Wrap around case
                address >= start || address < end
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::app_state::AppState;
    use super::super::types::*;

    #[test]
    fn test_get_line_index_skips_headers() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![
            0xA9, 0x00, // LDA #$00
            0x85, 0x02, // STA $02
            0x20, 0x00, 0x10, // JSR $1000
            0x60, // RTS
        ];
        app_state.block_types = vec![BlockType::Code; app_state.raw_data.len()];

        // Add an external label at address below origin → causes header lines at address 0
        use super::super::project::Label;
        app_state
            .labels
            .entry(Addr(0x0002))
            .or_default()
            .push(Label {
                name: "SOME_ZP".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ZeroPageField,
            });
        app_state
            .labels
            .entry(Addr(0x1000))
            .or_default()
            .push(Label {
                name: "ext_sub".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ExternalJump,
            });
        app_state.settings.all_labels = true;

        app_state.disassemble();

        // get_line_index_for_address(0x0801) should land on the first *content* line
        // (after external label definitions), not on a label-only line
        let idx = app_state.get_line_index_for_address(Addr(0x0801));
        assert!(
            idx.is_some(),
            "Should find a line at the origin address $0801"
        );

        let line = &app_state.disassembly[idx.unwrap()];
        assert_eq!(
            line.address,
            Addr(0x0801),
            "Returned line should be at $0801, not a header"
        );
        assert!(
            !line.bytes.is_empty(),
            "Returned line should have real opcode bytes"
        );
    }

    #[test]
    fn test_get_line_index_with_collapsed_block() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0xC000);
        app_state.raw_data = vec![0xEA; 10]; // 10 NOPs
        app_state.block_types = vec![BlockType::Code; 10];

        // Collapse bytes at offsets 3..6 (addresses $C003..$C006)
        app_state.collapsed_blocks.push((3, 6));
        app_state.disassemble();

        // $C004 falls inside the collapsed block → should return the line at $C003
        let idx = app_state.get_line_index_containing_address(Addr(0xC004));
        assert!(idx.is_some());
        let line = &app_state.disassembly[idx.unwrap()];
        assert_eq!(line.address, Addr(0xC003));
    }

    #[test]
    fn test_perform_analysis_regenerates_arrows() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0xC000);
        // beq $C005 ; nop ; nop ; nop ; nop ; rts
        app_state.raw_data = vec![0xF0, 0x03, 0xEA, 0xEA, 0xEA, 0x60];
        app_state.block_types = vec![BlockType::Code; 6];

        app_state.disassemble();
        assert!(
            !app_state.cached_arrows.is_empty(),
            "BEQ should produce an arrow"
        );
    }

    #[test]
    fn test_beq_forward_branch_arrow_with_intervening_instructions() {
        // Reproduces the pattern from user's binary around $1331-$1340:
        //   $1331: DEX           (CA)
        //   $1332: CPX #$00      (E0 00)
        //   $1334: BEQ +$0A      (F0 0A) -> $1340
        //   $1336: LDA $4A       (A5 4A)
        //   $1338: CLC           (18)
        //   $1339: ADC #$04      (69 04)
        //   $133B: STA $4A       (85 4A)
        //   $133D: JMP $1331     (4C 31 13) -> points back to first instruction
        //   $1340: LDX $0802     (AE 02 08)
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1331);
        app_state.raw_data = vec![
            0xCA, // DEX
            0xE0, 0x00, // CPX #$00
            0xF0, 0x0A, // BEQ $1340  (target = $1334 + 2 + $0A = $1340)
            0xA5, 0x4A, // LDA $4A
            0x18, // CLC
            0x69, 0x04, // ADC #$04
            0x85, 0x4A, // STA $4A
            0x4C, 0x31, 0x13, // JMP $1331
            0xAE, 0x02, 0x08, // LDX $0802
        ];
        app_state.block_types = vec![BlockType::Code; app_state.raw_data.len()];

        app_state.disassemble();

        // There should be at least 2 arrows:
        // 1. BEQ $1334 -> $1340 (forward branch)
        // 2. JMP $133D -> $1331 (backward jump)
        assert!(
            app_state.cached_arrows.len() >= 2,
            "Expected at least 2 arrows (BEQ + JMP), got {}",
            app_state.cached_arrows.len()
        );

        // Find the BEQ arrow specifically ($1334 -> $1340)
        let beq_arrow = app_state
            .cached_arrows
            .iter()
            .find(|a| a.target_addr == Some(Addr(0x1340)));
        assert!(
            beq_arrow.is_some(),
            "BEQ $1340 arrow should be present in cached_arrows"
        );

        let beq = beq_arrow.unwrap();
        assert!(
            beq.start < beq.end,
            "BEQ forward branch should have start < end"
        );

        // Find the JMP arrow ($133D -> $1331)
        let jmp_arrow = app_state
            .cached_arrows
            .iter()
            .find(|a| a.target_addr == Some(Addr(0x1331)));
        assert!(
            jmp_arrow.is_some(),
            "JMP $1331 arrow should be present in cached_arrows"
        );

        let jmp = jmp_arrow.unwrap();
        assert!(jmp.start > jmp.end, "JMP backward should have start > end");
    }
}
