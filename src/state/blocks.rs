use super::app_state::{AppState, BlockItem};
use super::project::Block;
use super::types::{Addr, BlockType, LabelType};
use crate::disassembler::DisassemblyLine;

impl AppState {
    #[must_use]
    pub fn get_compressed_blocks(&self) -> Vec<Block> {
        super::project::compress_block_types(&self.block_types, &self.collapsed_blocks)
    }

    #[must_use]
    pub fn get_block_range(&self, address: Addr) -> Option<(Addr, Addr)> {
        let origin = self.origin;
        if address < origin {
            return None;
        }
        let index = address.offset_from(origin);
        if index >= self.block_types.len() {
            return None;
        }

        let target_type = self.block_types[index];
        let mut start = index;
        let mut end = index;

        // Search backward
        while start > 0
            && self.block_types[start - 1] == target_type
            && !self.splitters.contains(&origin.wrapping_add(start as u16))
        {
            start -= 1;
        }

        // Search forward
        while end < self.block_types.len() - 1
            && self.block_types[end + 1] == target_type
            && !self
                .splitters
                .contains(&origin.wrapping_add((end + 1) as u16))
        {
            end += 1;
        }

        let start_addr = origin.wrapping_add(start as u16);
        let end_addr = origin.wrapping_add(end as u16);

        Some((start_addr, end_addr))
    }

    pub fn set_block_type_region(
        &mut self,
        new_type: BlockType,
        selection_start: Option<usize>,
        cursor_index: usize,
    ) {
        let range_opt = if let Some(selection_start) = selection_start {
            let (s, e) = if selection_start < cursor_index {
                (selection_start, cursor_index)
            } else {
                (cursor_index, selection_start)
            };

            // Find first and last lines with bytes in the selected range to determine the byte region
            let first_with_bytes =
                (s..=e).find(|&i| self.disassembly.get(i).is_some_and(|l| !l.bytes.is_empty()));
            let last_with_bytes = (s..=e)
                .rev()
                .find(|&i| self.disassembly.get(i).is_some_and(|l| !l.bytes.is_empty()));

            if let (Some(fs), Some(fe)) = (first_with_bytes, last_with_bytes) {
                let start_line = &self.disassembly[fs];
                let end_line = &self.disassembly[fe];

                let start_addr = start_line.address;
                let end_addr_inclusive = end_line
                    .address
                    .wrapping_add(end_line.bytes.len() as u16)
                    .wrapping_sub(1);

                let start_idx = start_addr.offset_from(self.origin);
                let end_idx = end_addr_inclusive.offset_from(self.origin);

                Some((start_idx, end_idx))
            } else {
                None
            }
        } else {
            // Single line action
            if let Some(line) = self.disassembly.get(cursor_index) {
                if line.bytes.is_empty() {
                    None
                } else {
                    let start_addr = line.address;
                    let end_addr_inclusive = line
                        .address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1);

                    let start_idx = start_addr.offset_from(self.origin);
                    let end_idx = end_addr_inclusive.offset_from(self.origin);
                    Some((start_idx, end_idx))
                }
            } else {
                None
            }
        };

        if let Some((start, end)) = range_opt {
            // Boundary check
            let max_len = self.block_types.len();
            if start < max_len {
                let valid_end = end.min(max_len);
                let range_end = valid_end + 1;
                let range = start..range_end;

                let old_types = self.block_types[range.clone()].to_vec();

                let command = crate::commands::Command::SetBlockType {
                    range: range.clone(),
                    new_type,
                    old_types,
                };

                command.apply(self);
                self.push_command(command);

                self.disassemble();
            }
        }
    }

    pub fn toggle_splitter(&mut self, address: Addr) {
        // Toggle splitter for the generic address
        if self.splitters.contains(&address) {
            self.splitters.remove(&address);
        } else {
            self.splitters.insert(address);
        }
        self.disassemble();
    }

    #[must_use]
    pub fn get_blocks_view_items(&self) -> Vec<BlockItem> {
        let compressed_blocks = self.get_compressed_blocks();
        let mut items = Vec::new();

        // Convert compressed blocks to our list, splicing in splitters
        for block in compressed_blocks {
            let block_start = self.origin.wrapping_add(block.start as u16);
            let block_end = self.origin.wrapping_add(block.end as u16);

            let current_end_idx = block.end;

            // Filter splitters relevant to this block range
            let relevant_splitters: Vec<Addr> = self
                .splitters
                .range(block_start..=block_end)
                .copied()
                .collect();

            let origin = self.origin;
            let mut sub_block_start = block.start;

            for splitter_addr in relevant_splitters {
                // Convert splitter address to index
                let splitter_idx = splitter_addr.offset_from(origin);

                // If splitter is outside current bounds (shouldn't happen due to range filter), skip.
                if splitter_idx < sub_block_start || splitter_idx > current_end_idx {
                    continue;
                }

                // If splitter is > sub_block_start, we have a chunk before the splitter.
                if splitter_idx > sub_block_start {
                    items.push(BlockItem::Block {
                        start: origin.wrapping_add(sub_block_start as u16),
                        end: origin.wrapping_add((splitter_idx - 1) as u16),
                        type_: block.type_,
                        collapsed: block.collapsed,
                    });
                }

                // Emit Splitter
                items.push(BlockItem::Splitter(splitter_addr));

                sub_block_start = splitter_idx;
            }

            // Emit remainder
            if sub_block_start <= current_end_idx {
                items.push(BlockItem::Block {
                    start: origin.wrapping_add(sub_block_start as u16),
                    end: origin.wrapping_add(current_end_idx as u16),
                    type_: block.type_,
                    collapsed: block.collapsed,
                });
            }
        }

        items
    }

    #[must_use]
    pub fn get_block_index_for_address(&self, address: Addr) -> Option<usize> {
        let items = self.get_blocks_view_items();
        items.iter().position(|item| match item {
            BlockItem::Block { start, end, .. } => {
                let s = *start;
                let e = *end;
                // Check if address is within [s, e]
                if s <= e {
                    address >= s && address <= e
                } else {
                    // Wrap around
                    address >= s || address <= e
                }
            }
            BlockItem::Splitter(addr) => *addr == address,
        })
    }

    #[must_use]
    pub fn get_external_label_definitions(&self) -> Vec<DisassemblyLine> {
        let mut candidates: Vec<(Addr, LabelType, &String)> = Vec::new();

        for (addr, labels) in &self.labels {
            if self.is_external(*addr) {
                // Only include if setting enabled

                if let Some(label) =
                    crate::disassembler::resolve_label(labels, (*addr).into(), &self.settings)
                {
                    candidates.push((*addr, label.label_type, &label.name));
                }
            }
        }

        let mut seen_names = std::collections::HashSet::new();
        let mut all_externals = Vec::new();

        for item in candidates {
            let name = item.2;
            if !seen_names.contains(name) {
                seen_names.insert(name);
                all_externals.push(item);
            }
        }

        let mut zp_fields = Vec::new();
        let mut zp_abs = Vec::new();
        let mut zp_ptrs = Vec::new();
        let mut fields = Vec::new();
        let mut abs = Vec::new();
        let mut ptrs = Vec::new();
        let mut ext_jumps = Vec::new();
        let mut others = Vec::new();

        for (addr, l_type, name) in all_externals {
            match l_type {
                LabelType::ZeroPageField => zp_fields.push((addr.0, name)),
                LabelType::ZeroPageAbsoluteAddress => zp_abs.push((addr.0, name)),
                LabelType::ZeroPagePointer => zp_ptrs.push((addr.0, name)),
                LabelType::Field => fields.push((addr.0, name)),
                LabelType::AbsoluteAddress => abs.push((addr.0, name)),
                LabelType::Pointer => ptrs.push((addr.0, name)),
                LabelType::ExternalJump => ext_jumps.push((addr.0, name)),
                _ => others.push((addr.0, name)),
            }
        }

        let sort_group = |group: &mut Vec<(u16, &String)>| {
            group.sort_by_key(|(a, _)| *a);
        };

        sort_group(&mut zp_fields);
        sort_group(&mut zp_abs);
        sort_group(&mut zp_ptrs);
        sort_group(&mut fields);
        sort_group(&mut abs);
        sort_group(&mut ptrs);
        sort_group(&mut ext_jumps);
        sort_group(&mut others);

        // Collect labels whose addresses fall inside ExternalFile blocks.
        let mut ext_file_labels: Vec<(u16, &String)> = Vec::new();
        {
            let mut seen: std::collections::HashSet<&String> = std::collections::HashSet::new();
            for (addr, labels) in &self.labels {
                if !self.is_external(*addr) {
                    let offset = addr.offset_from(self.origin);
                    if offset < self.block_types.len()
                        && self.block_types[offset] == BlockType::ExternalFile
                        && let Some(label) = crate::disassembler::resolve_label(
                            labels,
                            (*addr).into(),
                            &self.settings,
                        )
                        && seen.insert(&label.name)
                    {
                        ext_file_labels.push((addr.0, &label.name));
                    }
                }
            }
            ext_file_labels.sort_by_key(|(a, _)| *a);
        }

        let mut lines = Vec::new();

        let formatter = self.get_formatter();

        let mut add_group = |title: &str, group: Vec<(u16, &String)>, is_zp: bool| {
            if !group.is_empty() {
                lines.push(DisassemblyLine {
                    address: Addr::ZERO,
                    bytes: vec![],
                    mnemonic: format!("{} {}", formatter.comment_prefix(), title),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                });

                for (addr, name) in group {
                    // Logic for side comment
                    let mut comment = String::new();
                    if let Some(user_comment) = self.user_side_comments.get(&Addr(addr)) {
                        comment = user_comment.clone();
                    } else if let Some(sys_comment) = self.system_comments.get(&Addr(addr)) {
                        comment = sys_comment.clone();
                    }

                    lines.push(DisassemblyLine {
                        // address field is Addr
                        address: Addr::ZERO,
                        bytes: vec![],
                        mnemonic: formatter.format_definition(name, addr, is_zp),
                        operand: String::new(),
                        comment,
                        line_comment: None,
                        label: None,
                        opcode: None,
                        show_bytes: true,
                        target_address: None,
                        external_label_address: Some(Addr(addr)),
                        is_collapsed: false,
                    });
                }

                lines.push(DisassemblyLine {
                    address: Addr::ZERO,
                    bytes: vec![],
                    mnemonic: String::new(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                });
            }
        };

        add_group("ZP FIELDS", zp_fields, true);
        add_group("ZP ABSOLUTE ADDRESSES", zp_abs, true);
        add_group("ZP POINTERS", zp_ptrs, true);
        add_group("FIELDS", fields, false);
        add_group("ABSOLUTE ADDRESSES", abs, false);
        add_group("POINTERS", ptrs, false);
        add_group("EXTERNAL JUMPS", ext_jumps, false);
        add_group("EXTERNAL FILE LABELS", ext_file_labels, false);
        add_group("OTHERS", others, false);

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::super::app_state::AppState;
    use super::super::types::*;

    #[test]
    fn test_get_block_range_respects_splitters() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);

        // All bytes
        app_state.raw_data = vec![0xEA; 10];
        app_state.block_types = vec![BlockType::DataByte; 10];

        // Splitter at $1005
        app_state.splitters.insert(Addr(0x1005));

        // Query $1003 → block should be $1000..$1004 (stops before splitter)
        let range1 = app_state.get_block_range(Addr(0x1003));
        assert_eq!(range1, Some((Addr(0x1000), Addr(0x1004))));

        // Query $1005 → block should be $1005..$1009 (starts at splitter)
        let range2 = app_state.get_block_range(Addr(0x1005));
        assert_eq!(range2, Some((Addr(0x1005), Addr(0x1009))));
    }

    #[test]
    fn test_set_block_type_lohi_creates_labels() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0xC000);

        // 4 bytes: two pairs (lo/hi for $1234 and $5678)
        app_state.raw_data = vec![0x34, 0x12, 0x78, 0x56];
        app_state.block_types = vec![BlockType::DataByte; 4];
        app_state.disassemble();

        // Set as LoHiAddress
        app_state.set_block_type_region(BlockType::LoHiAddress, Some(0), 3);

        // Verify block types changed
        for bt in &app_state.block_types {
            assert_eq!(*bt, BlockType::LoHiAddress);
        }
    }
}
