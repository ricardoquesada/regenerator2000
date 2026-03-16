use super::app_state::AppState;
use super::types::{Addr, CachedArrow};

impl AppState {
    pub fn disassemble(&mut self) {
        let ctx = crate::disassembler::DisassemblyContext {
            data: &self.raw_data,
            block_types: &self.block_types,
            labels: &self.labels,
            origin: self.origin,
            settings: &self.settings,
            system_comments: &self.system_comments,
            user_side_comments: &self.user_side_comments,
            user_line_comments: &self.user_line_comments,
            immediate_value_formats: &self.immediate_value_formats,
            cross_refs: &self.cross_refs,
            collapsed_blocks: &self.collapsed_blocks,
            splitters: &self.splitters,
        };
        let mut lines = self.disassembler.disassemble_ctx(&ctx);

        // Add external label definitions at the top if enabled
        if self.settings.all_labels {
            let external_lines = self.get_external_label_definitions();
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
        // This avoids matching external label headers that might be at the same address (e.g. 0)
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
        // (after external headers), not on a header whose address field happens to be 0
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
}
