//! Block range queries, splitter management, Blocks View item construction, and preamble generation.
//!
//! This module implements [`AppState`] methods for querying and mutating [`BlockType`] regions,
//! toggling manual splitters, splicing manual and virtual (scope-boundary) splitters into
//! [`BlockItem`] lists for the Blocks View, and generating synthetic preamble [`DisassemblyLine`]s
//! for external label equates and enum definitions.

use super::app_state::{AppState, BlockItem};
use super::project::Block;
use super::types::{Addr, BlockType, LabelType};
use crate::disassembler::DisassemblyLine;
use std::collections::BTreeSet;

impl AppState {
    /// Compresses [`AppState::block_types`] and [`AppState::collapsed_blocks`] into contiguous [`Block`] spans.
    #[must_use]
    pub fn get_compressed_blocks(&self) -> Vec<Block> {
        super::project::compress_block_types(&self.block_types, &self.collapsed_blocks)
    }

    /// Returns the inclusive `(start_addr, end_addr)` bounds of the contiguous [`BlockType`] sub-block
    /// containing `address`, stopping at any boundary where [`AppState::is_virtual_splitter`] is `true`,
    /// or `None` if `address` lies outside the loaded binary.
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

    /// Applies `new_type` to the byte region spanned by `selection_start..=cursor_index` (or the single
    /// disassembly line at `cursor_index` when `selection_start` is `None`), refreshes the disassembly,
    /// and returns the applied [`Command::SetBlockType`](crate::commands::Command::SetBlockType).
    ///
    /// Returns `None` if the selected disassembly lines contain no underlying binary bytes or fall
    /// outside [`AppState::block_types`].
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

    /// Toggles a manual splitter at `address` in [`AppState::splitters`] and refreshes the disassembly.
    pub fn toggle_splitter(&mut self, address: Addr) {
        if self.splitters.contains(&address) {
            self.splitters.remove(&address);
        } else {
            self.splitters.insert(address);
        }
        self.disassemble();
    }

    /// Builds the ordered list of [`BlockItem`]s (`Scope`, `Splitter`, and `Block`) for the Blocks View.
    ///
    /// Compressed [`Block`] runs are partitioned at both manual splitters ([`AppState::splitters`])
    /// and lexical scope boundaries (`scopes` starts and `scope_ends_set` exclusive ends), while
    /// [`BlockItem::Splitter`] items are emitted only for manual splitters. Items are sorted by
    /// start [`Addr`], with ties broken so [`BlockItem::Scope`] precedes [`BlockItem::Splitter`]
    /// and [`BlockItem::Block`] at the same address.
    #[must_use]
    pub fn get_blocks_view_items(&self) -> Vec<BlockItem> {
        let compressed_blocks = self.get_compressed_blocks();
        let mut items = Vec::new();

        let mut virtual_splitters = self.splitters.clone();
        virtual_splitters.extend(self.annotations.scopes().keys().copied());
        virtual_splitters.extend(self.annotations.scope_ends_set().iter().copied());

        // Convert compressed blocks to our list, splicing in splitters
        for block in compressed_blocks {
            let block_start = self.origin.wrapping_add(block.start as u16);
            let block_end = self.origin.wrapping_add(block.end as u16);

            let current_end_idx = block.end;
            let origin = self.origin;
            let mut sub_block_start = block.start;

            // Guard against address wrap-around past 0xFFFF where block_start > block_end
            // so BTreeSet::range never panics.
            if block_start <= block_end && !virtual_splitters.is_empty() {
                for &splitter_addr in virtual_splitters.range(block_start..=block_end) {
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
        for (&start_addr, &end_addr) in self.annotations.scopes() {
            let name = self
                .labels
                .get(&start_addr)
                .and_then(|l| l.first().map(|l| l.name.clone()));
            items.push(BlockItem::Scope {
                start: start_addr,
                end: end_addr,
                name,
            });
        }

        // Sort items by start address, breaking ties with `type_rank` (`Scope` = 0, `Splitter` = 1,
        // `Block` = 2) so a `BlockItem::Scope` header always precedes any `Splitter` or `Block`
        // starting at the same address, allowing `BlocksView` to indent them inside the scope.
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

    /// Returns the index in [`AppState::get_blocks_view_items`] matching `address`.
    ///
    /// When `is_splitter` is `true`, matches a [`BlockItem::Splitter`] at `address` (or a
    /// [`BlockItem::Scope`] spanning `address`). When `is_splitter` is `false`, matches the first
    /// [`BlockItem::Scope`] or [`BlockItem::Block`] whose inclusive range `start..=end` contains
    /// `address` (accounting for 16-bit wrap-around when `start > end`).
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

    /// Generates synthetic preamble [`DisassemblyLine`]s defining all external and [`BlockType::ExternalFile`]
    /// labels, grouped with a prefix legend and annotated with comments and (when `include_xrefs` is `true`)
    /// cross-reference summaries.
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
            if let Some(entry) = self.annotations.get(Addr(addr)) {
                if let Some(ref user_comment) = entry.user_side_comment {
                    comment_parts.push(user_comment.clone());
                } else if let Some(ref sys_comment) = entry.system_comment {
                    comment_parts.push(sys_comment.clone());
                }
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
            if let Some(entry) = self.annotations.get(Addr(addr)) {
                if let Some(ref user_comment) = entry.user_side_comment {
                    comment_parts.push(user_comment.clone());
                } else if let Some(ref sys_comment) = entry.system_comment {
                    comment_parts.push(sys_comment.clone());
                }
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

    /// Returns the list of disassembly lines defining all enums used in the project.
    #[must_use]
    pub fn get_enum_definition_lines(&self) -> Vec<DisassemblyLine> {
        let mut lines = Vec::new();

        // Find unique used enums
        let mut used_enum_names: BTreeSet<String> = BTreeSet::new();
        for name in self
            .annotations
            .iter()
            .filter_map(|(_, entry)| entry.enum_usage.as_ref())
        {
            used_enum_names.insert(name.clone());
        }

        if used_enum_names.is_empty() {
            return lines;
        }

        let formatter = self.get_formatter();
        let cp = formatter.comment_prefix();

        // Header: "ENUMS"
        lines.push(DisassemblyLine {
            address: Addr::ZERO,
            bytes: vec![],
            mnemonic: format!("{cp} ENUMS"),
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

        for enum_name in used_enum_names {
            if let Some(enum_def) = self.get_enum(&enum_name) {
                // Separator line before each enum definition
                lines.push(DisassemblyLine {
                    address: Addr::ZERO,
                    bytes: vec![],
                    mnemonic: format!("{cp}   Enum: {}", enum_def.name),
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

                let enum_block = formatter.format_enum_definition(enum_def);
                for block_line in enum_block.lines() {
                    lines.push(DisassemblyLine {
                        address: Addr::ZERO,
                        bytes: vec![],
                        mnemonic: block_line.to_string(),
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
        }

        // Trailing blank line
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

    // T2.1
    #[test]
    fn test_is_virtual_splitter_manual_and_scopes() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);

        // Empty state fast-path
        assert!(!app_state.is_virtual_splitter(Addr(0x1005)));

        // Manual splitter at $1005
        app_state.splitters.insert(Addr(0x1005));
        // Scope [$1010..=$101F], so exclusive end is $1020
        app_state
            .annotations
            .update(Addr(0x1010), |e| e.scope = Some(Addr(0x101F)));

        assert!(app_state.is_virtual_splitter(Addr(0x1005)));
        assert!(app_state.is_virtual_splitter(Addr(0x1010)));
        assert!(app_state.is_virtual_splitter(Addr(0x1020)));

        assert!(!app_state.is_virtual_splitter(Addr(0x1000)));
        assert!(!app_state.is_virtual_splitter(Addr(0x1004)));
        assert!(!app_state.is_virtual_splitter(Addr(0x1015)));
        assert!(!app_state.is_virtual_splitter(Addr(0x101F)));
        assert!(!app_state.is_virtual_splitter(Addr(0x1021)));
    }

    // T2.2
    #[test]
    fn test_get_block_range_respects_scopes_and_splitters() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0x00; 32];
        app_state.block_types = vec![BlockType::DataByte; 32];

        // Scope $1008..=$100F (virtual splitters at $1008 and $1010)
        app_state
            .annotations
            .update(Addr(0x1008), |e| e.scope = Some(Addr(0x100F)));
        // Manual splitter at $1018
        app_state.splitters.insert(Addr(0x1018));

        assert_eq!(
            app_state.get_block_range(Addr(0x1002)),
            Some((Addr(0x1000), Addr(0x1007)))
        );
        assert_eq!(
            app_state.get_block_range(Addr(0x1008)),
            Some((Addr(0x1008), Addr(0x100F)))
        );
        assert_eq!(
            app_state.get_block_range(Addr(0x100F)),
            Some((Addr(0x1008), Addr(0x100F)))
        );
        assert_eq!(
            app_state.get_block_range(Addr(0x1010)),
            Some((Addr(0x1010), Addr(0x1017)))
        );
        assert_eq!(
            app_state.get_block_range(Addr(0x1018)),
            Some((Addr(0x1018), Addr(0x101F)))
        );
    }

    // T2.3
    #[test]
    fn test_get_blocks_view_items_splitter_positions_landmine_d() {
        use super::super::app_state::BlockItem;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0x00; 10];
        app_state.block_types = vec![BlockType::DataByte; 10];

        // 1. Splitter at block start ($1000)
        app_state.splitters.insert(Addr(0x1000));
        // 2. Adjacent splitters in the middle ($1004, $1005)
        app_state.splitters.insert(Addr(0x1004));
        app_state.splitters.insert(Addr(0x1005));
        // 3. Splitter at block end ($1009)
        app_state.splitters.insert(Addr(0x1009));
        // 4. Out-of-bounds splitter outside binary ($2000)
        app_state.splitters.insert(Addr(0x2000));

        let items = app_state.get_blocks_view_items();
        assert_eq!(
            items,
            vec![
                BlockItem::Splitter(Addr(0x1000)),
                BlockItem::Block {
                    start: Addr(0x1000),
                    end: Addr(0x1003),
                    type_: BlockType::DataByte,
                    collapsed: false,
                },
                BlockItem::Splitter(Addr(0x1004)),
                BlockItem::Block {
                    start: Addr(0x1004),
                    end: Addr(0x1004),
                    type_: BlockType::DataByte,
                    collapsed: false,
                },
                BlockItem::Splitter(Addr(0x1005)),
                BlockItem::Block {
                    start: Addr(0x1005),
                    end: Addr(0x1008),
                    type_: BlockType::DataByte,
                    collapsed: false,
                },
                BlockItem::Splitter(Addr(0x1009)),
                BlockItem::Block {
                    start: Addr(0x1009),
                    end: Addr(0x1009),
                    type_: BlockType::DataByte,
                    collapsed: false,
                },
            ]
        );
    }

    // T2.4
    #[test]
    fn test_get_blocks_view_items_with_scopes_and_labels() {
        use super::super::app_state::BlockItem;
        use super::super::project::Label;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xEA; 32];
        app_state.block_types = vec![BlockType::Code; 32];

        // Scope $1008..=$100F with label "my_scope" and manual splitter at $1008
        app_state
            .annotations
            .update(Addr(0x1008), |e| e.scope = Some(Addr(0x100F)));
        app_state.labels.insert(
            Addr(0x1008),
            vec![Label {
                name: "my_scope".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        app_state.splitters.insert(Addr(0x1008));

        let items = app_state.get_blocks_view_items();
        assert_eq!(
            items,
            vec![
                BlockItem::Block {
                    start: Addr(0x1000),
                    end: Addr(0x1007),
                    type_: BlockType::Code,
                    collapsed: false,
                },
                BlockItem::Scope {
                    start: Addr(0x1008),
                    end: Addr(0x100F),
                    name: Some("my_scope".to_string()),
                },
                BlockItem::Splitter(Addr(0x1008)),
                BlockItem::Block {
                    start: Addr(0x1008),
                    end: Addr(0x100F),
                    type_: BlockType::Code,
                    collapsed: false,
                },
                BlockItem::Block {
                    start: Addr(0x1010),
                    end: Addr(0x101F),
                    type_: BlockType::Code,
                    collapsed: false,
                },
            ]
        );
    }

    // T2.5
    #[test]
    fn test_get_blocks_view_items_address_wrap_around_landmine_c() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0xFFFE);
        app_state.raw_data = vec![0xEA; 4];
        app_state.block_types = vec![BlockType::Code; 4];
        app_state.splitters.insert(Addr(0xFFFF));
        app_state
            .annotations
            .update(Addr(0xFFFE), |e| e.scope = Some(Addr(0xFFFF)));

        // Must not panic when block_start ($FFFE) > block_end ($0001)
        let items = app_state.get_blocks_view_items();
        assert!(!items.is_empty());
    }

    // T2.6
    #[test]
    fn test_get_blocks_view_items_full_64k_boundary() {
        use super::super::app_state::BlockItem;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0000);
        app_state.raw_data = vec![0xEA; 65536];
        app_state.block_types = vec![BlockType::Code; 65536];
        app_state.splitters.insert(Addr(0x0000));
        app_state.splitters.insert(Addr(0x8000));
        app_state.splitters.insert(Addr(0xFFFF));
        app_state
            .annotations
            .update(Addr(0xC000), |e| e.scope = Some(Addr(0xFFFF)));

        let items = app_state.get_blocks_view_items();
        assert!(items.contains(&BlockItem::Splitter(Addr(0x0000))));
        assert!(items.contains(&BlockItem::Splitter(Addr(0x8000))));
        assert!(items.contains(&BlockItem::Splitter(Addr(0xFFFF))));
        assert!(items.contains(&BlockItem::Scope {
            start: Addr(0xC000),
            end: Addr(0xFFFF),
            name: None,
        }));
    }

    // T4.1
    #[test]
    fn test_performance_64k_blocks_view_and_block_range_with_2000_annotations() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0000);
        app_state.raw_data = vec![0xEA; 65536];
        app_state.block_types = vec![BlockType::Code; 65536];

        // Populate 2,000 system comments across the 64 KB address space
        for i in 0..2000u16 {
            let addr = Addr(i.wrapping_mul(31));
            app_state
                .annotations
                .update(addr, |e| e.system_comment = Some(format!("sys_{i}")));
        }

        // Populate 10 user scopes and 10 manual splitters
        for i in 0..10u16 {
            let start = Addr(0x1000 + i * 0x0400);
            let end = Addr(0x1000 + i * 0x0400 + 0x00FF);
            app_state.annotations.update(start, |e| e.scope = Some(end));
            app_state.splitters.insert(Addr(0x2000 + i * 0x0400));
        }

        // Populate 5 unsorted collapsed blocks
        app_state.collapsed_blocks = vec![
            (50000, 50100),
            (10000, 10200),
            (30000, 30050),
            (100, 200),
            (40000, 40500),
        ];

        let start_time = std::time::Instant::now();
        for _ in 0..10 {
            let items = app_state.get_blocks_view_items();
            assert!(!items.is_empty());
            let range = app_state.get_block_range(Addr(0x8000));
            assert!(range.is_some());
        }
        let elapsed = start_time.elapsed();
        assert!(
            elapsed.as_millis() < 200,
            "10x get_blocks_view_items + 10x get_block_range on 64KB with 2000 annotations took {elapsed:?} (expected < 200ms)"
        );
    }
}
