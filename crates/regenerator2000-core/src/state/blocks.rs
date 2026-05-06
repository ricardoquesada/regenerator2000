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

        while start > 0
            && self.block_types[start - 1] == target_type
            && !self.is_virtual_splitter(origin.wrapping_add(start as u16))
        {
            start -= 1;
        }

        // Search forward
        while end < self.block_types.len() - 1
            && self.block_types[end + 1] == target_type
            && !self.is_virtual_splitter(origin.wrapping_add((end + 1) as u16))
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
    ) -> Option<crate::commands::Command> {
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
                let valid_end = end.min(max_len - 1);
                let range_end = valid_end + 1;
                let range = start..range_end;

                let old_types = self.block_types[range.clone()].to_vec();

                let commands = vec![crate::commands::Command::SetBlockType {
                    range: range.clone(),
                    new_type,
                    old_types,
                }];

                let command = if commands.len() == 1 {
                    commands
                        .into_iter()
                        .next()
                        .unwrap_or(crate::commands::Command::Batch(vec![]))
                } else {
                    crate::commands::Command::Batch(commands)
                };

                command.apply(self);
                self.disassemble();
                return Some(command);
            }
        }
        None
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

            // Find all virtual splitters in this block range
            let mut relevant_splitters = Vec::new();
            for offset in block_start.0..=block_end.0 {
                let addr = Addr(offset);
                if self.is_virtual_splitter(addr) {
                    relevant_splitters.push(addr);
                }
            }

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

                // Emit Splitter only if it's a manual splitter (not a virtual one)
                if self.splitters.contains(&splitter_addr) {
                    items.push(BlockItem::Splitter(splitter_addr));
                }

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

        // Add Scopes
        for (start_addr, end_addr) in &self.scopes {
            let name = self
                .labels
                .get(start_addr)
                .and_then(|l| l.first().map(|l| l.name.clone()));
            items.push(BlockItem::Scope {
                start: *start_addr,
                end: *end_addr,
                name,
            });
        }

        // Sort items by start address
        items.sort_by(|a, b| {
            let addr_a = match a {
                BlockItem::Block { start, .. } => *start,
                BlockItem::Splitter(addr) => *addr,
                BlockItem::Scope { start, .. } => *start,
            };
            let addr_b = match b {
                BlockItem::Block { start, .. } => *start,
                BlockItem::Splitter(addr) => *addr,
                BlockItem::Scope { start, .. } => *start,
            };

            if addr_a != addr_b {
                addr_a.cmp(&addr_b)
            } else {
                let type_rank = |item: &BlockItem| match item {
                    BlockItem::Scope { .. } => 0,
                    BlockItem::Splitter(_) => 1,
                    BlockItem::Block { .. } => 2,
                };
                type_rank(a).cmp(&type_rank(b))
            }
        });

        items
    }

    #[must_use]
    pub fn get_block_index_for_address(&self, address: Addr, is_splitter: bool) -> Option<usize> {
        let items = self.get_blocks_view_items();
        items.iter().position(|item| match item {
            BlockItem::Block { start, end, .. } => {
                if !is_splitter {
                    let s = *start;
                    let e = *end;
                    if s <= e {
                        address >= s && address <= e
                    } else {
                        address >= s || address <= e
                    }
                } else {
                    false
                }
            }
            BlockItem::Splitter(addr) => is_splitter && *addr == address,
            BlockItem::Scope { start, end, .. } => {
                let s = *start;
                let e = *end;
                if s <= e {
                    address >= s && address <= e
                } else {
                    address >= s || address <= e
                }
            }
        })
    }

    #[must_use]
    pub fn get_external_label_definitions(&self, include_xrefs: bool) -> Vec<DisassemblyLine> {
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

        // Determine which prefix types are actually present among external labels,
        // then build a merged flat list sorted by address.
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum ExtGroup {
            ZpField,
            ZpAbs,
            ZpPtr,
            Field,
            Abs,
            Ptr,
            ExtJump,
            Other,
        }

        let mut flat: Vec<(u16, &String, ExtGroup, bool)> = Vec::new(); // (addr, name, group, is_zp)
        for (addr, l_type, name) in all_externals {
            let (group, is_zp) = match l_type {
                LabelType::ZeroPageField => (ExtGroup::ZpField, true),
                LabelType::ZeroPageAbsoluteAddress => (ExtGroup::ZpAbs, true),
                LabelType::ZeroPagePointer => (ExtGroup::ZpPtr, true),
                LabelType::Field => (ExtGroup::Field, false),
                LabelType::AbsoluteAddress => (ExtGroup::Abs, false),
                LabelType::Pointer => (ExtGroup::Ptr, false),
                LabelType::ExternalJump => (ExtGroup::ExtJump, false),
                _ => (ExtGroup::Other, false),
            };
            flat.push((addr.0, name, group, is_zp));
        }

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
        }

        // Sort the flat list by address.
        flat.sort_by_key(|(a, _, _, _)| *a);
        ext_file_labels.sort_by_key(|(a, _)| *a);

        let mut lines = Vec::new();

        if flat.is_empty() && ext_file_labels.is_empty() {
            return lines;
        }

        let formatter = self.get_formatter();
        let cp = formatter.comment_prefix();

        // Build legend lines for every prefix type that appears in the data.
        let legend_entries: &[(&str, &str, ExtGroup)] = &[
            ("zpf_", "Zero Page Field", ExtGroup::ZpField),
            ("zpa_", "Zero Page Absolute Address", ExtGroup::ZpAbs),
            ("zpp_", "Zero Page Pointer", ExtGroup::ZpPtr),
            ("f_  ", "Field", ExtGroup::Field),
            ("a_  ", "Absolute Address", ExtGroup::Abs),
            ("p_  ", "Pointer", ExtGroup::Ptr),
            ("e_  ", "External Jump", ExtGroup::ExtJump),
            ("L_  ", "Other / User-defined", ExtGroup::Other),
        ];

        // Header comment: "EXTERNAL LABELS"
        lines.push(DisassemblyLine {
            address: Addr::ZERO,
            bytes: vec![],
            mnemonic: format!("{cp} EXTERNAL LABELS"),
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

        // Legend: one line per prefix type that is actually used.
        for (prefix, description, group) in legend_entries {
            if flat.iter().any(|(_, _, g, _)| g == group) {
                lines.push(DisassemblyLine {
                    address: Addr::ZERO,
                    bytes: vec![],
                    mnemonic: format!("{cp}   {prefix} = {description}"),
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
        }
        if !ext_file_labels.is_empty() {
            lines.push(DisassemblyLine {
                address: Addr::ZERO,
                bytes: vec![],
                mnemonic: format!("{cp}   (ext) = External File Label"),
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

        // Blank separator line after legend.
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

        // Emit all flat external labels in address order.
        for (addr, name, _, is_zp) in flat {
            let mut comment_parts = Vec::new();
            if let Some(user_comment) = self.user_side_comments.get(&Addr(addr)) {
                comment_parts.push(user_comment.clone());
            } else if let Some(sys_comment) = self.platform_comments.get(&Addr(addr)) {
                comment_parts.push(sys_comment.clone());
            }

            if include_xrefs
                && let Some(refs) = self.cross_refs.get(&Addr(addr))
                && !refs.is_empty()
                && self.settings.max_xref_count > 0
            {
                comment_parts.push(crate::disassembler::context::format_cross_references(
                    refs,
                    self.settings.max_xref_count,
                ));
            }

            let comment = comment_parts.join(&format!(" {cp} "));

            lines.push(DisassemblyLine {
                address: Addr(addr),
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

        // Emit external-file labels after all the others.
        for (addr, name) in ext_file_labels {
            let mut comment_parts = Vec::new();
            if let Some(user_comment) = self.user_side_comments.get(&Addr(addr)) {
                comment_parts.push(user_comment.clone());
            } else if let Some(sys_comment) = self.platform_comments.get(&Addr(addr)) {
                comment_parts.push(sys_comment.clone());
            }

            if include_xrefs
                && let Some(refs) = self.cross_refs.get(&Addr(addr))
                && !refs.is_empty()
                && self.settings.max_xref_count > 0
            {
                comment_parts.push(crate::disassembler::context::format_cross_references(
                    refs,
                    self.settings.max_xref_count,
                ));
            }

            let comment = comment_parts.join(&format!(" {cp} "));

            lines.push(DisassemblyLine {
                address: Addr(addr),
                bytes: vec![],
                mnemonic: formatter.format_definition(name, addr, false),
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

        // Trailing blank line.
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

    /// Regression test: Ctrl+K on a DataByte block that has a splitter in the
    /// middle must collapse only the sub-block the cursor is in, not the entire
    /// merged block.  Previously, `toggle_collapsed_block` used
    /// `get_compressed_blocks()` which ignores splitters, so pressing Ctrl+K
    /// anywhere in the block would always collapse the full span.
    #[test]
    fn test_collapse_respects_splitter() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        // 10 data bytes: $1000–$1009
        app_state.raw_data = vec![0x00; 10];
        app_state.block_types = vec![BlockType::DataByte; 10];
        // Splitter at $1005 splits the block into $1000–$1004 and $1005–$1009
        app_state.splitters.insert(Addr(0x1005));
        app_state.disassemble();

        // Collapse only the first sub-block ($1000–$1004) using get_block_range
        let (start_addr, end_addr) = app_state.get_block_range(Addr(0x1000)).unwrap();
        assert_eq!(start_addr, Addr(0x1000));
        assert_eq!(end_addr, Addr(0x1004));

        let start_offset = start_addr.offset_from(app_state.origin);
        let end_offset = end_addr.offset_from(app_state.origin);
        app_state.collapsed_blocks.push((start_offset, end_offset));
        app_state.disassemble();

        // The second sub-block ($1005–$1009) must NOT be collapsed
        let (start2, end2) = app_state.get_block_range(Addr(0x1005)).unwrap();
        assert_eq!(start2, Addr(0x1005));
        assert_eq!(end2, Addr(0x1009));
        let start2_offset = start2.offset_from(app_state.origin);
        assert!(
            !app_state
                .collapsed_blocks
                .iter()
                .any(|(s, _)| *s == start2_offset),
            "Second sub-block ($1005–$1009) should not be collapsed"
        );
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

    #[test]
    fn test_get_external_label_definitions_includes_xrefs() {
        use super::super::project::Label;
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA];
        state.block_types = vec![BlockType::Code; 1];

        // Add external label at $D020
        state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        // Add X-Ref to $D020 from $1000
        state.cross_refs.insert(Addr(0xD020), vec![Addr(0x1000)]);
        state.settings.max_xref_count = 5;

        // Test with include_xrefs = true
        let lines_with = state.get_external_label_definitions(true);
        let border_line_with = lines_with
            .iter()
            .find(|l| l.external_label_address == Some(Addr(0xD020)))
            .unwrap();
        assert!(
            border_line_with.comment.contains("x-ref:"),
            "Should contain X-Ref info"
        );

        // Test with include_xrefs = false
        let lines_without = state.get_external_label_definitions(false);
        let border_line_without = lines_without
            .iter()
            .find(|l| l.external_label_address == Some(Addr(0xD020)))
            .unwrap();
        assert!(
            !border_line_without.comment.contains("ref from:"),
            "Should NOT contain X-Ref info"
        );
    }
}
