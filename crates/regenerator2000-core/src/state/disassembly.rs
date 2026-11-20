//! Disassembly pass execution, control-flow arrow caching, address-to-line index lookup,
//! and operand jump-target resolution.

use super::app_state::AppState;
use super::types::{
    Addr, BlockType, CachedArrow, ExternalXRefs, ImmediateFormat, OffsetParentheses,
    OperandJumpTarget,
};
use crate::cpu::AddressingMode;
use crate::disassembler::context::DisassemblyContext;
use crate::disassembler::formatter::Formatter;
use crate::disassembler::symbols::{
    apply_local_prefix, resolve_effective_symbolic_offset,
    resolve_effective_symbolic_offset_for_mode, resolve_label,
    resolve_split_pair_effective_symbolic_offset, resolve_split_pair_symbolic_label_name,
    resolve_symbolic_label_name,
};
use crate::disassembler::{Disassembler, DisassemblyLine};
use crate::navigation::{self, JumpTargetIndex};
use std::collections::{BTreeMap, BTreeSet};

impl AppState {
    /// Disassembles [`AppState::raw_data`] into [`AppState::disassembly`] and updates [`AppState::cached_arrows`].
    pub fn disassemble(&mut self) {
        let ctx = DisassemblyContext {
            data: &self.raw_data,
            block_types: &self.block_types,
            labels: &self.labels,
            origin: self.origin,
            settings: &self.settings,
            annotations: &self.annotations,
            cross_refs: &self.cross_refs,
            collapsed_blocks: &self.collapsed_blocks,
            splitters: &self.splitters,
            scope_ends: self.annotations.scope_ends_set(),
            enums: &self.enums,
            user_global_enums: &self.user_global_enums,
            builtin_enums: &self.builtin_enums,
        };
        let mut lines = self.disassembler.disassemble_ctx(&ctx);

        // Add external label definitions at the top if enabled
        if self.settings.all_labels {
            let enum_lines = self.enum_definition_lines();
            let external_lines = self.external_label_definitions(ExternalXRefs::Include);
            // Prepend enums, then external lines (so external lines appear first)
            lines.splice(0..0, enum_lines);
            lines.splice(0..0, external_lines);
        }

        self.disassembly = lines;
        self.compute_cached_arrows();
    }

    /// Recomputes [`AppState::cached_arrows`] for all in-range control-flow instructions in [`AppState::disassembly`].
    pub fn compute_cached_arrows(&mut self) {
        let mut arrows = Vec::new();

        if self.disassembly.is_empty() {
            self.cached_arrows = arrows;
            return;
        }

        // Build a temporary index for in-range disassembly body lines.
        // Skip external equate headers and preamble comment rows so arrows never target them.
        // Map: Address -> Line Index
        let mut addr_to_idx = BTreeMap::new();
        for (i, line) in self.disassembly.iter().enumerate() {
            if line.external_label_address.is_some() || self.is_external(line.address) {
                continue;
            }
            if !line.bytes.is_empty() || line.is_collapsed {
                addr_to_idx.insert(line.address, i);
            } else if line.label.is_some() {
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
                    // or into a collapsed block.
                    if dst_idx_opt.is_none() {
                        // Find the last line with address <= target_addr
                        if let Some((&base_addr, &base_idx)) =
                            addr_to_idx.range(..=target_addr).next_back()
                        {
                            let base_line = &self.disassembly[base_idx];
                            let span_len = if base_line.is_collapsed {
                                let start_offset = base_addr.offset_from(self.origin);
                                self.collapsed_blocks
                                    .iter()
                                    .find(|&&(s, _)| s == start_offset)
                                    .map_or(0, |&(s, e)| (e.saturating_sub(s) + 1) as u16)
                            } else {
                                base_line.bytes.len() as u16
                            };
                            if span_len > 0
                                && target_addr.0 >= base_addr.0
                                && target_addr.0 < base_addr.0.wrapping_add(span_len)
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

    /// Returns the index of the [`DisassemblyLine`] best matching `address`.
    ///
    /// Prioritizes content lines with bytes over label-only or external equate header lines.
    #[must_use]
    pub fn line_index_for_address(&self, address: Addr) -> Option<usize> {
        // First pass: exact match on an in-binary content line with non-empty bytes.
        if let Some(idx) = self.disassembly.iter().position(|line| {
            line.external_label_address.is_none()
                && line.address == address
                && !line.bytes.is_empty()
        }) {
            return Some(idx);
        }

        // Second pass: exact match on a zero-byte in-binary body line (such as a collapsed block or splitter).
        if !self.is_external(address)
            && let Some(idx) = self
                .disassembly
                .iter()
                .position(|line| line.external_label_address.is_none() && line.address == address)
        {
            return Some(idx);
        }

        // Third pass: external label equate in the preamble.
        if let Some(idx) = self
            .disassembly
            .iter()
            .position(|line| line.external_label_address == Some(address))
        {
            return Some(idx);
        }

        // Fourth pass: first in-binary content line (non-empty bytes) with `address >= target`.
        if let Some(idx) = self.disassembly.iter().position(|line| {
            line.external_label_address.is_none()
                && !line.bytes.is_empty()
                && line.address >= address
        }) {
            return Some(idx);
        }

        // Last resort: any line with `address >= target`.
        self.disassembly
            .iter()
            .position(|line| line.address >= address)
    }

    /// Returns the index of the [`DisassemblyLine`] whose byte span or collapsed range contains `address`.
    ///
    /// Prioritizes collapsed ranges and byte-bearing spans ahead of zero-byte lines.
    /// It ignores unlabelled `$0000` preamble rows (`; EXTERNAL LABELS`, `; ENUMS`, blank
    /// separators) so `$0000` lookups hit only body or equate rows.
    #[must_use]
    pub fn line_index_containing_address(&self, address: Addr) -> Option<usize> {
        let is_ext = self.is_external(address);

        if !is_ext {
            for (start_idx, end_idx) in &self.collapsed_blocks {
                let start_addr = self.origin.wrapping_add(*start_idx as u16);
                let end_addr = self.origin.wrapping_add(*end_idx as u16);
                if navigation::address_in_inclusive_span(address, start_addr, end_addr)
                    && let Some(idx) = self
                        .disassembly
                        .iter()
                        .position(|line| line.is_collapsed && line.address == start_addr)
                {
                    return Some(idx);
                }
            }
        }

        // Match byte-bearing body rows before zero-byte rows so `$0000` preamble headers never shadow real spans.
        if let Some(idx) = self.disassembly.iter().position(|line| {
            line.external_label_address.is_none()
                && !line.bytes.is_empty()
                && navigation::address_in_byte_span(address, line.address, line.bytes.len())
        }) {
            return Some(idx);
        }

        if is_ext {
            return self
                .disassembly
                .iter()
                .position(|line| line.external_label_address == Some(address));
        }

        // Collapsed summary rows emit `bytes: []`; match them before non-collapsed zero-byte headers.
        if let Some(idx) = self
            .disassembly
            .iter()
            .position(|line| line.is_collapsed && line.address == address)
        {
            return Some(idx);
        }

        self.disassembly.iter().position(|line| {
            line.external_label_address.is_none()
                && !line.is_collapsed
                && line.bytes.is_empty()
                // Isolate `$0000` preamble banners and blank separators (which carry no splitter or label).
                && (address != Addr::ZERO || line.is_splitter() || line.label.is_some())
                && line.address == address
        })
    }

    /// Resolves all navigable [`OperandJumpTarget`] destinations referenced by `line` left to right.
    ///
    /// Supports [`BlockType::Code`] instructions (`Relative`, `Absolute*`, `Indirect*`, `ZeroPage*`,
    /// and `<`/`>` immediate selectors), plus external equates (`ExternalFile` or Tier 2
    /// [`SymbolicOffset`](super::types::SymbolicOffset)). It also supports `.word`
    /// [`BlockType::Address`] tables and split [`BlockType::LoHiAddress`] /
    /// [`BlockType::HiLoAddress`] tables. Active symbolic offsets redirect targets to `sym.base`.
    ///
    /// Multi-entry lines preserve left-to-right operand order and deduplicate by resolved
    /// [`Addr`], keeping the leftmost expression. Collapsed rows, non-navigable data directives,
    /// odd trailing orphan bytes, and unmapped external equates return an empty `Vec`.
    #[must_use]
    pub fn resolve_disassembly_jump_targets(
        &self,
        line: &DisassemblyLine,
    ) -> Vec<OperandJumpTarget> {
        if line.is_collapsed {
            return Vec::new();
        }

        if line.opcode.is_some() {
            return self.resolve_code_jump_targets(line);
        }

        if line.external_label_address.is_some() {
            return self.resolve_external_equate_jump_targets(line);
        }

        if line.bytes.is_empty() || self.is_external(line.address) {
            return Vec::new();
        }

        let pc = line.address.offset_from(self.origin);
        match self.block_types.get(pc) {
            Some(BlockType::Address) => self.resolve_word_table_jump_targets(line),
            Some(BlockType::LoHiAddress) => self.resolve_split_table_jump_targets(line, true),
            Some(BlockType::HiLoAddress) => self.resolve_split_table_jump_targets(line, false),
            _ => Vec::new(),
        }
    }

    /// Resolves the single jump target (if any) of a [`BlockType::Code`] instruction line.
    fn resolve_code_jump_targets(&self, line: &DisassemblyLine) -> Vec<OperandJumpTarget> {
        let Some(opcode) = &line.opcode else {
            return Vec::new();
        };

        let target = if opcode.mode == AddressingMode::Relative {
            let Some(&offset_byte) = line.bytes.get(1) else {
                return Vec::new();
            };
            let offset = offset_byte as i8;
            line.address.wrapping_add(2).wrapping_add(offset as u16)
        } else {
            let raw_target = match opcode.mode {
                AddressingMode::Immediate => {
                    // Only `<label` / `>label` immediate selectors carry a navigable address.
                    let Some(target) = self
                        .annotations
                        .get(line.address)
                        .and_then(|e| e.immediate_format)
                        .and_then(|fmt| match fmt {
                            ImmediateFormat::LowByte(target)
                            | ImmediateFormat::HighByte(target) => Some(target),
                            _ => None,
                        })
                    else {
                        return Vec::new();
                    };
                    target
                }
                AddressingMode::Absolute
                | AddressingMode::AbsoluteX
                | AddressingMode::AbsoluteY
                | AddressingMode::Indirect => {
                    let (Some(&lo), Some(&hi)) = (line.bytes.get(1), line.bytes.get(2)) else {
                        return Vec::new();
                    };
                    Addr(u16::from_le_bytes([lo, hi]))
                }
                AddressingMode::ZeroPage
                | AddressingMode::ZeroPageX
                | AddressingMode::ZeroPageY
                | AddressingMode::IndirectX
                | AddressingMode::IndirectY => {
                    let Some(&zp) = line.bytes.get(1) else {
                        return Vec::new();
                    };
                    Addr(u16::from(zp))
                }
                _ => return Vec::new(),
            };

            resolve_effective_symbolic_offset_for_mode(
                line.address,
                raw_target,
                opcode.mode.is_zero_page(),
                &self.labels,
                &self.annotations,
            )
            .map_or(raw_target, |sym| sym.base)
        };

        // Keep `line.operand` verbatim without comma-splitting so indexed/indirect operands such
        // as `LDA ($FB), Y` and `LDA $102F, X` stay intact.
        let trimmed_operand = line.operand.trim();
        let operand_expr = if trimmed_operand.is_empty() {
            Disassembler::formatter(self.settings.assembler).format_address(target)
        } else {
            trimmed_operand.to_string()
        };
        let target_preview = self.build_target_preview(target);
        vec![OperandJumpTarget::new(target, operand_expr, target_preview)]
    }

    /// Resolves a jump target from a preamble external equate row (`ExternalFile` or Tier 2 redirect).
    fn resolve_external_equate_jump_targets(
        &self,
        line: &DisassemblyLine,
    ) -> Vec<OperandJumpTarget> {
        let Some(ext_addr) = line.external_label_address else {
            return Vec::new();
        };

        // Check Tier 2 symbolic redirects first (such as `ext_0fff = $0fff` -> `$1000 - 1`);
        // otherwise allow jumping only when `ext_addr` maps inside the loaded binary (`ExternalFile`).
        let resolved =
            resolve_effective_symbolic_offset(ext_addr, ext_addr, &self.labels, &self.annotations)
                .map(|sym| sym.base)
                .or_else(|| (!self.is_external(ext_addr)).then_some(ext_addr));

        let Some(target) = resolved else {
            return Vec::new();
        };

        // External equate rows store `"NAME = $XXXX"` in `line.mnemonic` with an empty `operand`.
        let formatter = Disassembler::formatter(self.settings.assembler);
        let trimmed_operand = line.operand.trim();
        let trimmed_mnemonic = line.mnemonic.trim();
        let operand_expr = if !trimmed_operand.is_empty() {
            trimmed_operand.to_string()
        } else if !trimmed_mnemonic.is_empty() {
            trimmed_mnemonic.to_string()
        } else {
            resolve_symbolic_label_name(
                ext_addr,
                ext_addr,
                &self.labels,
                &self.settings,
                &self.annotations,
                None,
                None,
                None,
                formatter.scope_resolution_separator(),
                formatter.local_label_prefix(),
                OffsetParentheses::Bare,
                |base| formatter.format_address(base),
            )
            .unwrap_or_else(|| formatter.format_address(target))
        };

        let target_preview = self.build_target_preview(target);
        vec![OperandJumpTarget::new(target, operand_expr, target_preview)]
    }

    /// Resolves deduplicated 16-bit targets from a [`BlockType::Address`] `.word` line.
    fn resolve_word_table_jump_targets(&self, line: &DisassemblyLine) -> Vec<OperandJumpTarget> {
        let entry_count = line.bytes.len() / 2;
        if entry_count == 0 {
            return Vec::new();
        }

        // Clamp `end_addr` with `saturating_add` so a `.word` line reaching `$FFFF` never wraps.
        let line_off = line.address.offset_from(self.origin);
        let max_offset = u16::try_from(line.bytes.len().saturating_sub(1)).unwrap_or(u16::MAX);
        let end_addr = Addr(line.address.0.saturating_add(max_offset));

        let mut raw_entries = Vec::with_capacity(entry_count);
        for pair_idx in 0..entry_count {
            let byte_off = pair_idx * 2;
            let Ok(byte_off_u16) = u16::try_from(byte_off) else {
                break;
            };
            let Some(word_addr_u16) = line.address.0.checked_add(byte_off_u16) else {
                break;
            };
            let Some(next_addr_u16) = word_addr_u16.checked_add(1) else {
                break;
            };
            let word_addr = Addr(word_addr_u16);
            let next_addr = Addr(next_addr_u16);
            if next_addr > end_addr || self.is_external(word_addr) || self.is_external(next_addr) {
                break;
            }
            let word_pc = line_off + byte_off;
            if self.block_types.get(word_pc) != Some(&BlockType::Address)
                || self.block_types.get(word_pc + 1) != Some(&BlockType::Address)
            {
                break;
            }
            // Stop before a virtual splitter cuts a 2-byte pointer in half.
            if (pair_idx > 0 && self.is_virtual_splitter(word_addr))
                || self.is_virtual_splitter(next_addr)
            {
                break;
            }
            let (Some(&lo), Some(&hi)) = (line.bytes.get(byte_off), line.bytes.get(byte_off + 1))
            else {
                break;
            };
            raw_entries.push((word_addr, Addr(u16::from_le_bytes([lo, hi]))));
        }

        self.collect_table_jump_targets(
            &line.operand,
            raw_entries,
            |&(site_addr, raw_target)| {
                resolve_effective_symbolic_offset(
                    site_addr,
                    raw_target,
                    &self.labels,
                    &self.annotations,
                )
                .map_or(raw_target, |sym| sym.base)
            },
            |&(site_addr, raw_target), formatter| {
                resolve_symbolic_label_name(
                    site_addr,
                    raw_target,
                    &self.labels,
                    &self.settings,
                    &self.annotations,
                    None,
                    None,
                    None,
                    formatter.scope_resolution_separator(),
                    formatter.local_label_prefix(),
                    OffsetParentheses::Bare,
                    |base| formatter.format_address(base),
                )
                .unwrap_or_else(|| formatter.format_address(raw_target))
            },
        )
    }

    /// Resolves deduplicated 16-bit targets from either half of a split [`BlockType::LoHiAddress`]
    /// (`lo_first = true`) or [`BlockType::HiLoAddress`] (`lo_first = false`) byte table.
    fn resolve_split_table_jump_targets(
        &self,
        line: &DisassemblyLine,
        lo_first: bool,
    ) -> Vec<OperandJumpTarget> {
        let line_off = line.address.offset_from(self.origin);
        let line_end_off = line_off.saturating_add(line.bytes.len());

        let mut raw_entries = Vec::with_capacity(line.bytes.len());
        let mut cached_bounds: Option<(usize, usize, BlockType)> = None;
        for byte_idx in 0..line.bytes.len() {
            let Ok(byte_off_u16) = u16::try_from(byte_idx) else {
                break;
            };
            let byte_addr = line.address.wrapping_add(byte_off_u16);
            if self.is_external(byte_addr) {
                continue;
            }
            let pc = byte_addr.offset_from(self.origin);
            let Some(&target_type) = self.block_types.get(pc) else {
                continue;
            };
            if !matches!(target_type, BlockType::LoHiAddress | BlockType::HiLoAddress) {
                continue;
            }
            // Cache `(run_start, count)` across consecutive bytes in the same split-table sub-block
            // so a packed multi-entry line scans sub-block boundaries once instead of per byte.
            let (run_start, count) = match cached_bounds {
                Some((start, len, cached_type))
                    if cached_type == target_type && pc >= start && pc < start + len =>
                {
                    (start, len)
                }
                _ => {
                    let bounds = self.split_table_subblock_bounds(pc, target_type);
                    cached_bounds = Some((bounds.0, bounds.1, target_type));
                    bounds
                }
            };
            let Some((first_addr, second_addr, raw_target)) =
                self.split_table_pair_in_bounds(pc, target_type, run_start, count)
            else {
                continue;
            };
            // Classify half membership by origin-relative offset span `(line_off..line_end_off)`.
            // Naive `line.address < second_addr` checks fail both on wrapped multi-line tables
            // (`addresses_per_line`, where entry `k > 0` has `second_addr > line.address`) and on
            // binaries that wrap across `$FFFF -> $0000`.
            let is_second_half =
                (line_off..line_end_off).contains(&second_addr.offset_from(self.origin));
            raw_entries.push((first_addr, second_addr, raw_target, is_second_half));
        }

        self.collect_table_jump_targets(
            &line.operand,
            raw_entries,
            |&(first_addr, second_addr, raw_target, _)| {
                resolve_split_pair_effective_symbolic_offset(
                    first_addr,
                    second_addr,
                    raw_target,
                    &self.labels,
                    &self.annotations,
                )
                .map_or(raw_target, |sym| sym.base)
            },
            |&(first_addr, second_addr, raw_target, is_second_half), formatter| {
                // `LoHiAddress` puts `<` on the first half (`is_second_half == false`).
                // `HiLoAddress` reverses that order.
                let prefix = if lo_first != is_second_half { '<' } else { '>' };
                let inner = resolve_split_pair_symbolic_label_name(
                    first_addr,
                    second_addr,
                    raw_target,
                    &self.labels,
                    &self.settings,
                    &self.annotations,
                    None,
                    None,
                    None,
                    formatter.scope_resolution_separator(),
                    formatter.local_label_prefix(),
                    OffsetParentheses::Parenthesized,
                    |base| formatter.format_address(base),
                )
                .unwrap_or_else(|| formatter.format_address(raw_target));
                format!("{prefix}{inner}")
            },
        )
    }

    /// Deduplicates table entries in left-to-right order and attaches target previews.
    #[must_use]
    fn collect_table_jump_targets<T, FTarget, FExpr>(
        &self,
        operand: &str,
        raw_entries: Vec<T>,
        mut resolve_target: FTarget,
        mut format_fallback_expr: FExpr,
    ) -> Vec<OperandJumpTarget>
    where
        FTarget: FnMut(&T) -> Addr,
        FExpr: FnMut(&T, &dyn Formatter) -> String,
    {
        if raw_entries.is_empty() {
            return Vec::new();
        }

        let formatter = Disassembler::formatter(self.settings.assembler);
        let expr_parts = Self::split_operand_expressions(operand, raw_entries.len());
        let mut seen_targets = BTreeSet::new();
        let mut pending = Vec::with_capacity(raw_entries.len());

        for (idx, entry) in raw_entries.into_iter().enumerate() {
            let target = resolve_target(&entry);
            if !seen_targets.insert(target) {
                continue;
            }

            let operand_expr =
                if let Some(part) = expr_parts.as_ref().and_then(|parts| parts.get(idx)) {
                    (*part).to_string()
                } else {
                    format_fallback_expr(&entry, formatter)
                };

            pending.push((target, operand_expr));
        }

        let preview_index = (pending.len() >= 2).then(|| JumpTargetIndex::new(self));
        pending
            .into_iter()
            .map(|(target, operand_expr)| {
                let target_preview =
                    self.build_target_preview_with_index(target, preview_index.as_ref());
                OperandJumpTarget::new(target, operand_expr, target_preview)
            })
            .collect()
    }

    /// Splits a comma-separated data operand string when it yields exact `expected_count` non-empty terms.
    #[must_use]
    fn split_operand_expressions(operand: &str, expected_count: usize) -> Option<Vec<&str>> {
        if expected_count == 0 || operand.trim().is_empty() {
            return None;
        }
        // Require exact arity on `", "` so malformed or hand-crafted strings fall back to
        // per-entry symbol lookup instead of misaligning expressions across entries.
        let parts: Vec<&str> = operand.split(", ").map(str::trim).collect();
        if parts.len() == expected_count && parts.iter().all(|part| !part.is_empty()) {
            Some(parts)
        } else {
            None
        }
    }

    /// Builds a single-line preview of the instruction, data line, equate, or `(out of range)`
    /// label at `target_address` for target disambiguation pickers.
    #[must_use]
    fn build_target_preview(&self, target_address: Addr) -> Option<String> {
        self.build_target_preview_with_index(target_address, None)
    }

    /// Builds a single-line preview at `target_address`, using `preview_index` when supplied so
    /// multi-entry tables resolve previews in `O(log L)` per unique target.
    #[must_use]
    fn build_target_preview_with_index(
        &self,
        target_address: Addr,
        preview_index: Option<&JumpTargetIndex>,
    ) -> Option<String> {
        // Use the same preamble-safe resolver as cursor navigation so previews always match where
        // jumping lands. Once a content line resolves, an empty unlabeled row returns `None` rather
        // than `(out of range)`.
        let resolved_idx = match preview_index {
            Some(idx) => idx.resolve_content_line_index(self, target_address),
            None => navigation::resolve_jump_content_line_index(self, target_address),
        };
        if let Some(line_idx) = resolved_idx
            && let Some(target_line) = self.disassembly.get(line_idx)
        {
            let mnemonic = target_line.mnemonic.trim();
            let operand = target_line.operand.trim();
            if target_line.external_label_address.is_some() {
                return (!mnemonic.is_empty()).then(|| mnemonic.to_string());
            }
            if !mnemonic.is_empty() {
                return Some(if operand.is_empty() {
                    mnemonic.to_string()
                } else {
                    format!("{mnemonic} {operand}")
                });
            }
            return target_line
                .label
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);
        }

        // Query `effective_labels_at` so unmerged system/KERNAL labels (such as `$FFD2`) surface
        // even when `settings.all_labels` is `false`.
        let formatter = Disassembler::formatter(self.settings.assembler);
        let labels = self.effective_labels_at(target_address);
        if let Some(label) = resolve_label(labels, target_address.into(), &self.settings) {
            let name = apply_local_prefix(
                &label.name,
                label.label_type,
                formatter.local_label_prefix(),
            );
            Some(format!("{name} (out of range)"))
        } else {
            Some("(out of range)".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::app_state::AppState;
    use super::super::project::Label;
    use super::super::types::{
        Addr, Assembler, BlockType, EnumDefinition, ImmediateFormat, LabelKind, LabelType,
        OffsetRadix, OperandJumpTarget, SymbolicOffset,
    };
    use crate::cpu;
    use crate::disassembler::DisassemblyLine;

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

        // line_index_for_address(0x0801) should land on the first *content* line
        // (after external label definitions), not on a label-only line
        let idx = app_state
            .line_index_for_address(Addr(0x0801))
            .expect("Should find a line at the origin address $0801");

        let line = &app_state.disassembly[idx];
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
        let idx = app_state
            .line_index_containing_address(Addr(0xC004))
            .expect("line containing $C004 should exist");
        let line = &app_state.disassembly[idx];
        assert_eq!(line.address, Addr(0xC003));
    }

    /// Regression test: when `all_labels = true` and the third pass of
    /// `line_index_for_address` ("first line with address >= target") is
    /// triggered, it must prefer *content* lines over external-label definition
    /// lines that are prepended at the start of `disassembly` and may have
    /// addresses larger than the target.
    ///
    /// This was the root cause of the hex-dump right-arrow causing the
    /// disassembly cursor to jump to the "external labels" section.
    #[test]
    fn test_get_line_index_prefers_content_over_external_label_on_third_pass() {
        use super::super::project::Label;

        // Two code bytes at $3A20 (comes AFTER the queried address $3A10).
        // With `all_labels = true` and an external label at $9000, the external
        // label line is prepended before the main disassembly body. The old
        // "first address >= target" pass would return the $9000 label line
        // (which appears before $3A20 in the array); the new pass must skip it
        // and return the $3A20 content line instead.
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x3A20);
        app_state.raw_data = vec![0xEA, 0x60]; // NOP ; RTS
        app_state.block_types = vec![BlockType::Code; 2];

        // External label at $9000 — higher than origin $3A20, so it gets
        // prepended by `all_labels = true` before $3A20 in the disassembly array.
        app_state
            .labels
            .entry(Addr(0x9000))
            .or_default()
            .push(Label {
                name: "ext_high".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ExternalJump,
            });
        app_state.settings.all_labels = true;
        app_state.disassemble();

        // Ask for $3A10 — below origin $3A20, so passes 1 and 2 find nothing.
        // The third pass must return the first *content* line ($3A20 NOP),
        // not the external label definition at $9000.
        let idx = app_state
            .line_index_for_address(Addr(0x3A10))
            .expect("Should find a line for $3A10");
        let line = &app_state.disassembly[idx];
        assert!(
            !line.bytes.is_empty(),
            "Returned line must have content bytes, not be a label-only external definition \
             (got address ${:04X})",
            line.address.0
        );
        assert_eq!(
            line.address,
            Addr(0x3A20),
            "Returned line should be the first real content line at $3A20"
        );
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
        let beq = app_state
            .cached_arrows
            .iter()
            .find(|a| a.target_addr == Some(Addr(0x1340)))
            .expect("BEQ $1340 arrow should be present in cached_arrows");
        assert!(
            beq.start < beq.end,
            "BEQ forward branch should have start < end"
        );

        // Find the JMP arrow ($133D -> $1331)
        let jmp = app_state
            .cached_arrows
            .iter()
            .find(|a| a.target_addr == Some(Addr(0x1331)))
            .expect("JMP $1331 arrow should be present in cached_arrows");
        assert!(jmp.start > jmp.end, "JMP backward should have start > end");
    }

    #[test]
    fn test_compute_cached_arrows_ignores_external_label_equates() {
        use super::super::project::Label;

        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![
            0x20, 0xD2, 0xFF, // JSR $FFD2
            0x60, // RTS
        ];
        app_state.block_types = vec![BlockType::Code; app_state.raw_data.len()];
        app_state
            .labels
            .entry(Addr(0xFFD2))
            .or_default()
            .push(Label {
                name: "CHROUT".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ExternalJump,
            });
        app_state.settings.all_labels = true;

        app_state.disassemble();

        assert!(
            app_state.cached_arrows.is_empty(),
            "JSR to external equate $FFD2 must not produce a cached arrow"
        );
    }

    #[test]
    fn test_compute_cached_arrows_targets_collapsed_block_interior() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0xC000);
        // $C000: BEQ $C005 (F0 03), followed by 6 NOPs at $C002..=$C007
        app_state.raw_data = vec![0xF0, 0x03, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA];
        app_state.block_types = vec![BlockType::Code; 8];
        // Collapse offsets 2..=6 ($C002..=$C006), so $C005 is inside the collapsed block
        app_state.collapsed_blocks.push((2, 6));

        app_state.disassemble();

        assert_eq!(app_state.cached_arrows.len(), 1);
        let arrow = &app_state.cached_arrows[0];
        assert_eq!(arrow.target_addr, Some(Addr(0xC005)));
        assert_eq!(app_state.disassembly[arrow.end].address, Addr(0xC002));
        assert!(app_state.disassembly[arrow.end].is_collapsed);
    }

    #[test]
    fn test_get_line_index_containing_address_isolates_zero_enum_preamble_headers() {
        use crate::disassembler::DisassemblyLine;

        // 1. Out-of-range `$0000`: enum preamble headers at `$0000` must never match unless an
        // external equate exists for `$0000`.
        let mut state = AppState::new();
        state.origin = Addr(0x0801);
        state.raw_data = vec![0x60]; // RTS at $0801
        state.block_types = vec![BlockType::Code];
        let header = || DisassemblyLine {
            address: Addr::ZERO,
            bytes: Vec::new(),
            mnemonic: "; enum State".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        state.disassembly = vec![
            header(),
            DisassemblyLine {
                address: Addr(0x0801),
                bytes: vec![0x60],
                mnemonic: "rts".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
        ];
        assert_eq!(state.line_index_containing_address(Addr::ZERO), None);

        // Adding a `$0000` external equate after the enum preamble must resolve to that equate row.
        state.disassembly.insert(
            1,
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "zp_zero = $00".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: Some(Addr::ZERO),
                is_collapsed: false,
            },
        );
        assert_eq!(state.line_index_containing_address(Addr::ZERO), Some(1));

        // 2. Origin `$0000` with enum preamble before a `$0000` collapsed block and before an
        // uncollapsed `$0000` body row must hit the collapsed summary row / byte-bearing row, not index 0.
        let mut collapsed_state = AppState::new();
        collapsed_state.origin = Addr::ZERO;
        collapsed_state.raw_data = vec![0xEA, 0xEA];
        collapsed_state.block_types = vec![BlockType::Code, BlockType::Code];
        collapsed_state.collapsed_blocks = vec![(0, 1)];
        collapsed_state.disassembly = vec![
            header(),
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: Vec::new(),
                mnemonic: "; Collapsed Code block from $0000-$0001".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: true,
            },
        ];
        assert_eq!(
            collapsed_state.line_index_containing_address(Addr::ZERO),
            Some(1)
        );
        assert_eq!(
            collapsed_state.line_index_containing_address(Addr(0x0001)),
            Some(1)
        );

        let mut body_state = AppState::new();
        body_state.origin = Addr::ZERO;
        body_state.raw_data = vec![0xEA, 0x60];
        body_state.block_types = vec![BlockType::Code, BlockType::Code];
        body_state.disassembly = vec![
            header(),
            DisassemblyLine {
                address: Addr::ZERO,
                bytes: vec![0xEA],
                mnemonic: "nop".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
        ];
        assert_eq!(
            body_state.line_index_containing_address(Addr::ZERO),
            Some(1)
        );
    }

    #[test]
    fn core_dis_01_code_absolute_indirect_indexed_preserves_operand() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: LDA $1234,X (BD 34 12)
        // $1003: JMP ($1234) (6C 34 12)
        // $1006: JSR $1234   (20 34 12)
        // $1009: RTS         (60)
        state.raw_data = vec![0xBD, 0x34, 0x12, 0x6C, 0x34, 0x12, 0x20, 0x34, 0x12, 0x60];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.disassemble();

        let lda_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && l.opcode.is_some())
            .expect("LDA $1234,X line");
        assert_eq!(
            state.resolve_disassembly_jump_targets(lda_line),
            vec![OperandJumpTarget::new(
                Addr(0x1234),
                lda_line.operand.trim(),
                Some("(out of range)".to_string()),
            )]
        );
        assert!(lda_line.operand.contains(','));

        let jmp_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1003) && l.opcode.is_some())
            .expect("JMP ($1234) line");
        assert_eq!(
            state.resolve_disassembly_jump_targets(jmp_line),
            vec![OperandJumpTarget::new(
                Addr(0x1234),
                jmp_line.operand.trim(),
                Some("(out of range)".to_string()),
            )]
        );

        let jsr_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1006) && l.opcode.is_some())
            .expect("JSR $1234 line");
        assert_eq!(
            state.resolve_disassembly_jump_targets(jsr_line),
            vec![OperandJumpTarget::new(
                Addr(0x1234),
                jsr_line.operand.trim(),
                Some("(out of range)".to_string()),
            )]
        );
    }

    #[test]
    fn core_dis_02_code_relative_branch_resolves_target_and_preview() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: BNE $1004 (D0 02), $1002: NOP (EA), $1003: NOP (EA), $1004: RTS (60), $1005: BNE $1000 (D0 F9)
        state.raw_data = vec![0xD0, 0x02, 0xEA, 0xEA, 0x60, 0xD0, 0xF9];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.disassemble();

        let fwd_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && l.opcode.is_some())
            .expect("forward BNE");
        assert_eq!(
            state.resolve_disassembly_jump_targets(fwd_line),
            vec![OperandJumpTarget::new(
                Addr(0x1004),
                fwd_line.operand.trim(),
                Some("rts".to_string()),
            )]
        );

        let back_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1005) && l.opcode.is_some())
            .expect("backward BNE");
        assert_eq!(
            state.resolve_disassembly_jump_targets(back_line),
            vec![OperandJumpTarget::new(
                Addr(0x1000),
                back_line.operand.trim(),
                Some(format!(
                    "{} {}",
                    fwd_line.mnemonic.trim(),
                    fwd_line.operand.trim()
                )),
            )]
        );
    }

    #[test]
    fn core_dis_03_code_immediate_formatted_and_plain() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: LDA #$FF (LowByte of $10FF, Tier 1 sym offset base = $1100 - 1)
        // $1002: LDX #$11 (HighByte of $1100)
        // $1004: LDY #$05 (plain hex immediate -> no jump target)
        state.raw_data = vec![0xA9, 0xFF, 0xA2, 0x11, 0xA0, 0x05];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.annotations.update(Addr(0x1000), |e| {
            e.immediate_format = Some(ImmediateFormat::LowByte(Addr(0x10FF)));
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x1100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
        state.annotations.update(Addr(0x1002), |e| {
            e.immediate_format = Some(ImmediateFormat::HighByte(Addr(0x1100)));
        });
        state.labels.insert(
            Addr(0x1100),
            vec![Label {
                name: "sub_1100".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.disassemble();

        let lda_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && l.opcode.is_some())
            .expect("LDA #<(sub_1100 - 1)");
        assert_eq!(
            state.resolve_disassembly_jump_targets(lda_line),
            vec![OperandJumpTarget::new(
                Addr(0x1100),
                lda_line.operand.trim(),
                Some("sub_1100 (out of range)".to_string()),
            )]
        );

        let ldx_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1002) && l.opcode.is_some())
            .expect("LDX #>sub_1100");
        assert_eq!(
            state.resolve_disassembly_jump_targets(ldx_line),
            vec![OperandJumpTarget::new(
                Addr(0x1100),
                ldx_line.operand.trim(),
                Some("sub_1100 (out of range)".to_string()),
            )]
        );

        let ldy_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1004) && l.opcode.is_some())
            .expect("plain LDY #$05");
        assert!(state.resolve_disassembly_jump_targets(ldy_line).is_empty());
    }

    #[test]
    fn core_dis_04_code_zero_page_symbolic_offset_respects_zp_bound() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: LDA $04,X (B5 04) -> Tier 1 base $0002 + 2 (valid ZP <= $00FF)
        // $1002: STA $06   (85 06) -> Tier 1 base $0200 (invalid ZP > $00FF -> ignored, stays $0006)
        state.raw_data = vec![0xB5, 0x04, 0x85, 0x06];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.annotations.update(Addr(0x1000), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0002),
                offset: 2,
                radix: OffsetRadix::Decimal,
            });
        });
        state.annotations.update(Addr(0x1002), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0200),
                offset: -0x01FA,
                radix: OffsetRadix::Decimal,
            });
        });
        state.labels.insert(
            Addr(0x0002),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ZeroPagePointer,
            }],
        );
        state.disassemble();

        let lda_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && l.opcode.is_some())
            .expect("LDA zp_ptr + 2,x");
        assert_eq!(
            state.resolve_disassembly_jump_targets(lda_line),
            vec![OperandJumpTarget::new(
                Addr(0x0002),
                lda_line.operand.trim(),
                Some("zp_ptr (out of range)".to_string()),
            )]
        );

        let sta_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1002) && l.opcode.is_some())
            .expect("STA $06");
        assert_eq!(
            state.resolve_disassembly_jump_targets(sta_line),
            vec![OperandJumpTarget::new(
                Addr(0x0006),
                sta_line.operand.trim(),
                Some("(out of range)".to_string()),
            )]
        );

        // When `all_labels` emits the external equate `zp_ptr = $02`, preview resolves to the equate row.
        state.settings.all_labels = true;
        state.disassemble();
        let lda_line_with_eq = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && l.opcode.is_some())
            .expect("LDA zp_ptr + 2,x with all_labels");
        assert_eq!(
            state.resolve_disassembly_jump_targets(lda_line_with_eq),
            vec![OperandJumpTarget::new(
                Addr(0x0002),
                lda_line_with_eq.operand.trim(),
                Some("zp_ptr = $02".to_string()),
            )]
        );
    }

    #[test]
    fn core_dis_05_out_of_range_target_preview_uses_system_labels_unless_excluded() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: JSR $FFD2 (20 D2 FF), $1003: JSR $DEAD (20 AD DE)
        state.raw_data = vec![0x20, 0xD2, 0xFF, 0x20, 0xAD, 0xDE];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.settings.all_labels = false;
        state.system_labels.insert(
            Addr(0xFFD2),
            vec![Label {
                name: "KERNAL_CHROUT".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Predefined,
            }],
        );
        state.disassemble();

        let chrout_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && l.opcode.is_some())
            .expect("JSR $FFD2");
        assert_eq!(
            state.resolve_disassembly_jump_targets(chrout_line),
            vec![OperandJumpTarget::new(
                Addr(0xFFD2),
                chrout_line.operand.trim(),
                Some("KERNAL_CHROUT (out of range)".to_string()),
            )]
        );

        let unknown_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1003) && l.opcode.is_some())
            .expect("JSR $DEAD");
        assert_eq!(
            state.resolve_disassembly_jump_targets(unknown_line),
            vec![OperandJumpTarget::new(
                Addr(0xDEAD),
                unknown_line.operand.trim(),
                Some("(out of range)".to_string()),
            )]
        );

        // Excluded `$FFD2` omits the system label and renders bare `(out of range)`.
        state.excluded_addresses.insert(Addr(0xFFD2));
        state.disassemble();
        let chrout_excluded = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && l.opcode.is_some())
            .expect("JSR $FFD2 excluded");
        assert_eq!(
            state.resolve_disassembly_jump_targets(chrout_excluded),
            vec![OperandJumpTarget::new(
                Addr(0xFFD2),
                chrout_excluded.operand.trim(),
                Some("(out of range)".to_string()),
            )]
        );
    }

    #[test]
    fn core_dis_06_address_word_table_multi_targets_and_symbolic_offset() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..=$1003: Address table [$1004, $1006] where $1006 redirects via Tier 1 to $1004 + 2?
        // Let entry 1 redirect to $1007 - 1 (base $1007, actual raw word $1006 -> `$1007` is $1006+1: nop; rts).
        // $1004: RTS (60)
        // $1005: NOP (EA)
        // $1006: NOP (EA)
        // $1007: LDA #$01 (A9 01)
        state.raw_data = vec![0x04, 0x10, 0x06, 0x10, 0x60, 0xEA, 0xEA, 0xA9, 0x01];
        state.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
        ];
        state.annotations.update(Addr(0x1002), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x1007),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
        state.labels.insert(
            Addr(0x1004),
            vec![Label {
                name: "handler_a".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.labels.insert(
            Addr(0x1007),
            vec![Label {
                name: "handler_b".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.disassemble();

        let word_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect(".word table line");
        assert_eq!(
            state.resolve_disassembly_jump_targets(word_line),
            vec![
                OperandJumpTarget::new(Addr(0x1004), "handler_a", Some("rts".to_string()),),
                OperandJumpTarget::new(Addr(0x1007), "handler_b - 1", Some("lda #$01".to_string()),),
            ]
        );
    }

    #[test]
    fn core_dis_07_address_word_table_deduplicates_preserving_leftmost_expr() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..=$1005: Address table with raw words [$1008, $1009, $100A]:
        // - entry 0 ($1000): raw $1008 -> base target_a (`target_a`)
        // - entry 1 ($1002): raw $1009 -> Tier 1 base target_a (`target_a + 1`) -> deduped against entry 0!
        // - entry 2 ($1004): raw $100A -> `target_b`
        // $1006..=$1007: RTS, NOP padding; $1008: RTS; $1009: NOP; $100A: NOP
        state.raw_data = vec![
            0x08, 0x10, 0x09, 0x10, 0x0A, 0x10, 0x60, 0xEA, 0x60, 0xEA, 0xEA,
        ];
        state.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
        ];
        state.annotations.update(Addr(0x1002), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x1008),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        state.labels.insert(
            Addr(0x1008),
            vec![Label {
                name: "target_a".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.labels.insert(
            Addr(0x100A),
            vec![Label {
                name: "target_b".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.disassemble();

        let word_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect(".word table line");
        assert_eq!(
            state.resolve_disassembly_jump_targets(word_line),
            vec![
                OperandJumpTarget::new(Addr(0x1008), "target_a", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x100A), "target_b", Some("nop".to_string())),
            ]
        );
    }

    #[test]
    fn core_dis_08_address_word_table_breaks_on_mid_word_virtual_splitter() {
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // 4-byte Address region at $1000..=$1003, plus RTS at $1004
        state.raw_data = vec![0x04, 0x10, 0x05, 0x10, 0x60, 0xEA];
        state.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Code,
            BlockType::Code,
        ];
        // Put virtual splitter in the middle of word 1 ($1003) -> word 0 intact, word 1 excluded.
        state.splitters.insert(Addr(0x1003));
        state.disassemble();

        let synthetic_full_line = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0x04, 0x10, 0x05, 0x10],
            mnemonic: ".word".to_string(),
            operand: "$1004".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        assert_eq!(
            state.resolve_disassembly_jump_targets(&synthetic_full_line),
            vec![OperandJumpTarget::new(
                Addr(0x1004),
                "$1004",
                Some("rts".to_string()),
            )]
        );

        // Mid-word splitter inside word 0 ($1001) yields zero targets.
        state.splitters.insert(Addr(0x1001));
        assert!(
            state
                .resolve_disassembly_jump_targets(&synthetic_full_line)
                .is_empty()
        );
    }

    #[test]
    fn core_dis_09_address_word_table_odd_trailing_and_ffff_clamp() {
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0xFFFD);
        // 3 bytes: $FFFD, $FFFE, $FFFF
        state.raw_data = vec![0x00, 0x10, 0xAA];
        state.block_types = vec![BlockType::Address; 3];
        state.disassemble();

        let lines: Vec<_> = state
            .disassembly
            .iter()
            .filter(|l| !l.bytes.is_empty())
            .collect();
        // $FFFD..=$FFFE forms 1 word ($1000), $FFFF is odd trailing byte (`.byte $aa`).
        assert_eq!(lines.len(), 2);
        assert_eq!(
            state.resolve_disassembly_jump_targets(lines[0]),
            vec![OperandJumpTarget::new(
                Addr(0x1000),
                "$1000",
                Some("(out of range)".to_string()),
            )]
        );
        assert!(state.resolve_disassembly_jump_targets(lines[1]).is_empty());

        // Synthetic 3-byte row ending at $FFFF extracts only word 0 and clamps cleanly at $FFFF.
        let line_at_fffe = DisassemblyLine {
            address: Addr(0xFFFE),
            bytes: vec![0x00, 0x10],
            mnemonic: ".word".to_string(),
            operand: "$1000".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let mut fffe_state = AppState::new();
        fffe_state.origin = Addr(0xFFFE);
        fffe_state.raw_data = vec![0x00, 0x10];
        fffe_state.block_types = vec![BlockType::Address; 2];
        assert_eq!(
            fffe_state.resolve_disassembly_jump_targets(&line_at_fffe),
            vec![OperandJumpTarget::new(
                Addr(0x1000),
                "$1000",
                Some("(out of range)".to_string()),
            )]
        );

        // Single byte at $FFFF must never overflow or wrap to $0000.
        let mut ffff_state = AppState::new();
        ffff_state.origin = Addr(0xFFFF);
        ffff_state.raw_data = vec![0x00, 0x10];
        ffff_state.block_types = vec![BlockType::Address; 2];
        let line_at_ffff = DisassemblyLine {
            address: Addr(0xFFFF),
            bytes: vec![0x00, 0x10],
            mnemonic: ".word".to_string(),
            operand: "$1000".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        assert!(
            ffff_state
                .resolve_disassembly_jump_targets(&line_at_ffff)
                .is_empty()
        );
    }

    #[test]
    fn core_dis_10_expression_arity_mismatch_falls_back_deterministically() {
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // 4 bytes Address [$1008, $100A], 4 bytes LoHiAddress [lo($1008, $100A), hi($1008, $100A)],
        // 2 bytes Code: RTS ($1008), NOP ($1009), RTS ($100A)
        state.raw_data = vec![
            0x08, 0x10, 0x0A, 0x10, 0x08, 0x0A, 0x10, 0x10, 0x60, 0xEA, 0x60,
        ];
        state.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
        ];
        state.annotations.update(Addr(0x1005), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x100B),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
        state.labels.insert(
            Addr(0x1008),
            vec![Label {
                name: "sub_a".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.disassemble();

        // Address line with mismatched single operand on a 2-word line -> deterministic fallback.
        let word_mismatch = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0x08, 0x10, 0x0A, 0x10],
            mnemonic: ".word".to_string(),
            operand: "stale_expr_only".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        assert_eq!(
            state.resolve_disassembly_jump_targets(&word_mismatch),
            vec![
                OperandJumpTarget::new(Addr(0x1008), "sub_a", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x100A), "$100a", Some("rts".to_string())),
            ]
        );

        // Split-table lines with empty/mismatched operand -> parenthesized `<` / `>` fallback.
        let split_lo_mismatch = DisassemblyLine {
            address: Addr(0x1004),
            bytes: vec![0x08, 0x0A],
            mnemonic: ".byte".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let split_hi_mismatch = DisassemblyLine {
            address: Addr(0x1006),
            bytes: vec![0x10, 0x10],
            mnemonic: ".byte".to_string(),
            operand: "only_one".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        assert_eq!(
            state.resolve_disassembly_jump_targets(&split_lo_mismatch),
            vec![
                OperandJumpTarget::new(Addr(0x1008), "<sub_a", Some("rts".to_string())),
                OperandJumpTarget::new(
                    Addr(0x100B),
                    "<($100b - 1)",
                    Some("(out of range)".to_string())
                ),
            ]
        );
        assert_eq!(
            state.resolve_disassembly_jump_targets(&split_hi_mismatch),
            vec![
                OperandJumpTarget::new(Addr(0x1008), ">sub_a", Some("rts".to_string())),
                OperandJumpTarget::new(
                    Addr(0x100B),
                    ">($100b - 1)",
                    Some("(out of range)".to_string())
                ),
            ]
        );
    }

    #[test]
    fn core_dis_11_lohi_address_both_halves_resolve_and_deduplicate() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // LoHiAddress 6 bytes (3 pairs): targets [$1006, $1007, $1006]
        // Lo: [06, 07, 06], Hi: [10, 10, 10]
        // $1006: RTS (60), $1007: NOP (EA)
        state.raw_data = vec![0x06, 0x07, 0x06, 0x10, 0x10, 0x10, 0x60, 0xEA];
        state.block_types = vec![
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::Code,
            BlockType::Code,
        ];
        state.labels.insert(
            Addr(0x1006),
            vec![Label {
                name: "fn_one".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.labels.insert(
            Addr(0x1007),
            vec![Label {
                name: "fn_two".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.disassemble();

        let lo_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect("LoHiAddress lo half");
        let hi_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1003) && !l.bytes.is_empty())
            .expect("LoHiAddress hi half");

        assert_eq!(
            state.resolve_disassembly_jump_targets(lo_line),
            vec![
                OperandJumpTarget::new(Addr(0x1006), "<fn_one", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x1007), "<fn_two", Some("nop".to_string())),
            ]
        );
        assert_eq!(
            state.resolve_disassembly_jump_targets(hi_line),
            vec![
                OperandJumpTarget::new(Addr(0x1006), ">fn_one", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x1007), ">fn_two", Some("nop".to_string())),
            ]
        );
    }

    #[test]
    fn core_dis_12_hilo_address_both_halves_resolve_targets() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // HiLoAddress 4 bytes (2 pairs): targets [$1004, $1005]
        // Hi: [10, 10], Lo: [04, 05]; $1004: RTS (60), $1005: NOP (EA)
        state.raw_data = vec![0x10, 0x10, 0x04, 0x05, 0x60, 0xEA];
        state.block_types = vec![
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
            BlockType::Code,
            BlockType::Code,
        ];
        state.disassemble();

        let hi_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect("HiLoAddress hi half");
        let lo_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .expect("HiLoAddress lo half");

        assert_eq!(
            state.resolve_disassembly_jump_targets(hi_line),
            vec![
                OperandJumpTarget::new(Addr(0x1004), ">$1004", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x1005), ">$1005", Some("nop".to_string())),
            ]
        );
        assert_eq!(
            state.resolve_disassembly_jump_targets(lo_line),
            vec![
                OperandJumpTarget::new(Addr(0x1004), "<$1004", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x1005), "<$1005", Some("nop".to_string())),
            ]
        );
    }

    #[test]
    fn core_dis_13_wrapped_split_table_across_ffff_identifies_halves() {
        use crate::disassembler::DisassemblyLine;

        // LoHiAddress spanning $FFFE, $FFFF (lo) -> $0000, $0001 (hi), followed by RTS at $0002.
        let mut state = AppState::new();
        state.origin = Addr(0xFFFE);
        state.raw_data = vec![0x02, 0x34, 0x00, 0x12, 0x60];
        state.block_types = vec![
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::Code,
        ];
        state.disassemble();

        let first_half = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0xFFFE) && !l.bytes.is_empty())
            .expect("wrapped LoHi first half at $FFFE");
        let second_half = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr::ZERO && !l.bytes.is_empty())
            .expect("wrapped LoHi second half at $0000");

        assert_eq!(
            state.resolve_disassembly_jump_targets(first_half),
            vec![
                OperandJumpTarget::new(Addr(0x0002), "<$0002", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x1234), "<$1234", Some("(out of range)".to_string())),
            ]
        );
        assert_eq!(
            state.resolve_disassembly_jump_targets(second_half),
            vec![
                OperandJumpTarget::new(Addr(0x0002), ">$0002", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x1234), ">$1234", Some("(out of range)".to_string())),
            ]
        );

        // Verify fallback `<`/`>` prefix generation also handles wrap when operand is mismatched.
        let mut first_mismatch = first_half.clone();
        first_mismatch.operand = String::new();
        let mut second_mismatch = second_half.clone();
        second_mismatch.operand = String::new();
        assert_eq!(
            state.resolve_disassembly_jump_targets(&first_mismatch),
            vec![
                OperandJumpTarget::new(Addr(0x0002), "<$0002", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x1234), "<$1234", Some("(out of range)".to_string())),
            ]
        );
        assert_eq!(
            state.resolve_disassembly_jump_targets(&second_mismatch),
            vec![
                OperandJumpTarget::new(Addr(0x0002), ">$0002", Some("rts".to_string())),
                OperandJumpTarget::new(Addr(0x1234), ">$1234", Some("(out of range)".to_string())),
            ]
        );

        // Also check HiLoAddress wrap with swapped halves (`>` at $FFFE, `<` at $0000).
        let mut hilo_state = AppState::new();
        hilo_state.origin = Addr(0xFFFE);
        hilo_state.raw_data = vec![0x00, 0x12, 0x02, 0x34, 0x60];
        hilo_state.block_types = vec![
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
            BlockType::Code,
        ];
        let hilo_first = DisassemblyLine {
            address: Addr(0xFFFE),
            bytes: vec![0x00, 0x12],
            mnemonic: ".byte".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let hilo_second = DisassemblyLine {
            address: Addr::ZERO,
            bytes: vec![0x02, 0x34],
            mnemonic: ".byte".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        assert_eq!(
            hilo_state.resolve_disassembly_jump_targets(&hilo_first),
            vec![
                OperandJumpTarget::new(Addr(0x0002), ">$0002", Some("(out of range)".to_string())),
                OperandJumpTarget::new(Addr(0x1234), ">$1234", Some("(out of range)".to_string())),
            ]
        );
        assert_eq!(
            hilo_state.resolve_disassembly_jump_targets(&hilo_second),
            vec![
                OperandJumpTarget::new(Addr(0x0002), "<$0002", Some("(out of range)".to_string())),
                OperandJumpTarget::new(Addr(0x1234), "<$1234", Some("(out of range)".to_string())),
            ]
        );
    }

    #[test]
    fn core_dis_14_collapsed_line_returns_empty() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0x20, 0x34, 0x12, 0x04, 0x10, 0x60];
        state.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Address,
            BlockType::Address,
            BlockType::Code,
        ];
        state.collapsed_blocks = vec![(0, 4)];
        state.disassemble();

        let collapsed_line = state
            .disassembly
            .iter()
            .find(|l| l.is_collapsed)
            .expect("collapsed summary line");
        assert!(
            state
                .resolve_disassembly_jump_targets(collapsed_line)
                .is_empty()
        );

        // Even a synthetic line carrying `opcode` + `bytes` returns empty when `is_collapsed == true`.
        let mut collapsed_with_opcode = collapsed_line.clone();
        collapsed_with_opcode.bytes = vec![0x20, 0x34, 0x12];
        collapsed_with_opcode.opcode = cpu::opcodes()[0x20].clone();
        assert!(
            state
                .resolve_disassembly_jump_targets(&collapsed_with_opcode)
                .is_empty()
        );
    }

    #[test]
    fn core_dis_15_non_navigable_rows_return_empty_and_external_file_equate_resolves() {
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.all_labels = true;
        // Build blocks of every non-navigable kind plus JSR $FFD2 for a plain external equate.
        state.raw_data = vec![
            0x20, 0xD2, 0xFF, // $1000..=$1002: Code JSR $FFD2
            0x01, 0x02, // $1003..=$1004: DataByte
            0x34, 0x12, // $1005..=$1006: DataWord
            0x41, 0x42, // $1007..=$1008: PetsciiText
            0x01, 0x02, // $1009..=$100A: ScreencodeText
            0x00, 0x10, // $100B..=$100C: LoHiWord
            0x10, 0x00, // $100D..=$100E: HiLoWord
            0xAA, 0xBB, // $100F..=$1010: ExternalFile
            0xCC, // $1011: Undefined
        ];
        state.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::DataByte,
            BlockType::DataByte,
            BlockType::DataWord,
            BlockType::DataWord,
            BlockType::PetsciiText,
            BlockType::PetsciiText,
            BlockType::ScreencodeText,
            BlockType::ScreencodeText,
            BlockType::LoHiWord,
            BlockType::LoHiWord,
            BlockType::HiLoWord,
            BlockType::HiLoWord,
            BlockType::ExternalFile,
            BlockType::ExternalFile,
            BlockType::Undefined,
        ];
        state.labels.insert(
            Addr(0xFFD2),
            vec![Label {
                name: "CHROUT".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Predefined,
            }],
        );
        state.labels.insert(
            Addr(0x100F),
            vec![Label {
                name: "music_init".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.disassemble();

        // Header/spacer lines
        let header_line = DisassemblyLine {
            address: Addr::ZERO,
            bytes: Vec::new(),
            mnemonic: "; header".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        assert!(
            state
                .resolve_disassembly_jump_targets(&header_line)
                .is_empty()
        );

        // Plain external equate `CHROUT = $ffd2` returns empty.
        let chrout_equate = state
            .disassembly
            .iter()
            .find(|l| l.external_label_address == Some(Addr(0xFFD2)))
            .expect("CHROUT external equate");
        assert!(
            state
                .resolve_disassembly_jump_targets(chrout_equate)
                .is_empty()
        );

        // ExternalFile equate `music_init = $100f` navigates to `$100F` body row and preserves
        // the equate `mnemonic` string as `operand_expr` (Design Doc Stage 2b).
        let ext_file_equate = state
            .disassembly
            .iter()
            .find(|l| l.external_label_address == Some(Addr(0x100F)))
            .expect("ExternalFile equate");
        let ext_targets = state.resolve_disassembly_jump_targets(ext_file_equate);
        assert_eq!(ext_targets.len(), 1);
        assert_eq!(ext_targets[0].target_address, Addr(0x100F));
        assert_eq!(ext_targets[0].operand_expr, ext_file_equate.mnemonic.trim());

        // Tier 2 redirected ExternalEquate (`ext_0fff = $0fff` -> `$1000 - 1`) resolves to `$1000`
        // while preserving `line.mnemonic.trim()` as `operand_expr`.
        let mut ext_redirect_state = AppState::new();
        ext_redirect_state.origin = Addr(0x1000);
        ext_redirect_state.raw_data = vec![0x60];
        ext_redirect_state.block_types = vec![BlockType::Code];
        ext_redirect_state.annotations.update(Addr(0x0FFF), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x1000),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
        ext_redirect_state.disassembly = vec![
            DisassemblyLine {
                address: Addr(0x0FFF),
                bytes: Vec::new(),
                mnemonic: "ext_0fff = $0fff".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: Some(Addr(0x0FFF)),
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1000),
                bytes: vec![0x60],
                mnemonic: "rts".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: cpu::opcodes()[0x60].clone(),
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
        ];
        assert_eq!(
            ext_redirect_state.resolve_disassembly_jump_targets(&ext_redirect_state.disassembly[0]),
            vec![OperandJumpTarget::new(
                Addr(0x1000),
                "ext_0fff = $0fff",
                Some("rts".to_string()),
            )]
        );

        // Every non-navigable body block type returns empty.
        for addr in [
            Addr(0x1003), // DataByte
            Addr(0x1005), // DataWord
            Addr(0x1007), // PetsciiText
            Addr(0x1009), // ScreencodeText
            Addr(0x100B), // LoHiWord
            Addr(0x100D), // HiLoWord
            Addr(0x100F), // ExternalFile body row
            Addr(0x1011), // Undefined
        ] {
            let line = state
                .disassembly
                .iter()
                .find(|l| {
                    l.address == addr && l.external_label_address.is_none() && !l.bytes.is_empty()
                })
                .expect("body line for block type");
            assert!(
                state.resolve_disassembly_jump_targets(line).is_empty(),
                "Expected no jump targets for line at {addr}"
            );
        }
    }

    #[test]
    fn core_dis_16_target_preview_edge_cases() {
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // Body rows covering empty mnemonic (no label), empty mnemonic (with label), normal instruction,
        // and an external equate row for `$FFD2`.
        state.raw_data = vec![0xEA, 0xEA, 0x60];
        state.block_types = vec![BlockType::Code; 3];
        state.disassembly = vec![
            DisassemblyLine {
                address: Addr(0xFFD2),
                bytes: Vec::new(),
                mnemonic: "CHROUT = $ffd2".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: Some(Addr(0xFFD2)),
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1000),
                bytes: vec![0xEA],
                mnemonic: String::new(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1001),
                bytes: vec![0xEA],
                mnemonic: String::new(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: Some("only_label".to_string()),
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
            DisassemblyLine {
                address: Addr(0x1002),
                bytes: vec![0x60],
                mnemonic: "rts".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: cpu::opcodes()[0x60].clone(),
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            },
        ];

        let jmp_line = |target: Addr| DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0x20, (target.0 & 0xFF) as u8, (target.0 >> 8) as u8],
            mnemonic: "jsr".to_string(),
            operand: format!("${target:04x}"),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: cpu::opcodes()[0x20].clone(),
            show_bytes: true,
            target_address: Some(target),
            external_label_address: None,
            is_collapsed: false,
        };

        // 1. In-range with empty mnemonic & no label -> `None`.
        assert_eq!(
            state.resolve_disassembly_jump_targets(&jmp_line(Addr(0x1000)))[0].target_preview,
            None
        );
        // 2. In-range with empty mnemonic & non-empty label -> falls back to `label`.
        assert_eq!(
            state.resolve_disassembly_jump_targets(&jmp_line(Addr(0x1001)))[0].target_preview,
            Some("only_label".to_string())
        );
        // 3. In-range instruction with empty operand -> `"rts"`.
        assert_eq!(
            state.resolve_disassembly_jump_targets(&jmp_line(Addr(0x1002)))[0].target_preview,
            Some("rts".to_string())
        );
        // 4. External equate line -> trimmed mnemonic `"CHROUT = $ffd2"`.
        assert_eq!(
            state.resolve_disassembly_jump_targets(&jmp_line(Addr(0xFFD2)))[0].target_preview,
            Some("CHROUT = $ffd2".to_string())
        );
    }

    #[test]
    fn core_dis_extra_02_03_04_identical_reverse_and_tier2_word_table() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // Layout:
        // $1000..=$1005: .word idle, idle, idle ($1020 x3) -> len == 1
        // $1006..=$1009: .word sub, (sub - 1) ($1030, $102F where $1008 has Tier 1 sub - 1) -> keeps "sub"
        // $100A..=$100D: .word (sub - 1), sub ($102F Tier 1, $1030) -> keeps "(sub - 1)" / "sub - 1"
        // $100E..=$1011: .word (sub - 1), struct_field ($102F Tier 1 -> $1030, $103A Tier 2 struct_base + 2 -> $1038)
        let mut raw = vec![
            0x20, 0x10, 0x20, 0x10, 0x20, 0x10, // $1000..=$1005
            0x30, 0x10, 0x2F, 0x10, // $1006..=$1009
            0x2F, 0x10, 0x30, 0x10, // $100A..=$100D
            0x2F, 0x10, 0x3A, 0x10, // $100E..=$1011
        ];
        // Pad $1012..=$103A with NOPs and put RTS at $1020, LDA #$01 at $1030, RTS at $1038
        raw.resize(0x3B, 0xEA);
        raw[0x20] = 0x60; // $1020: RTS
        raw[0x30] = 0xA9; // $1030: LDA #$01
        raw[0x31] = 0x01;
        raw[0x38] = 0x60; // $1038: RTS
        state.raw_data = raw;

        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        for b in &mut state.block_types[0..0x12] {
            *b = BlockType::Address;
        }
        // Split each table onto its own line using virtual splitters so every case is tested independently.
        state.splitters.insert(Addr(0x1006));
        state.splitters.insert(Addr(0x100A));
        state.splitters.insert(Addr(0x100E));

        let tier1_sub = Some(SymbolicOffset {
            base: Addr(0x1030),
            offset: -1,
            radix: OffsetRadix::Decimal,
        });
        state.annotations.update(Addr(0x1008), |e| {
            e.operand_offset = tier1_sub;
        });
        state.annotations.update(Addr(0x100A), |e| {
            e.operand_offset = tier1_sub;
        });
        state.annotations.update(Addr(0x100E), |e| {
            e.operand_offset = tier1_sub;
        });
        state.annotations.update(Addr(0x103A), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x1038),
                offset: 2,
                radix: OffsetRadix::Decimal,
            });
        });

        for (addr, name) in [
            (Addr(0x1020), "idle"),
            (Addr(0x1030), "sub"),
            (Addr(0x1038), "struct_base"),
        ] {
            state.labels.insert(
                addr,
                vec![Label {
                    name: name.to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::Subroutine,
                }],
            );
        }
        state.disassemble();

        let line_at = |addr: Addr| {
            state
                .disassembly
                .iter()
                .find(|l| l.address == addr && !l.bytes.is_empty())
                .expect("table line")
        };

        // 1. All-identical `.word idle, idle, idle` collapses to 1 target.
        assert_eq!(
            state.resolve_disassembly_jump_targets(line_at(Addr(0x1000))),
            vec![OperandJumpTarget::new(
                Addr(0x1020),
                "idle",
                Some("rts".to_string()),
            )]
        );

        // 2. `.word sub, (sub - 1)` keeps leftmost `"sub"`.
        assert_eq!(
            state.resolve_disassembly_jump_targets(line_at(Addr(0x1006))),
            vec![OperandJumpTarget::new(
                Addr(0x1030),
                "sub",
                Some("lda #$01".to_string()),
            )]
        );

        // 3. `.word (sub - 1), sub` keeps leftmost `"sub - 1"`.
        assert_eq!(
            state.resolve_disassembly_jump_targets(line_at(Addr(0x100A))),
            vec![OperandJumpTarget::new(
                Addr(0x1030),
                "sub - 1",
                Some("lda #$01".to_string()),
            )]
        );

        // 4. Tier 1 `sub - 1` ($1030) + Tier 2 `struct_base + 2` ($1038).
        assert_eq!(
            state.resolve_disassembly_jump_targets(line_at(Addr(0x100E))),
            vec![
                OperandJumpTarget::new(Addr(0x1030), "sub - 1", Some("lda #$01".to_string())),
                OperandJumpTarget::new(Addr(0x1038), "struct_base + 2", Some("rts".to_string())),
            ]
        );
    }

    #[test]
    fn core_dis_08_wrapped_multiline_split_tables_lohi_and_hilo() {
        // Pre-Implementation Guidance §2.1 & CORE-DIS-08:
        // `pair_count = 6`, `addresses_per_line = 5` produces 4 lines:
        // L0 (first half, 5B), L1 (first half, 1B), L2 (second half, 5B), L3 (second half, 1B).
        // On L2, `line.address` is `hi_start + 0`, while entry `k = 4` has `second_pair_addr = hi_start + 4`,
        // so `line.address < second_pair_addr` even though L2 IS in the second half.
        let targets = [
            Addr(0x2000),
            Addr(0x2100),
            Addr(0x2200),
            Addr(0x2300),
            Addr(0x2400),
            Addr(0x2500),
        ];

        for (block_type, lo_first) in [
            (BlockType::LoHiAddress, true),
            (BlockType::HiLoAddress, false),
        ] {
            let mut state = AppState::new();
            state.origin = Addr(0x1000);
            state.settings.addresses_per_line = 5;
            state.raw_data = if lo_first {
                let mut v: Vec<u8> = targets.iter().map(|a| (a.0 & 0xFF) as u8).collect();
                v.extend(targets.iter().map(|a| (a.0 >> 8) as u8));
                v
            } else {
                let mut v: Vec<u8> = targets.iter().map(|a| (a.0 >> 8) as u8).collect();
                v.extend(targets.iter().map(|a| (a.0 & 0xFF) as u8));
                v
            };
            state.block_types = vec![block_type; 12];
            state.disassemble();

            let lines: Vec<_> = state
                .disassembly
                .iter()
                .filter(|l| !l.bytes.is_empty())
                .collect();
            assert_eq!(
                lines.len(),
                4,
                "expected 4 split-table lines for {block_type:?}"
            );
            assert_eq!(lines[0].bytes.len(), 5);
            assert_eq!(lines[1].bytes.len(), 1);
            assert_eq!(lines[2].bytes.len(), 5);
            assert_eq!(lines[3].bytes.len(), 1);

            let first_prefix = if lo_first { '<' } else { '>' };
            let second_prefix = if lo_first { '>' } else { '<' };

            // Assert `k = 4` on L2 indeed satisfies `line.address < second_pair_addr`.
            let k4_addr = lines[2].address.wrapping_add(4);
            assert!(lines[2].address < k4_addr);

            let check_four_lines = |test_lines: &[&DisassemblyLine]| {
                let t0 = state.resolve_disassembly_jump_targets(test_lines[0]);
                assert_eq!(t0.len(), 5);
                for (i, t) in t0.iter().enumerate() {
                    assert_eq!(t.target_address, targets[i]);
                    assert!(
                        t.operand_expr.starts_with(first_prefix),
                        "L0[{i}] expected `{first_prefix}`, got `{}`",
                        t.operand_expr
                    );
                }

                let t1 = state.resolve_disassembly_jump_targets(test_lines[1]);
                assert_eq!(t1.len(), 1);
                assert_eq!(t1[0].target_address, targets[5]);
                assert!(t1[0].operand_expr.starts_with(first_prefix));

                let t2 = state.resolve_disassembly_jump_targets(test_lines[2]);
                assert_eq!(t2.len(), 5);
                for (i, t) in t2.iter().enumerate() {
                    assert_eq!(t.target_address, targets[i]);
                    assert!(
                        t.operand_expr.starts_with(second_prefix),
                        "L2[{i}] expected `{second_prefix}`, got `{}`",
                        t.operand_expr
                    );
                }

                let t3 = state.resolve_disassembly_jump_targets(test_lines[3]);
                assert_eq!(t3.len(), 1);
                assert_eq!(t3[0].target_address, targets[5]);
                assert!(t3[0].operand_expr.starts_with(second_prefix));
            };

            // 1. Disassembled lines.
            check_four_lines(&lines);

            // 2. Synthetic arity-mismatched operands forcing fallback prefix generation.
            let mut mismatched: Vec<_> = lines.iter().map(|&l| l.clone()).collect();
            for l in &mut mismatched {
                l.operand = "mismatched_only_one".to_string();
            }
            // Also force empty operand on L1 and L3 (`len == 1`) so fallback branch runs for 1-byte tails too.
            mismatched[1].operand.clear();
            mismatched[3].operand.clear();
            let mismatched_refs: Vec<_> = mismatched.iter().collect();
            check_four_lines(&mismatched_refs);
        }
    }

    #[test]
    fn core_dis_09_wrapped_word_table_across_addresses_per_line() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.addresses_per_line = 5;
        // 6 `.word` entries (12 bytes): $2000..=$2500
        state.raw_data = vec![
            0x00, 0x20, 0x00, 0x21, 0x00, 0x22, 0x00, 0x23, 0x00, 0x24, 0x00, 0x25,
        ];
        state.block_types = vec![BlockType::Address; 12];
        state.disassemble();

        let lines: Vec<_> = state
            .disassembly
            .iter()
            .filter(|l| !l.bytes.is_empty())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].bytes.len(), 10);
        assert_eq!(lines[1].bytes.len(), 2);

        let line0_targets = state.resolve_disassembly_jump_targets(lines[0]);
        assert_eq!(line0_targets.len(), 5);
        assert_eq!(
            line0_targets
                .iter()
                .map(|t| t.target_address)
                .collect::<Vec<_>>(),
            vec![
                Addr(0x2000),
                Addr(0x2100),
                Addr(0x2200),
                Addr(0x2300),
                Addr(0x2400),
            ]
        );

        let line1_targets = state.resolve_disassembly_jump_targets(lines[1]);
        assert_eq!(line1_targets.len(), 1);
        assert_eq!(line1_targets[0].target_address, Addr(0x2500));
    }

    #[test]
    fn core_dis_11_odd_trailing_orphan_byte_on_split_tables() {
        for block_type in [BlockType::LoHiAddress, BlockType::HiLoAddress] {
            // 1-byte split table (`pair_count == 0`) -> single orphan row returning empty `Vec`.
            let mut single = AppState::new();
            single.origin = Addr(0x1000);
            single.raw_data = vec![0x42];
            single.block_types = vec![block_type];
            single.disassemble();
            let single_line = single
                .disassembly
                .iter()
                .find(|l| !l.bytes.is_empty())
                .expect("1-byte orphan line");
            assert!(
                single
                    .resolve_disassembly_jump_targets(single_line)
                    .is_empty()
            );

            // 5-byte split table -> 2 pairs (`L0`, `L1` each yield 2 targets) + 1 orphan byte returning empty `Vec`.
            let mut five = AppState::new();
            five.origin = Addr(0x1000);
            five.raw_data = vec![0x00, 0x01, 0x20, 0x21, 0x99];
            five.block_types = vec![block_type; 5];
            five.disassemble();
            let lines: Vec<_> = five
                .disassembly
                .iter()
                .filter(|l| !l.bytes.is_empty())
                .collect();
            assert_eq!(lines.len(), 3);
            assert_eq!(five.resolve_disassembly_jump_targets(lines[0]).len(), 2);
            assert_eq!(five.resolve_disassembly_jump_targets(lines[1]).len(), 2);
            assert!(five.resolve_disassembly_jump_targets(lines[2]).is_empty());
        }
    }

    #[test]
    fn core_dis_14_whitespace_no_space_arity_and_code_commas() {
        use crate::disassembler::DisassemblyLine;

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..=$1003: `.word` [$2000, $2010]
        // $1004..=$1005: `LDA ($FB),Y` (B1 FB)
        // $1006..=$1008: `LDA $102F,X` (BD 2F 10)
        state.raw_data = vec![0x00, 0x20, 0x10, 0x20, 0xB1, 0xFB, 0xBD, 0x2F, 0x10];
        state.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
        ];
        state.disassemble();

        // (a) Whitespace-padded operand trims each part cleanly.
        let padded_line = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0x00, 0x20, 0x10, 0x20],
            mnemonic: ".word".to_string(),
            operand: "  alpha,   beta  ".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        assert_eq!(
            state.resolve_disassembly_jump_targets(&padded_line),
            vec![
                OperandJumpTarget::new(Addr(0x2000), "alpha", Some("(out of range)".to_string())),
                OperandJumpTarget::new(Addr(0x2010), "beta", Some("(out of range)".to_string())),
            ]
        );

        // (b) No-space `"alpha,beta"` -> arity `1 != 2` -> falls back deterministically to hex lookup.
        let no_space_line = DisassemblyLine {
            operand: "alpha,beta".to_string(),
            ..padded_line
        };
        assert_eq!(
            state.resolve_disassembly_jump_targets(&no_space_line),
            vec![
                OperandJumpTarget::new(Addr(0x2000), "$2000", Some("(out of range)".to_string())),
                OperandJumpTarget::new(Addr(0x2010), "$2010", Some("(out of range)".to_string())),
            ]
        );

        // (c) `Code` with `", "` (for example synthetic `($FB), Y` or `$102F, X`) is NEVER split.
        let ind_y = DisassemblyLine {
            address: Addr(0x1004),
            bytes: vec![0xB1, 0xFB],
            mnemonic: "lda".to_string(),
            operand: "($FB), Y".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: cpu::opcodes()[0xB1].clone(),
            show_bytes: true,
            target_address: Some(Addr(0x00FB)),
            external_label_address: None,
            is_collapsed: false,
        };
        assert_eq!(
            state.resolve_disassembly_jump_targets(&ind_y),
            vec![OperandJumpTarget::new(
                Addr(0x00FB),
                "($FB), Y",
                Some("(out of range)".to_string()),
            )]
        );

        let abs_x = DisassemblyLine {
            address: Addr(0x1006),
            bytes: vec![0xBD, 0x2F, 0x10],
            mnemonic: "lda".to_string(),
            operand: "$102F, X".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: cpu::opcodes()[0xBD].clone(),
            show_bytes: true,
            target_address: Some(Addr(0x102F)),
            external_label_address: None,
            is_collapsed: false,
        };
        assert_eq!(
            state.resolve_disassembly_jump_targets(&abs_x),
            vec![OperandJumpTarget::new(
                Addr(0x102F),
                "$102F, X",
                Some("(out of range)".to_string()),
            )]
        );
    }

    #[test]
    fn core_dis_15_utf8_preview_and_all_four_assemblers() {
        // Part 1: multi-byte UTF-8 label in out-of-range and in-range `target_preview` renders without panic.
        let mut utf8_state = AppState::new();
        utf8_state.origin = Addr(0x1000);
        utf8_state.raw_data = vec![0x00, 0x20, 0x04, 0x10, 0xEA]; // .word $2000, $1004; $1004: NOP
        utf8_state.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Address,
            BlockType::Code,
        ];
        utf8_state.labels.insert(
            Addr(0x2000),
            vec![Label {
                name: "rocket_🚀_café".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        utf8_state.annotations.update(Addr(0x1004), |e| {
            e.user_side_comment = Some("🚀 café — loop".to_string());
        });
        utf8_state.disassemble();
        let word_line = utf8_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect("utf8 .word line");
        let utf8_targets = utf8_state.resolve_disassembly_jump_targets(word_line);
        assert_eq!(utf8_targets.len(), 2);
        assert_eq!(
            utf8_targets[0].target_preview.as_deref(),
            Some("rocket_🚀_café (out of range)")
        );
        assert_eq!(utf8_targets[1].target_preview.as_deref(), Some("nop"));

        // Part 2: all 4 Assembler variants preserve local/scoped `operand_expr` syntax on `.word`.
        for assembler in [
            Assembler::Tass64,
            Assembler::Acme,
            Assembler::Ca65,
            Assembler::Kick,
        ] {
            let mut asm_state = AppState::new();
            asm_state.origin = Addr(0x1000);
            asm_state.settings.assembler = assembler;
            // $1000..=$1005: `.word local_tgt, scoped_tgt, $2000`
            // $1006: NOP (local inside scope_a), $1007..=$1008: scope_b ($1007: RTS, $1008: NOP)
            asm_state.raw_data = vec![0x06, 0x10, 0x08, 0x10, 0x00, 0x20, 0xEA, 0x60, 0xEA];
            asm_state.block_types = vec![
                BlockType::Address,
                BlockType::Address,
                BlockType::Address,
                BlockType::Address,
                BlockType::Address,
                BlockType::Address,
                BlockType::Code,
                BlockType::Code,
                BlockType::Code,
            ];
            asm_state.annotations.update(Addr(0x1000), |e| {
                e.scope = Some(Addr(0x1006));
            });
            asm_state.annotations.update(Addr(0x1007), |e| {
                e.scope = Some(Addr(0x1008));
            });
            asm_state.labels.insert(
                Addr(0x1000),
                vec![Label {
                    name: "outer_a".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::UserDefined,
                }],
            );
            asm_state.labels.insert(
                Addr(0x1006),
                vec![Label {
                    name: "local_in_a".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::LocalUserDefined,
                }],
            );
            asm_state.labels.insert(
                Addr(0x1007),
                vec![Label {
                    name: "scope_b".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::Subroutine,
                }],
            );
            asm_state.labels.insert(
                Addr(0x1008),
                vec![Label {
                    name: "inner_b".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::LocalUserDefined,
                }],
            );
            asm_state.disassemble();

            let asm_word = asm_state
                .disassembly
                .iter()
                .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
                .expect(".word line");
            let expected_exprs: Vec<String> = asm_word
                .operand
                .split(", ")
                .map(|s| s.trim().to_string())
                .collect();
            let resolved = asm_state.resolve_disassembly_jump_targets(asm_word);
            assert_eq!(resolved.len(), 3, "assembler {assembler:?}");
            assert_eq!(
                resolved
                    .iter()
                    .map(|t| t.operand_expr.clone())
                    .collect::<Vec<_>>(),
                expected_exprs,
                "operand_expr mismatch under {assembler:?}"
            );
            assert_eq!(resolved[2].operand_expr, "$2000");
        }
    }

    #[test]
    fn core_dis_16_real_disassemble_zero_enum_preamble_collapsed_and_body() {
        use std::collections::BTreeMap;

        let setup_enum_and_external = |state: &mut AppState| {
            state.origin = Addr::ZERO;
            state.settings.all_labels = true;
            // $0000: LDA #$00 (enum Colors), $0002: STA $D020 (external), $0005: RTS
            state.raw_data = vec![0xA9, 0x00, 0x8D, 0x20, 0xD0, 0x60];
            state.block_types = vec![BlockType::Code; state.raw_data.len()];
            let mut variants = BTreeMap::new();
            variants.insert(0, "BLACK".to_string());
            state.enums.insert(
                "Colors".to_string(),
                EnumDefinition {
                    name: "Colors".to_string(),
                    description: None,
                    variants,
                    source_file: None,
                },
            );
            state.annotations.update(Addr::ZERO, |e| {
                e.enum_usage = Some("Colors".to_string());
            });
            state.labels.insert(
                Addr(0xD020),
                vec![Label {
                    name: "VIC_BORDER".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::AbsoluteAddress,
                }],
            );
        };

        // (a) `(0, 2)` collapsed at `$0000` with `all_labels = true` and active enum preamble.
        let mut collapsed_state = AppState::new();
        setup_enum_and_external(&mut collapsed_state);
        collapsed_state.collapsed_blocks.push((0, 2));
        collapsed_state.disassemble();

        assert!(
            collapsed_state
                .disassembly
                .first()
                .is_some_and(|l| l.address == Addr::ZERO && l.bytes.is_empty()),
            "expected real enum/equate header preamble at index 0"
        );
        let collapsed_idx = collapsed_state
            .line_index_containing_address(Addr::ZERO)
            .expect("collapsed $0000 index");
        assert!(collapsed_state.disassembly[collapsed_idx].is_collapsed);
        assert_eq!(
            collapsed_state.disassembly[collapsed_idx].address,
            Addr::ZERO
        );

        // (b) Non-collapsed `$0000` instruction preceded by `{splitter}` and scope at `$0000`.
        let mut body_state = AppState::new();
        setup_enum_and_external(&mut body_state);
        body_state.splitters.insert(Addr::ZERO);
        body_state.annotations.update(Addr::ZERO, |e| {
            e.scope = Some(Addr(0x0005));
        });
        body_state.labels.insert(
            Addr::ZERO,
            vec![Label {
                name: "zp_entry".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        body_state.disassemble();

        let body_idx = body_state
            .line_index_containing_address(Addr::ZERO)
            .expect("body $0000 index");
        let line = &body_state.disassembly[body_idx];
        assert!(!line.bytes.is_empty());
        assert!(line.external_label_address.is_none());
        assert_eq!(line.address, Addr::ZERO);
    }
}
