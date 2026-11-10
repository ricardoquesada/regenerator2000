//! Static control-flow and data-reference analyzer for MOS 6502 binaries.
//!
//! Scans [`AppState`] blocks to decode 6502 instructions, pointer tables, and indirect vectors.
//! Redirects active [`SymbolicOffset`](crate::state::SymbolicOffset) targets to `sym.base`,
//! generates canonical [`LabelType`] auto-labels, builds typed cross-references, and runs
//! reachability analysis ([`flow_analyze`]).

use crate::cpu::{AddressingMode, Opcode};
use crate::disassembler::symbols::{
    resolve_effective_symbolic_offset_for_mode, resolve_split_pair_effective_symbolic_offset,
};
use crate::state::{
    Addr, AppState, BlockType, ImmediateFormat, Label, LabelKind, LabelType, XRef, XRefKind,
};
use std::collections::BTreeMap;

/// Contains the generated label map and typed cross-reference index from [`analyze`].
pub struct AnalysisResult {
    /// Address-keyed map of preserved user/system labels and generated [`LabelKind::Auto`] labels.
    pub labels: BTreeMap<Addr, Vec<Label>>,
    /// Inverted index that maps each target [`Addr`] to a sorted, deduplicated [`XRef`] list.
    pub cross_refs: BTreeMap<Addr, Vec<XRef>>,
}

/// Stores incoming [`XRef`] edges and the first observed [`LabelType`] for a target address.
type UsageData = (Vec<XRef>, LabelType);

/// Redirects `raw_target` to `sym.base` when an active [`SymbolicOffset`](crate::state::SymbolicOffset) applies.
///
/// Normalizes `label_type` only when an offset applies, preserving 16-bit prefixes on direct 3-byte zero-page operands.
#[must_use]
fn resolve_analyzed_target(
    site_addr: Addr,
    raw_target: Addr,
    label_type: LabelType,
    is_zero_page_mode: bool,
    state: &AppState,
) -> (Addr, LabelType) {
    if let Some(sym) = resolve_effective_symbolic_offset_for_mode(
        site_addr,
        raw_target,
        is_zero_page_mode,
        &state.labels,
        &state.annotations,
    ) {
        (sym.base, label_type.normalize_for_address(sym.base))
    } else {
        (raw_target, label_type)
    }
}

/// Runs a full static analysis pass over `state` and returns generated labels and cross-references.
///
/// The analyzer executes four stages:
///
/// 1. **Linear scan**: Decodes [`BlockType::Code`] instructions and address tables, routing any
///    active [`SymbolicOffset`](crate::state::SymbolicOffset) reference to `sym.base`.
/// 2. **Return-stub promotion**: Promotes jump and branch targets that start with `RTS` (`$60`)
///    or `RTI` (`$40`) to [`LabelType::Return`].
/// 3. **Immediate byte offsets**: Records [`XRefKind::Offset`] edges for `LowByte` and `HighByte`
///    immediates, redirecting displaced pointers to `sym.base`.
/// 4. **Normalization**: Sorts and deduplicates [`XRef`] lists by `(from, kind)` and generates
///    auto-labels while preserving user and system labels.
#[must_use]
pub fn analyze(state: &AppState) -> AnalysisResult {
    let mut usage_map: BTreeMap<Addr, UsageData> = BTreeMap::new();
    let mut pc = 0;
    let data_len = state.raw_data.len();
    let origin = state.origin;

    while pc < data_len {
        let current_type = state
            .block_types
            .get(pc)
            .copied()
            .unwrap_or(BlockType::Code);

        match current_type {
            BlockType::Code => {
                let opcode_byte = state.raw_data[pc];
                let Some(opcode) = &state.disassembler.opcodes[opcode_byte as usize] else {
                    pc += 1;
                    continue;
                };
                if opcode.illegal && !state.settings.use_illegal_opcodes {
                    pc += 1;
                    continue;
                }

                // Consume the signature byte for 2-byte `BRK` when the next byte is also `Code`.
                if opcode.mnemonic == "BRK"
                    && !state.settings.brk_single_byte
                    && pc + 1 < data_len
                    && state.block_types.get(pc + 1) == Some(&BlockType::Code)
                {
                    pc += 2;
                    continue;
                }

                let size = opcode.size as usize;
                let all_code = pc + size <= data_len
                    && (1..size).all(|i| state.block_types.get(pc + i) == Some(&BlockType::Code));

                if all_code {
                    analyze_instruction(
                        state,
                        opcode,
                        &state.raw_data[pc + 1..pc + size],
                        origin.wrapping_add(pc as u16),
                        &mut usage_map,
                    );
                    pc += size;
                } else {
                    pc += 1;
                }
            }
            BlockType::Address => {
                if has_unsplit_pair(state, pc, BlockType::Address) {
                    let low = state.raw_data[pc];
                    let high = state.raw_data[pc + 1];
                    let val = Addr(u16::from(high) << 8 | u16::from(low));
                    let site_addr = origin.wrapping_add(pc as u16);
                    let (effective_target, effective_label_type) = resolve_analyzed_target(
                        site_addr,
                        val,
                        LabelType::AbsoluteAddress,
                        false,
                        state,
                    );
                    update_usage(
                        &mut usage_map,
                        effective_target,
                        effective_label_type,
                        XRef::new(site_addr, XRefKind::Offset),
                    );
                    pc += 2;
                } else {
                    pc += 1;
                }
            }
            BlockType::DataWord => {
                if has_unsplit_pair(state, pc, BlockType::DataWord) {
                    pc += 2;
                } else {
                    pc += 1;
                }
            }
            BlockType::LoHiAddress => {
                let count = contiguous_block_len(state, pc, BlockType::LoHiAddress);
                analyze_split_address_table(state, pc, count, true, &mut usage_map);
                pc += count;
            }
            BlockType::HiLoAddress => {
                let count = contiguous_block_len(state, pc, BlockType::HiLoAddress);
                analyze_split_address_table(state, pc, count, false, &mut usage_map);
                pc += count;
            }
            BlockType::LoHiWord | BlockType::HiLoWord => {
                pc += contiguous_block_len(state, pc, current_type);
            }
            _ => {
                pc += 1;
            }
        }
    }

    promote_return_labels(state, &mut usage_map);

    let mut labels: BTreeMap<Addr, Vec<Label>> = BTreeMap::new();
    let mut cross_refs: BTreeMap<Addr, Vec<XRef>> = BTreeMap::new();

    // Record immediate low-byte and high-byte pointer references.
    for (addr, entry) in state.annotations.iter() {
        let Some(ImmediateFormat::LowByte(target_addr) | ImmediateFormat::HighByte(target_addr)) =
            &entry.immediate_format
        else {
            continue;
        };
        let (effective_target, effective_label_type) =
            resolve_analyzed_target(addr, *target_addr, LabelType::Pointer, false, state);
        update_usage(
            &mut usage_map,
            effective_target,
            effective_label_type,
            XRef::new(addr, XRefKind::Offset),
        );
    }

    // Normalize cross-references and build canonical label lists for referenced addresses.
    for (addr, (mut refs, first_type)) in usage_map {
        let mut addr_labels = Vec::new();

        if !refs.is_empty() {
            refs.sort_unstable();
            refs.dedup();
            cross_refs.insert(addr, refs);
        }

        if let Some(existing_vec) = state.labels.get(&addr) {
            for existing in existing_vec {
                if existing.kind == LabelKind::User {
                    addr_labels.push(existing.clone());
                }
            }
        }

        if !state.excluded_addresses.contains(&addr) {
            if let Some(sys_vec) = state.system_labels.get(&addr).filter(|v| !v.is_empty()) {
                addr_labels.extend(sys_vec.iter().cloned());
            } else if let Some(existing_vec) = state.labels.get(&addr) {
                for existing in existing_vec {
                    if existing.kind == LabelKind::System {
                        addr_labels.push(existing.clone());
                    }
                }
            }
        }

        if addr_labels.is_empty() && !state.excluded_addresses.contains(&addr) {
            let final_type = if state.is_external(addr)
                && matches!(
                    first_type,
                    LabelType::Jump | LabelType::Subroutine | LabelType::Branch | LabelType::Return
                ) {
                LabelType::ExternalJump
            } else {
                first_type
            };

            addr_labels.push(Label {
                name: final_type.format_label(addr.0),
                label_type: final_type,
                kind: LabelKind::Auto,
            });
        }

        if !addr_labels.is_empty() {
            labels.insert(addr, addr_labels);
        }
    }

    // Preserve unreferenced user-defined labels.
    for (addr, label_vec) in &state.labels {
        if labels.contains_key(addr) {
            continue;
        }
        let preserved: Vec<Label> = label_vec
            .iter()
            .filter(|l| l.kind == LabelKind::User)
            .cloned()
            .collect();
        if !preserved.is_empty() {
            labels.insert(*addr, preserved);
        }
    }

    AnalysisResult { labels, cross_refs }
}

/// Returns `true` when `pc` and `pc + 1` both have `block_type` with no splitter between them.
#[must_use]
fn has_unsplit_pair(state: &AppState, pc: usize, block_type: BlockType) -> bool {
    pc + 2 <= state.raw_data.len()
        && state.block_types.get(pc) == Some(&block_type)
        && state.block_types.get(pc + 1) == Some(&block_type)
        && !state.is_virtual_splitter(state.origin.wrapping_add((pc + 1) as u16))
}

/// Returns the byte length of the contiguous split-table `block_type` sub-block starting at `pc`,
/// stopping at virtual splitters and user line/side comments via
/// [`AppState::is_split_table_subblock_boundary`].
///
/// Sharing [`AppState::is_split_table_subblock_boundary`] guarantees that adjacent `LoHiAddress`
/// or `HiLoAddress` tables separated solely by a user comment pair `(lo, hi)` bytes identically
/// across the analyzer, `AppState`, and `disassembler::handlers`.
#[must_use]
fn contiguous_block_len(state: &AppState, pc: usize, block_type: BlockType) -> usize {
    let data_len = state.raw_data.len();
    let origin = state.origin;
    let mut count = 0;

    while pc + count < data_len && state.block_types.get(pc + count) == Some(&block_type) {
        if count > 0
            && state.is_split_table_subblock_boundary(origin.wrapping_add((pc + count) as u16))
        {
            break;
        }
        count += 1;
    }

    count
}

/// Extracts 16-bit target addresses from a split `LoHiAddress` or `HiLoAddress` table.
///
/// Queries Tier 1 `operand_offset` on both `first_addr` (canonical) and `second_addr` (fallback)
/// before Tier 2 `address_offset`, while emitting strictly **one** [`XRefKind::Offset`] edge from
/// `first_addr` per pair.
fn analyze_split_address_table(
    state: &AppState,
    pc: usize,
    count: usize,
    lo_first: bool,
    usage_map: &mut BTreeMap<Addr, UsageData>,
) {
    let pair_count = count / 2;
    let origin = state.origin;

    for i in 0..pair_count {
        let first = state.raw_data[pc + i];
        let second = state.raw_data[pc + pair_count + i];
        let (lo, hi) = if lo_first {
            (first, second)
        } else {
            (second, first)
        };
        let val = Addr(u16::from(hi) << 8 | u16::from(lo));
        let first_addr = origin.wrapping_add((pc + i) as u16);
        let second_addr = origin.wrapping_add((pc + pair_count + i) as u16);
        let (effective_target, effective_label_type) = if let Some(sym) =
            resolve_split_pair_effective_symbolic_offset(
                first_addr,
                second_addr,
                val,
                &state.labels,
                &state.annotations,
            ) {
            (
                sym.base,
                LabelType::AbsoluteAddress.normalize_for_address(sym.base),
            )
        } else {
            (val, LabelType::AbsoluteAddress)
        };
        update_usage(
            usage_map,
            effective_target,
            effective_label_type,
            XRef::new(first_addr, XRefKind::Offset),
        );
    }
}

/// Decodes one instruction's operand target address and records its [`LabelType`] and [`XRef`].
fn analyze_instruction(
    state: &AppState,
    opcode: &Opcode,
    operands: &[u8],
    address: Addr,
    usage_map: &mut BTreeMap<Addr, UsageData>,
) {
    let kind = opcode.xref_kind().unwrap_or(XRefKind::Read);
    let xref = XRef::new(address, kind);

    match opcode.mode {
        AddressingMode::Implied
        | AddressingMode::Accumulator
        | AddressingMode::Immediate
        | AddressingMode::Unknown => {}
        AddressingMode::ZeroPage => {
            if let Some(&zp) = operands.first() {
                let (effective_target, effective_label_type) = resolve_analyzed_target(
                    address,
                    Addr(u16::from(zp)),
                    LabelType::ZeroPageAbsoluteAddress,
                    true,
                    state,
                );
                update_usage(usage_map, effective_target, effective_label_type, xref);
            }
        }
        AddressingMode::ZeroPageX | AddressingMode::ZeroPageY => {
            if let Some(&zp) = operands.first() {
                let (effective_target, effective_label_type) = resolve_analyzed_target(
                    address,
                    Addr(u16::from(zp)),
                    LabelType::ZeroPageField,
                    true,
                    state,
                );
                update_usage(usage_map, effective_target, effective_label_type, xref);
            }
        }
        AddressingMode::Relative => {
            if let Some(&rel) = operands.first() {
                let offset = rel as i8;
                let target = Addr((address.0.wrapping_add(2)).wrapping_add(offset as u16));
                let (effective_target, effective_label_type) =
                    resolve_analyzed_target(address, target, LabelType::Branch, false, state);
                update_usage(usage_map, effective_target, effective_label_type, xref);
            }
        }
        AddressingMode::Absolute => {
            if operands.len() >= 2 {
                let target = Addr(u16::from(operands[1]) << 8 | u16::from(operands[0]));
                let label_type = match opcode.mnemonic {
                    "JSR" => LabelType::Subroutine,
                    "JMP" => LabelType::Jump,
                    _ => LabelType::AbsoluteAddress,
                };
                let (effective_target, effective_label_type) =
                    resolve_analyzed_target(address, target, label_type, false, state);
                update_usage(usage_map, effective_target, effective_label_type, xref);
            }
        }
        AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => {
            if operands.len() >= 2 {
                let target = Addr(u16::from(operands[1]) << 8 | u16::from(operands[0]));
                let (effective_target, effective_label_type) =
                    resolve_analyzed_target(address, target, LabelType::Field, false, state);
                update_usage(usage_map, effective_target, effective_label_type, xref);
            }
        }
        AddressingMode::Indirect => {
            if operands.len() >= 2 {
                let pointer_addr = Addr(u16::from(operands[1]) << 8 | u16::from(operands[0]));
                let (effective_target, effective_label_type) = resolve_analyzed_target(
                    address,
                    pointer_addr,
                    LabelType::Pointer,
                    false,
                    state,
                );
                update_usage(usage_map, effective_target, effective_label_type, xref);

                // When `JMP ($xxxx)` points to an internal 2-byte `BlockType::Address` vector,
                // route the `Pointer` usage above to `effective_target` (`sym.base`) while still
                // dereferencing `raw_data` at the actual runtime `pointer_addr` vector bytes.
                if opcode.mnemonic == "JMP" && !state.is_external(pointer_addr) {
                    let ptr_offset = pointer_addr.offset_from(state.origin);
                    if has_unsplit_pair(state, ptr_offset, BlockType::Address) {
                        let raw_dest = Addr(
                            u16::from(state.raw_data[ptr_offset + 1]) << 8
                                | u16::from(state.raw_data[ptr_offset]),
                        );
                        let (dest_target, dest_type) = resolve_analyzed_target(
                            pointer_addr,
                            raw_dest,
                            LabelType::Jump,
                            false,
                            state,
                        );
                        update_usage(usage_map, dest_target, dest_type, xref);
                        if let Some((_, first_type)) = usage_map.get_mut(&dest_target)
                            && *first_type == LabelType::AbsoluteAddress
                        {
                            *first_type = LabelType::Jump;
                        }
                    }
                }
            }
        }
        AddressingMode::IndirectX | AddressingMode::IndirectY => {
            if let Some(&base) = operands.first() {
                let (effective_target, effective_label_type) = resolve_analyzed_target(
                    address,
                    Addr(u16::from(base)),
                    LabelType::ZeroPagePointer,
                    true,
                    state,
                );
                update_usage(usage_map, effective_target, effective_label_type, xref);
            }
        }
    }
}

/// Promotes `Branch`, `Jump`, and `Subroutine` targets to [`LabelType::Return`] when the target
/// is an internal [`BlockType::Code`] byte containing `RTS` (`$60`) or `RTI` (`$40`).
fn promote_return_labels(state: &AppState, usage_map: &mut BTreeMap<Addr, UsageData>) {
    let origin = state.origin;
    let data = &state.raw_data;

    for (addr, (_refs, first_type)) in usage_map.iter_mut() {
        if !matches!(
            first_type,
            LabelType::Branch | LabelType::Jump | LabelType::Subroutine
        ) {
            continue;
        }
        if state.is_external(*addr) {
            continue;
        }

        let offset = addr.offset_from(origin);
        if state
            .block_types
            .get(offset)
            .copied()
            .unwrap_or(BlockType::Code)
            != BlockType::Code
        {
            continue;
        }

        if matches!(data[offset], 0x60 | 0x40) {
            *first_type = LabelType::Return;
        }
    }
}

/// Appends `xref` to `addr` in `map` and retains the first [`LabelType`] recorded for `addr`.
fn update_usage(map: &mut BTreeMap<Addr, UsageData>, addr: Addr, priority: LabelType, xref: XRef) {
    map.entry(addr)
        .and_modify(|(refs, _first)| {
            refs.push(xref);
        })
        .or_insert_with(|| (vec![xref], priority));
}

/// Traces reachable code segments from `start` using breadth-first search and returns their
/// byte-offset ranges in `state.raw_data`.
#[must_use]
pub fn flow_analyze(state: &AppState, start: Addr) -> Vec<std::ops::Range<usize>> {
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut ranges = Vec::new();

    let origin = state.origin;
    let data_len = state.raw_data.len();

    let to_offset = |addr: Addr| -> Option<usize> {
        let offset = addr.offset_from(origin);
        (offset < data_len).then_some(offset)
    };

    let Some(offset) = to_offset(start) else {
        return ranges;
    };
    queue.push_back(offset);

    while let Some(mut pc) = queue.pop_front() {
        if visited.contains(&pc) {
            continue;
        }

        let segment_start = pc;

        loop {
            if pc >= data_len || visited.contains(&pc) {
                if pc > segment_start {
                    ranges.push(segment_start..pc);
                }
                break;
            }

            let opcode_byte = state.raw_data[pc];
            let Some(op) = state.disassembler.opcodes[opcode_byte as usize].as_ref() else {
                if pc > segment_start {
                    ranges.push(segment_start..pc);
                }
                break;
            };

            if (op.illegal && !state.settings.use_illegal_opcodes)
                || pc + op.size as usize > data_len
            {
                if pc > segment_start {
                    ranges.push(segment_start..pc);
                }
                break;
            }

            visited.insert(pc);
            let next_pc = pc + op.size as usize;

            match op.mnemonic {
                "JMP" => {
                    if op.mode == AddressingMode::Absolute && op.size == 3 {
                        let lo = state.raw_data[pc + 1];
                        let hi = state.raw_data[pc + 2];
                        let target_addr = Addr(u16::from(hi) << 8 | u16::from(lo));
                        if let Some(target_offset) = to_offset(target_addr) {
                            queue.push_back(target_offset);
                        }
                    }
                    if next_pc > segment_start {
                        ranges.push(segment_start..next_pc);
                    }
                    break;
                }
                "RTS" | "RTI" => {
                    if next_pc > segment_start {
                        ranges.push(segment_start..next_pc);
                    }
                    break;
                }
                "JSR" => {
                    if op.size == 3 {
                        let lo = state.raw_data[pc + 1];
                        let hi = state.raw_data[pc + 2];
                        let target_addr = Addr(u16::from(hi) << 8 | u16::from(lo));
                        if let Some(target_offset) = to_offset(target_addr) {
                            queue.push_back(target_offset);
                        }
                    }
                    pc = next_pc;
                }
                _ => {
                    if op.mode == AddressingMode::Relative {
                        let rel_offset = state.raw_data[pc + 1] as i8;
                        let current_addr_u16 = origin.0.wrapping_add(pc as u16);
                        let target_addr = Addr(
                            (current_addr_u16.wrapping_add(2)).wrapping_add(rel_offset as u16),
                        );
                        if let Some(target_offset) = to_offset(target_addr) {
                            queue.push_back(target_offset);
                        }
                    }
                    pc = next_pc;
                }
            }
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, BlockType};

    #[test]
    fn test_analyze_simple() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // JMP $1005 (4C 05 10)
        // JSR $1008 (20 08 10)
        // NOP (EA)
        // LDA $1000 (AD 00 10)
        let data = vec![0x4C, 0x05, 0x10, 0x20, 0x08, 0x10, 0xEA, 0xAD, 0x00, 0x10];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let result = analyze(&state);
        let labels = result.labels;

        // $1005 is JMP target -> j1005
        assert_eq!(
            labels
                .get(&0x1005)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("j_1005")
        );
        // $1008 is JSR target -> s1008
        assert_eq!(
            labels
                .get(&0x1008)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("s_1008")
        );
        // $1000 is accessed via LDA (Absolute) -> a1000
        assert_eq!(
            labels
                .get(&0x1000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("a_1000")
        );
    }

    #[test]
    fn test_priority_override() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // JMP $2000 (4C 00 20) -> usage j2000 -> external e2000
        // JSR $2000 (20 00 20) -> usage s2000 -> external e2000
        // Since both are external, and both allow external prefix, result is e2000.
        let data = vec![0x4C, 0x00, 0x20, 0x20, 0x00, 0x20];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let result = analyze(&state);
        let labels = result.labels;
        // Changed expectations: explicit ExternalJump type logic
        assert_eq!(
            labels
                .get(&0x2000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e_2000")
        );
    }

    #[test]
    fn test_access_types_zp() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // LDA $10 (ZP) -> A5 10
        let data = vec![0xA5, 0x10];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 2];

        let result = analyze(&state);
        let labels = result.labels;
        // ZP access -> ZeroPage Priority -> a10
        assert_eq!(
            labels
                .get(&0x0010)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("zpa_10")
        );
    }

    #[test]
    fn test_zp_field() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // LDA $50, X (B5 50) -> Field usage, ZP address
        let data = vec![0xB5, 0x50];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 2];

        let result = analyze(&state);
        let labels = result.labels;
        // Field usage in ZP -> f50
        assert_eq!(
            labels
                .get(&0x0050)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("zpf_50")
        );
    }

    #[test]
    fn test_external() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // JMP $0010 (External, out of range [1000..1003])
        let data = vec![0x4C, 0x10, 0x00];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 3];

        let result = analyze(&state);
        let labels = result.labels;
        // External Jump -> e0010
        // (Note: Jumps use 4 digits usually, unless we want e10?
        // User said "Only for external jumps... not for data".
        // Standard external handling logic still uses 4 digits for 'e' prefix in my code:
        // format!("{}{:04X}", prefix, addr)
        // ZeroPage special formatting is only for ZP priority or ZP Field.
        // Jump is Jump priority. So e0010 is correct.)
        assert_eq!(
            labels
                .get(&0x0010)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e_0010")
        );
    }

    #[test]
    fn test_data_word_vs_address() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: DataWord ($2000) -> 00 20
        // $1002: Address ($1000) -> 00 10 (Internal)
        let data = vec![0x00, 0x20, 0x00, 0x10];
        state.raw_data = data;
        state.block_types = vec![
            BlockType::DataWord,
            BlockType::DataWord,
            BlockType::Address,
            BlockType::Address,
        ];

        // Re-analyze reference counts and labels
        let result = analyze(&state);
        let labels = result.labels;

        // DataWord at $1000 should NOT generate label for ITSELF ($1000)
        // BUT $1002 IS Reference to $1000. So $1000 SHOULD have a label now.
        // Address type usage at 1002 -> Absolute priority -> a1000
        assert_eq!(
            labels
                .get(&0x1000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("a_1000")
        );

        // And content of DataWord ($2000) should still be None (assuming it's external/ignored)
        assert_eq!(labels.get(&0x2000), None);
    }
    #[test]
    fn test_branch_offsets() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        // BNE instructions
        let data = vec![0xD0, 0x00, 0xD0, 0x7F, 0xD0, 0x80, 0xD0, 0xFF, 0xD0, 0xFE];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let result = analyze(&state);
        let labels = result.labels;

        // Case 1: $1000 -> jump to $1002. Usage: b1002 (Internal)
        assert_eq!(
            labels
                .get(&0x1002)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b_1002")
        );

        // Case 2: $1002 -> jump to $1083. Usage: e1083 (External logic applies to Branch too now)
        assert_eq!(
            labels
                .get(&0x1083)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e_1083")
        );

        // Case 3: $1004 -> jump to $0F86. Usage: e0F86
        assert_eq!(
            labels
                .get(&0x0F86)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e_0F86")
        );

        // Case 4: $1006 -> jump to $1007. Usage: b1007
        assert_eq!(
            labels
                .get(&0x1007)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b_1007")
        );

        // Case 5: $1008 -> jump to $1008. Usage: b1008
        assert_eq!(
            labels
                .get(&0x1008)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b_1008")
        );
    }

    #[test]
    fn test_new_pointer_field_types() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        // Indirect JMP (JMP ($1000)) -> 6C 00 10
        // LDA ($10, X) -> A1 10
        // LDA ($10), Y -> B1 10
        // LDA $1000, X -> BD 00 10
        // LDA $1000, Y -> B9 00 10
        // LDA $10, X -> B5 10
        let data = vec![
            0x6C, 0x00, 0x10, // JMP ($1000) -> Indirect -> p1000
            0xA1, 0x10, // LDA ($10, X) -> Indirect X -> p10
            0xB1, 0x20, // LDA ($20), Y -> Indirect Y -> p20
            0xBD, 0x50, 0x10, // LDA $1050, X -> Absolute X -> f1050
            0xB9, 0x60, 0x10, // LDA $1060, Y -> Absolute Y -> f1060
            0xB5, 0x30, // LDA $30, X -> ZeroPage X -> f30
        ];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let result = analyze(&state);
        let labels = result.labels;

        // Indirect JMP -> p1000
        assert_eq!(
            labels
                .get(&0x1000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("p_1000")
        );

        // Indirect X -> p10
        assert_eq!(
            labels
                .get(&0x0010)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("zpp_10")
        );

        // Indirect Y -> p20
        assert_eq!(
            labels
                .get(&0x0020)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("zpp_20")
        );

        // Absolute X -> f1050
        assert_eq!(
            labels
                .get(&0x1050)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("f_1050")
        );

        // Absolute Y -> f1060
        assert_eq!(
            labels
                .get(&0x1060)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("f_1060")
        );

        // ZeroPage X -> f30
        assert_eq!(
            labels
                .get(&0x0030)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("zpf_30")
        );
    }

    #[test]
    fn test_internal_label_priority() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        // Scenario: Internal address $1005 accessed via:
        // 1. Branch (BNE $1005) -> D0 03 (assuming PC is 1000 + 2 = 1002. 1002+3=1005)
        // 2. Jump (JMP $1005) -> 4C 05 10
        // 3. Subroutine (JSR $1005) -> 20 05 10
        //
        // Priority should be Subroutine ("s") > Jump ("j") > Branch ("b").
        // Result should be b1005 (First usage wins).

        // $1000: BNE $1005 (D0 03)
        // $1002: JMP $1005 (4C 05 10)
        // $1005: JSR $1005 (20 05 10) (recursive call just for usage)

        let data = vec![
            0xD0, 0x03, // 1000: BNE +3 -> 1005
            0x4C, 0x05, 0x10, // 1002: JMP $1005
            0x20, 0x05, 0x10, // 1005: JSR $1005
        ];

        state.raw_data = data;
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let result = analyze(&state);
        let labels = result.labels;

        // Check 1005
        // Expect 'b' because it was referenced by Branch FIRST.
        assert_eq!(
            labels
                .get(&0x1005)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b_1005")
        );

        // Also verify usage map contains all types?
        // We can't easily check usage map here as it's internal to analyze, but we check the result name.
    }

    #[test]
    fn test_internal_label_first_wins() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);

        // BEQ $1005 (D0 03) -> Branch type
        // JMP $1005 (4C 05 10) -> Jump type
        // Internal address $1005.
        // BEQ appears first, so it should define the label as 'b1005'.
        let data = vec![
            0xD0, 0x03, // 1000: BEQ $1005
            0x4C, 0x05, 0x10, // 1002: JMP $1005
            0xEA, // 1005: NOP (make it valid internal address)
        ];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let result = analyze(&state);
        let labels = result.labels;

        // Expectation: b1005 (Branch) because it was first.
        // (Before fix, this would likely be j1005 because Jump > Branch in priority)
        assert_eq!(
            labels
                .get(&0x1005)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b_1005")
        );
    }

    #[test]
    fn test_external_label_still_prioritized() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        let len = 100;
        state.raw_data = vec![0; len]; // Just dummy data placeholder
        state.block_types = vec![BlockType::Code; len];

        // We construct a specific scenario:
        // LDA $E000 (AD 00 E0) -> Absolute usage
        // JMP $E000 (4C 00 E0) -> Jump usage
        // Address $E000 is external.
        // LDA appears first.
        // WE apply "first wins" to external.
        // LDA appears first, so we expect aE000.
        // (Previously it was eE000 because ExternalJump > Absolute priority).

        let data = [
            0xAD, 0x00, 0xE0, // 1000: LDA $E000
            0x4C, 0x00, 0xE0, // 1003: JMP $E000
        ];
        // replace beginning of dummy data
        for (i, b) in data.iter().enumerate() {
            state.raw_data[i] = *b;
        }

        let result = analyze(&state);
        let labels = result.labels;

        // Result should be aE000
        assert_eq!(
            labels
                .get(&0xE000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("a_E000")
        );
    }

    #[test]
    fn test_excluded_addresses() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // JMP $E500 (4C 00 E5) -> normally eE500
        // But we exclude E500.
        let data = vec![0x4C, 0x00, 0xE5];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 3];

        state.excluded_addresses.insert(Addr(0xE500));

        let result = analyze(&state);
        let labels = result.labels;

        // Should be None
        assert_eq!(labels.get(&0xE500), None);

        // Verification: if we remove it from excludes, it should appear
        state.excluded_addresses.remove(&Addr(0xE500));
        let result = analyze(&state);
        let labels = result.labels;
        assert_eq!(
            labels
                .get(&0xE500)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e_E500")
        );
    }

    #[test]
    fn test_external_branch() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // BNE target that is out of range
        // Origin 1000. Data len 2. Range 1000..1002.
        // 1000: D0 7F (BNE +$7F) -> Target: 1002 + 7F = 1081.
        // 1081 is >= origin+len (1002), so it is external.

        let data = vec![0xD0, 0x7F];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 2];

        let result = analyze(&state);
        let labels = result.labels;
        // Should be e1081 (ExternalJump type), NOT b1081 (Branch type).
        assert_eq!(
            labels
                .get(&0x1081)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e_1081")
        );
    }

    #[test]
    fn test_absolute_addressing_on_zp_formatting() {
        let mut app_state = AppState::new();
        // 8D A0 00 -> STA $00A0 (Answer to Life, Universe, and Everything)
        // Absolute addressing mode targeting a ZP address.
        app_state.raw_data = vec![0x8D, 0xA0, 0x00];
        app_state.origin = Addr(0x1000);
        app_state.block_types = vec![BlockType::Code; 3];
        // Fill opcodes
        app_state.disassembler.opcodes[0x8D] = Some(crate::cpu::Opcode {
            mnemonic: "STA",
            mode: AddressingMode::Absolute,
            size: 3,
            cycles: 4,
            description: "Store Accumulator",
            illegal: false,
        });

        let result = analyze(&app_state);
        let labels_map = result.labels;
        let labels = labels_map
            .get(&0x00A0)
            .expect("Should have a label at $00A0");
        let label = labels.first().expect("Label vector should not be empty");

        // User wants "a_00A0" because it was forced absolute / accessed absolutely.
        // Current bug: "aA0"
        assert_eq!(label.name, "a_00A0");
    }

    #[test]
    fn test_field_formatting_on_zp() {
        let mut app_state = AppState::new();
        // 9D A0 00 -> STA $00A0, X (Absolute, X)
        // Absolute indexed addressing mode targeting a ZP address.
        // Should generate "f_00A0" (Field, 4 digits) NOT "fA0" (ZP Field, 2 digits).
        app_state.raw_data = vec![0x9D, 0xA0, 0x00];
        app_state.origin = Addr(0x1000);
        app_state.block_types = vec![BlockType::Code; 3];
        // Fill opcodes
        app_state.disassembler.opcodes[0x9D] = Some(crate::cpu::Opcode {
            mnemonic: "STA",
            mode: AddressingMode::AbsoluteX,
            size: 3,
            cycles: 5,
            description: "Store Accumulator",
            illegal: false,
        });

        let result = analyze(&app_state);
        let labels_map = result.labels;
        let labels = labels_map
            .get(&0x00A0)
            .expect("Should have a label at $00A0");
        let label = labels.first().expect("Label vector should not be empty");

        assert_eq!(
            label.label_type,
            LabelType::Field,
            "Should be Field type (Absolute Indexed)"
        );
        assert_eq!(label.name, "f_00A0");
    }

    #[test]
    fn test_external_pointer_first_wins() {
        let mut app_state = AppState::new();
        // Scenario: Two instructions using address $00FB.
        // 1. JMP ($00FB) -> 6C FB 00  (Indirect) -> LabelType::Pointer
        // 2. LDA ($FB), Y -> B1 FB    (IndirectY) -> LabelType::ZeroPagePointer
        //
        // First-wins: JMP comes first, so the label should be "p_00FB".

        app_state.origin = Addr(0x1000);
        let data = vec![
            0x6C, 0xFB, 0x00, // JMP ($00FB)
            0xB1, 0xFB, // LDA ($FB), Y
        ];
        app_state.raw_data = data;
        app_state.block_types = vec![BlockType::Code; 5];

        // JMP Indirect
        app_state.disassembler.opcodes[0x6C] = Some(crate::cpu::Opcode {
            mnemonic: "JMP",
            mode: AddressingMode::Indirect,
            size: 3,
            cycles: 5,
            description: "Jump Indirect",
            illegal: false,
        });
        // LDA IndirectY
        app_state.disassembler.opcodes[0xB1] = Some(crate::cpu::Opcode {
            mnemonic: "LDA",
            mode: AddressingMode::IndirectY,
            size: 2,
            cycles: 5,
            description: "Load Accumulator Indirect Y",
            illegal: false,
        });

        let result = analyze(&app_state);
        let labels_map = result.labels;
        let label_vec = labels_map
            .get(&0x00FB)
            .expect("Should have a label at $00FB");

        assert_eq!(label_vec.len(), 1, "Should have ONE label for $00FB");
        assert_eq!(label_vec[0].name, "p_00FB");
        assert_eq!(label_vec[0].label_type, LabelType::Pointer);
    }

    #[test]
    fn test_hilo_analysis() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // HiLo Block: 4 bytes
        // Hi: $C0, $D0
        // Lo: $00, $01
        // Addresses: $C000, $D001 (External)
        let data = vec![0xC0, 0xD0, 0x00, 0x01];
        state.raw_data = data;
        state.block_types = vec![BlockType::HiLoAddress; 4];

        let result = analyze(&state);
        let labels = result.labels;

        // Check $C000 -> aC000 (AbsoluteAddress usage from HiLo)
        assert_eq!(
            labels
                .get(&0xC000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("a_C000")
        );

        // Check $D001 -> aD001
        assert_eq!(
            labels
                .get(&0xD001)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("a_D001")
        );
    }

    #[test]
    fn test_analyze_lohi_address_with_splitter() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // Two blocks of LoHiAddress, 6 bytes each.
        // Block 1: $1000-$1005 -> addresses $1954, $19A1, $19DA
        // Block 2: $1006-$100B -> addresses $1951, $199E, $19D7
        let data = vec![
            0x54, 0xA1, 0xDA, 0x19, 0x19, 0x19, // Block 1
            0x51, 0x9E, 0xD7, 0x19, 0x19, 0x19, // Block 2
        ];
        state.raw_data = data;
        state.block_types = vec![BlockType::LoHiAddress; 12];

        // Add splitter at $1006 (start of block 2)
        state.toggle_splitter(Addr(0x1006));

        let result = analyze(&state);
        let labels = result.labels;

        // Verify labels generated for Block 1 targets
        assert!(labels.contains_key(&0x1954));
        assert!(labels.contains_key(&0x19A1));
        assert!(labels.contains_key(&0x19DA));

        // Verify labels generated for Block 2 targets
        assert!(labels.contains_key(&0x1951));
        assert!(labels.contains_key(&0x199E));
        assert!(labels.contains_key(&0x19D7));

        // Verify NO labels generated for the wrong pairings (e.g., $5154 from data[0]+data[6])
        assert!(!labels.contains_key(&0x5154));
    }

    // -----------------------------------------------------------------------
    // Tests for the `r_` Return label promotion
    // -----------------------------------------------------------------------

    #[test]
    fn test_branch_to_rts_becomes_return_label() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: BEQ +0 (F0 00) → target $1002
        // $1002: RTS (60)
        let data = vec![0xF0, 0x00, 0x60];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 3];

        let result = analyze(&state);
        assert_eq!(
            result
                .labels
                .get(&0x1002)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("r_1002"),
            "Branch to RTS must get the r_ prefix"
        );
    }

    #[test]
    fn test_jmp_to_rts_becomes_return_label() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: JMP $1003 (4C 03 10)
        // $1003: RTS (60)
        let data = vec![0x4C, 0x03, 0x10, 0x60];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 4];

        let result = analyze(&state);
        assert_eq!(
            result
                .labels
                .get(&0x1003)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("r_1003"),
            "JMP to RTS must get the r_ prefix"
        );
    }

    #[test]
    fn test_jsr_to_rts_becomes_return_label() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: JSR $1003 (20 03 10)
        // $1003: RTS (60)
        let data = vec![0x20, 0x03, 0x10, 0x60];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 4];

        let result = analyze(&state);
        assert_eq!(
            result
                .labels
                .get(&0x1003)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("r_1003"),
            "JSR to RTS must get the r_ prefix"
        );
    }

    #[test]
    fn test_branch_to_rti_becomes_return_label() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: BNE +0 (D0 00) → target $1002
        // $1002: RTI (40)
        let data = vec![0xD0, 0x00, 0x40];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 3];

        let result = analyze(&state);
        assert_eq!(
            result
                .labels
                .get(&0x1002)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("r_1002"),
            "Branch to RTI must get the r_ prefix"
        );
    }

    #[test]
    fn test_branch_to_nop_keeps_branch_label() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: BEQ +0 (F0 00) → target $1002
        // $1002: NOP (EA)
        let data = vec![0xF0, 0x00, 0xEA];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 3];

        let result = analyze(&state);
        assert_eq!(
            result
                .labels
                .get(&0x1002)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b_1002"),
            "Branch to NOP must keep the b_ prefix"
        );
    }

    #[test]
    fn test_external_rts_target_stays_external_jump() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // JMP $2000 (external)
        let data = vec![0x4C, 0x00, 0x20];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 3];

        let result = analyze(&state);
        // External addresses cannot be inspected; must remain e_
        assert_eq!(
            result
                .labels
                .get(&0x2000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e_2000"),
            "External target must stay as e_ even if it happens to start with RTS"
        );
    }

    #[test]
    fn test_return_label_format() {
        assert_eq!(
            LabelType::Return.format_label(0xAACC),
            "r_AACC",
            "Return label outside ZP must use 4 hex digits"
        );
        assert_eq!(
            LabelType::Return.format_label(0x10),
            "r_10",
            "Return label in ZP must use 2 hex digits"
        );
    }

    #[test]
    fn test_analyze_typed_xrefs_read_write_rmw_call_jump() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: LDA $2000 (AD 00 20) -> Read
        // $1003: STA $2000 (8D 00 20) -> Write
        // $1006: INC $2000 (EE 00 20) -> ReadModifyWrite
        // $1009: JSR $1012 (20 12 10) -> Call
        // $100C: JMP $1012 (4C 12 10) -> Jump
        // $100F: BNE $1012 (D0 01)    -> Jump
        // $1011: NOP       (EA)
        // $1012: RTS       (60)
        state.raw_data = vec![
            0xAD, 0x00, 0x20, // $1000
            0x8D, 0x00, 0x20, // $1003
            0xEE, 0x00, 0x20, // $1006
            0x20, 0x12, 0x10, // $1009
            0x4C, 0x12, 0x10, // $100C
            0xD0, 0x01, // $100F
            0xEA, // $1011
            0x60, // $1012
        ];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let result = analyze(&state);
        assert_eq!(
            result.cross_refs.get(&Addr(0x2000)),
            Some(&vec![
                XRef::new(Addr(0x1000), XRefKind::Read),
                XRef::new(Addr(0x1003), XRefKind::Write),
                XRef::new(Addr(0x1006), XRefKind::ReadModifyWrite),
            ])
        );
        assert_eq!(
            result.cross_refs.get(&Addr(0x1012)),
            Some(&vec![
                XRef::new(Addr(0x1009), XRefKind::Call),
                XRef::new(Addr(0x100C), XRefKind::Jump),
                XRef::new(Addr(0x100F), XRefKind::Jump),
            ])
        );
    }

    #[test]
    fn test_analyze_typed_xrefs_illegal_opcodes_and_indirect_zp() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.use_illegal_opcodes = true;
        // $1000: LAX ($FB),Y (B3 FB) -> Read
        // $1002: STA ($FB),Y (91 FB) -> Write
        // $1004: SAX $FB     (87 FB) -> Write
        // $1006: SLO ($FB),Y (13 FB) -> ReadModifyWrite
        // $1008: RLA $FB     (27 FB) -> ReadModifyWrite
        // $100A: SRE $FB     (47 FB) -> ReadModifyWrite
        // $100C: RRA $FB     (67 FB) -> ReadModifyWrite
        // $100E: DCP $FB     (C7 FB) -> ReadModifyWrite
        // $1010: ISC $FB     (E7 FB) -> ReadModifyWrite
        state.raw_data = vec![
            0xB3, 0xFB, // $1000
            0x91, 0xFB, // $1002
            0x87, 0xFB, // $1004
            0x13, 0xFB, // $1006
            0x27, 0xFB, // $1008
            0x47, 0xFB, // $100A
            0x67, 0xFB, // $100C
            0xC7, 0xFB, // $100E
            0xE7, 0xFB, // $1010
        ];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let result = analyze(&state);
        assert_eq!(
            result.cross_refs.get(&Addr(0x00FB)),
            Some(&vec![
                XRef::new(Addr(0x1000), XRefKind::Read),
                XRef::new(Addr(0x1002), XRefKind::Write),
                XRef::new(Addr(0x1004), XRefKind::Write),
                XRef::new(Addr(0x1006), XRefKind::ReadModifyWrite),
                XRef::new(Addr(0x1008), XRefKind::ReadModifyWrite),
                XRef::new(Addr(0x100A), XRefKind::ReadModifyWrite),
                XRef::new(Addr(0x100C), XRefKind::ReadModifyWrite),
                XRef::new(Addr(0x100E), XRefKind::ReadModifyWrite),
                XRef::new(Addr(0x1010), XRefKind::ReadModifyWrite),
            ])
        );
    }

    #[test]
    fn test_analyze_typed_xrefs_immediate_low_high_byte_is_offset() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: LDA #$34 (A9 34) with LowByte($1234)
        // $1002: LDY #$12 (A0 12) with HighByte($1234)
        state.raw_data = vec![0xA9, 0x34, 0xA0, 0x12];
        state.block_types = vec![BlockType::Code; 4];
        state.annotations.update(Addr(0x1000), |e| {
            e.immediate_format = Some(crate::state::ImmediateFormat::LowByte(Addr(0x1234)));
        });
        state.annotations.update(Addr(0x1002), |e| {
            e.immediate_format = Some(crate::state::ImmediateFormat::HighByte(Addr(0x1234)));
        });

        let result = analyze(&state);
        assert_eq!(
            result.cross_refs.get(&Addr(0x1234)),
            Some(&vec![
                XRef::new(Addr(0x1000), XRefKind::Offset),
                XRef::new(Addr(0x1002), XRefKind::Offset),
            ])
        );
    }

    #[test]
    fn test_analyze_typed_xrefs_jmp_indirect_pointer_and_resolved_target() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: JMP ($1006) (6C 06 10)
        // $1003..$1005: NOP (EA EA EA)
        // $1006..=$1007: .word $2000 (00 20) as BlockType::Address
        state.raw_data = vec![0x6C, 0x06, 0x10, 0xEA, 0xEA, 0xEA, 0x00, 0x20];
        state.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Address,
            BlockType::Address,
        ];

        let result = analyze(&state);
        assert_eq!(
            result.cross_refs.get(&Addr(0x1006)),
            Some(&vec![XRef::new(Addr(0x1000), XRefKind::Jump)])
        );
        assert_eq!(
            result.cross_refs.get(&Addr(0x2000)),
            Some(&vec![
                XRef::new(Addr(0x1000), XRefKind::Jump),
                XRef::new(Addr(0x1006), XRefKind::Offset),
            ])
        );
    }

    #[test]
    fn test_analyze_typed_xrefs_data_blocks_address_lohi_hilo() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..=$1001: BlockType::Address -> $3000 (00 30)
        // $1002..=$1005: BlockType::LoHiAddress -> $3001 (lo 01 at $1002, hi 30 at $1004), $3002 (lo 02 at $1003, hi 30 at $1005)
        // $1006..=$1009: BlockType::HiLoAddress -> $3003 (hi 30 at $1006, lo 03 at $1008), $3004 (hi 30 at $1007, lo 04 at $1009)
        state.raw_data = vec![
            0x00, 0x30, // Address $3000
            0x01, 0x02, 0x30, 0x30, // LoHiAddress $3001, $3002
            0x30, 0x30, 0x03, 0x04, // HiLoAddress $3003, $3004
        ];
        state.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
            BlockType::HiLoAddress,
        ];

        let result = analyze(&state);
        assert_eq!(
            result.cross_refs.get(&Addr(0x3000)),
            Some(&vec![XRef::new(Addr(0x1000), XRefKind::Offset)])
        );
        assert_eq!(
            result.cross_refs.get(&Addr(0x3001)),
            Some(&vec![XRef::new(Addr(0x1002), XRefKind::Offset)])
        );
        assert_eq!(
            result.cross_refs.get(&Addr(0x3002)),
            Some(&vec![XRef::new(Addr(0x1003), XRefKind::Offset)])
        );
        assert_eq!(
            result.cross_refs.get(&Addr(0x3003)),
            Some(&vec![XRef::new(Addr(0x1006), XRefKind::Offset)])
        );
        assert_eq!(
            result.cross_refs.get(&Addr(0x3004)),
            Some(&vec![XRef::new(Addr(0x1007), XRefKind::Offset)])
        );
    }

    #[test]
    fn test_analyze_typed_xrefs_sorted_and_deduplicated() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: LDA $2000 (AD 00 20) -> Read from $1000 AND LowByte($2000) annotation at $1000 -> Offset from $1000
        // $1003: STA $2000 (8D 00 20) -> Write from $1003
        state.raw_data = vec![0xAD, 0x00, 0x20, 0x8D, 0x00, 0x20];
        state.block_types = vec![BlockType::Code; 6];
        state.annotations.update(Addr(0x1000), |e| {
            e.immediate_format = Some(crate::state::ImmediateFormat::LowByte(Addr(0x2000)));
        });

        let result = analyze(&state);
        assert_eq!(
            result.cross_refs.get(&Addr(0x2000)),
            Some(&vec![
                XRef::new(Addr(0x1000), XRefKind::Read),
                XRef::new(Addr(0x1000), XRefKind::Offset),
                XRef::new(Addr(0x1003), XRefKind::Write),
            ])
        );
    }

    #[test]
    fn test_flow_analyze_traces_calls_branches_and_stops_at_rts_or_illegal() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..$1003: JSR $100A (20 0A 10)
        // $1003..$1005: BNE $1008 (D0 03)
        // $1005..$1008: JMP $100C (4C 0C 10)
        // $1008..$100A: LAX $10 (A7 10) -> illegal opcode, stops when disabled
        // $100A..$100B: NOP (EA)
        // $100B..$100C: RTS (60)
        // $100C..$100D: RTI (40)
        state.raw_data = vec![
            0x20, 0x0A, 0x10, // 0..3: JSR $100A
            0xD0, 0x03, // 3..5: BNE $1008
            0x4C, 0x0C, 0x10, // 5..8: JMP $100C
            0xA7, 0x10, // 8..10: LAX $10 (illegal)
            0xEA, // 10..11: NOP
            0x60, // 11..12: RTS
            0x40, // 12..13: RTI
        ];
        state.block_types = vec![BlockType::DataByte; state.raw_data.len()];
        state.settings.use_illegal_opcodes = false;

        let ranges = flow_analyze(&state, Addr(0x1000));
        // Expected traced code ranges:
        // - 0..8 (JSR, BNE, JMP)
        // - 10..12 (NOP, RTS at $100A)
        // - 8..8 (stopped immediately at illegal LAX when disabled -> not pushed)
        // - 12..13 (RTI at $100C)
        assert_eq!(ranges, vec![0..8, 10..12, 12..13]);

        // Enabling illegal opcodes allows $1008..$100A (LAX $10) to be traced as code
        state.settings.use_illegal_opcodes = true;
        let ranges_with_illegal = flow_analyze(&state, Addr(0x1000));
        assert_eq!(ranges_with_illegal, vec![0..8, 10..12, 8..10, 12..13]);
    }

    #[test]
    fn test_analyze_two_byte_brk_and_partial_blocks() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..$1002: BRK + signature $EA (00 EA)
        // $1002: 1-byte Address block (0x34) followed by Code at $1003: LDA $2000 (AD 00 20)
        state.raw_data = vec![0x00, 0xEA, 0x34, 0xAD, 0x00, 0x20];
        state.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Address,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
        ];
        state.settings.brk_single_byte = false;

        let result = analyze(&state);
        // The 1-byte partial Address block at $1002 must not consume $1003 (`0xAD`),
        // so `LDA $2000` at $1003 is still analyzed cleanly.
        assert_eq!(
            result.cross_refs.get(&Addr(0x2000)),
            Some(&vec![XRef::new(Addr(0x1003), XRefKind::Read)])
        );
    }

    #[test]
    fn test_analyze_jmp_indirect_promotes_address_to_jump_without_duplicate() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000..$1002: Address word pointing to $1005 (05 10) -> processed first!
        // $1002..$1005: JMP ($1000) (6C 00 10)
        // $1005: NOP (EA)
        state.raw_data = vec![0x05, 0x10, 0x6C, 0x00, 0x10, 0xEA];
        state.block_types = vec![
            BlockType::Address,
            BlockType::Address,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
        ];

        let result = analyze(&state);
        let labels_at_1005 = result
            .labels
            .get(&0x1005)
            .expect("Target $1005 must have a label");
        assert_eq!(
            labels_at_1005.len(),
            1,
            "Target $1005 must not have duplicate Auto labels"
        );
        assert_eq!(labels_at_1005[0].name, "j_1005");
        assert_eq!(labels_at_1005[0].label_type, LabelType::Jump);
    }

    #[test]
    fn test_analyze_symbolic_offset_zp_pointer_reclaims_orphan_and_routes_xrefs() {
        use crate::state::{OffsetRadix, SymbolicOffset};

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: LDA #$00 (A9 00)
        // $1002: STA $FB  (85 FB)
        // $1004: STA $FC  (85 FC)
        // $1006: LDA ($FB),Y (B1 FB)
        // $1008: INC $FC  (E6 FC)
        state.raw_data = vec![0xA9, 0x00, 0x85, 0xFB, 0x85, 0xFC, 0xB1, 0xFB, 0xE6, 0xFC];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.annotations.update(Addr(0x00FC), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });

        let result = analyze(&state);

        assert!(
            !result.labels.contains_key(&Addr(0x00FC)),
            "Orphan auto-label zpa_FC at $00FC must be reclaimed"
        );
        assert!(
            !result.cross_refs.contains_key(&Addr(0x00FC)),
            "$00FC must not retain separate cross-references"
        );
        assert_eq!(
            result.cross_refs.get(&Addr(0x00FB)),
            Some(&vec![
                XRef::new(Addr(0x1002), XRefKind::Write),
                XRef::new(Addr(0x1004), XRefKind::Write),
                XRef::new(Addr(0x1006), XRefKind::Read),
                XRef::new(Addr(0x1008), XRefKind::ReadModifyWrite),
            ])
        );
    }

    #[test]
    fn test_analyze_symbolic_offset_all_four_sites_and_single_xref_per_split_pair() {
        use crate::state::{OffsetRadix, SymbolicOffset};

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // Layout:
        // $1000..=$1001: BlockType::Address -> $1017 (17 10)
        // $1002..=$1003: LDA #$17 (A9 17) -> LowByte($1017)
        // $1004..=$1005: LDY #$10 (A0 10) -> HighByte($1017)
        // $1006..=$1009: BlockType::LoHiAddress (2 pairs: $1017, $1017 -> 17 17 10 10)
        // $100A..=$100B: BlockType::HiLoAddress (1 pair:  $1017 -> 10 17)
        // $100C..=$100E: LDA $1017,X (BD 17 10)
        // $100F..=$1011: JMP ($1012) (6C 12 10) with Tier 1 base: $1014, offset: -2
        // $1012..=$1013: BlockType::Address vector -> $101A (1A 10)
        // $1014..=$1016: NOP; NOP; NOP (EA EA EA)
        // $1017:         RTS (60) — preceding routine's RTS!
        // $1018..=$1019: NOP; RTS (EA 60) — target subroutine!
        // $101A:         NOP (EA) — indirect jump destination!
        state.raw_data = vec![
            0x17, 0x10, // $1000..=$1001
            0xA9, 0x17, // $1002..=$1003
            0xA0, 0x10, // $1004..=$1005
            0x17, 0x17, 0x10, 0x10, // $1006..=$1009
            0x10, 0x17, // $100A..=$100B
            0xBD, 0x17, 0x10, // $100C..=$100E
            0x6C, 0x12, 0x10, // $100F..=$1011
            0x1A, 0x10, // $1012..=$1013
            0xEA, 0xEA, 0xEA, // $1014..=$1016
            0x60, // $1017
            0xEA, 0x60, // $1018..=$1019
            0xEA, // $101A
        ];
        state.block_types = vec![
            BlockType::Address,
            BlockType::Address, // $1000..=$1001
            BlockType::Code,
            BlockType::Code, // $1002..=$1003
            BlockType::Code,
            BlockType::Code, // $1004..=$1005
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress, // $1006..=$1009
            BlockType::HiLoAddress,
            BlockType::HiLoAddress, // $100A..=$100B
            BlockType::Code,
            BlockType::Code,
            BlockType::Code, // $100C..=$100E
            BlockType::Code,
            BlockType::Code,
            BlockType::Code, // $100F..=$1011
            BlockType::Address,
            BlockType::Address, // $1012..=$1013
            BlockType::Code,
            BlockType::Code,
            BlockType::Code, // $1014..=$1016
            BlockType::Code, // $1017
            BlockType::Code,
            BlockType::Code, // $1018..=$1019
            BlockType::Code, // $101A
        ];

        let rts_sym = SymbolicOffset {
            base: Addr(0x1018),
            offset: -1,
            radix: OffsetRadix::Decimal,
        };
        // Site 1: inline BlockType::Address at $1000
        state
            .annotations
            .update(Addr(0x1000), |e| e.operand_offset = Some(rts_sym));
        // Site 2: LowByte and HighByte at $1002 and $1004
        state.annotations.update(Addr(0x1002), |e| {
            e.immediate_format = Some(ImmediateFormat::LowByte(Addr(0x1017)));
            e.operand_offset = Some(rts_sym);
        });
        state.annotations.update(Addr(0x1004), |e| {
            e.immediate_format = Some(ImmediateFormat::HighByte(Addr(0x1017)));
            e.operand_offset = Some(rts_sym);
        });
        // Site 3: LoHiAddress pair 0 on first-half ($1006), pair 1 on second-half ($1009), HiLoAddress on $100A
        state
            .annotations
            .update(Addr(0x1006), |e| e.operand_offset = Some(rts_sym));
        state
            .annotations
            .update(Addr(0x1009), |e| e.operand_offset = Some(rts_sym));
        state
            .annotations
            .update(Addr(0x100A), |e| e.operand_offset = Some(rts_sym));
        // Site 4: LDA $1017,X at $100C + JMP ($1012) at $100F
        state
            .annotations
            .update(Addr(0x100C), |e| e.operand_offset = Some(rts_sym));
        state.annotations.update(Addr(0x100F), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x1014),
                offset: -2,
                radix: OffsetRadix::Decimal,
            });
        });

        let result = analyze(&state);

        // 1. Zero auto-labels and zero cross-refs at preceding RTS $1017
        assert!(!result.labels.contains_key(&Addr(0x1017)));
        assert!(!result.cross_refs.contains_key(&Addr(0x1017)));

        // 2 & 3. All 7 routed XRefs on $1018, with strictly ONE XRef per split-table pair (Landmine 2)
        let refs_1018 = result
            .cross_refs
            .get(&Addr(0x1018))
            .expect("$1018 must receive routed XRefs");
        assert_eq!(
            refs_1018,
            &vec![
                XRef::new(Addr(0x1000), XRefKind::Offset),
                XRef::new(Addr(0x1002), XRefKind::Offset),
                XRef::new(Addr(0x1004), XRefKind::Offset),
                XRef::new(Addr(0x1006), XRefKind::Offset),
                XRef::new(Addr(0x1007), XRefKind::Offset),
                XRef::new(Addr(0x100A), XRefKind::Offset),
                XRef::new(Addr(0x100C), XRefKind::Read),
            ]
        );
        assert!(
            !refs_1018
                .iter()
                .any(|r| matches!(r.from.0, 0x1008 | 0x1009 | 0x100B)),
            "Split tables must never emit duplicate second-half XRefs"
        );

        // 4. JMP ($1012) routes Pointer XRef to $1014 while still dereferencing raw $1012 -> $101A
        assert_eq!(
            result.cross_refs.get(&Addr(0x1014)),
            Some(&vec![XRef::new(Addr(0x100F), XRefKind::Jump)])
        );
        assert_eq!(result.labels.get(&Addr(0x101A)).unwrap()[0].name, "j_101A");
        assert_eq!(
            result.cross_refs.get(&Addr(0x101A)),
            Some(&vec![
                XRef::new(Addr(0x100F), XRefKind::Jump),
                XRef::new(Addr(0x1012), XRefKind::Offset),
            ])
        );
    }

    #[test]
    fn test_analyze_label_type_normalization_only_when_symbolic_offset_active() {
        use crate::state::{OffsetRadix, SymbolicOffset};

        // Case A (Landmine 1): No symbolic offset -> 3-byte ZP operands preserve a_00A0, f_00A1, p_00FB
        let mut state_no_offset = AppState::new();
        state_no_offset.origin = Addr(0x1000);
        // STA $00A0 (8D A0 00), STA $00A1,X (9D A1 00), JMP ($00FB) (6C FB 00)
        state_no_offset.raw_data = vec![0x8D, 0xA0, 0x00, 0x9D, 0xA1, 0x00, 0x6C, 0xFB, 0x00];
        state_no_offset.block_types = vec![BlockType::Code; 9];

        let res_a = analyze(&state_no_offset);
        assert_eq!(res_a.labels.get(&Addr(0x00A0)).unwrap()[0].name, "a_00A0");
        assert_eq!(res_a.labels.get(&Addr(0x00A1)).unwrap()[0].name, "f_00A1");
        assert_eq!(res_a.labels.get(&Addr(0x00FB)).unwrap()[0].name, "p_00FB");

        // Case B: Active symbolic offsets on 3-byte and immediate sites normalize LabelType to base space.
        let mut state_with_offset = AppState::new();
        state_with_offset.origin = Addr(0x1000);
        // $1000: LDA $0100,X (BD 00 01) -> base $00FF (+1) produces zpf_FF.
        // $1003: LDA #$00    (A9 00)    -> LowByte($0100) with base $00FE (+2) produces zpp_FE.
        // $1005: LDA $00FF,X (BD FF 00) -> base $0100 (-1) produces f_0100.
        // $1008: LDA $00FD   (AD FD 00) -> base $0101 (-4) produces a_0101.
        // $100B: JMP ($00FC) (6C FC 00) -> base $0102 (-6) produces p_0102.
        state_with_offset.raw_data = vec![
            0xBD, 0x00, 0x01, // $1000
            0xA9, 0x00, // $1003
            0xBD, 0xFF, 0x00, // $1005
            0xAD, 0xFD, 0x00, // $1008
            0x6C, 0xFC, 0x00, // $100B
        ];
        state_with_offset.block_types = vec![BlockType::Code; 14];

        state_with_offset.annotations.update(Addr(0x1000), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x00FF),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        state_with_offset.annotations.update(Addr(0x1003), |e| {
            e.immediate_format = Some(ImmediateFormat::LowByte(Addr(0x0100)));
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x00FE),
                offset: 2,
                radix: OffsetRadix::Decimal,
            });
        });
        state_with_offset.annotations.update(Addr(0x1005), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
        state_with_offset.annotations.update(Addr(0x1008), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0101),
                offset: -4,
                radix: OffsetRadix::Decimal,
            });
        });
        state_with_offset.annotations.update(Addr(0x100B), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0102),
                offset: -6,
                radix: OffsetRadix::Decimal,
            });
        });

        let res_b = analyze(&state_with_offset);
        assert_eq!(res_b.labels.get(&Addr(0x00FF)).unwrap()[0].name, "zpf_FF");
        assert_eq!(res_b.labels.get(&Addr(0x00FE)).unwrap()[0].name, "zpp_FE");
        assert_eq!(res_b.labels.get(&Addr(0x0100)).unwrap()[0].name, "f_0100");
        assert_eq!(res_b.labels.get(&Addr(0x0101)).unwrap()[0].name, "a_0101");
        assert_eq!(res_b.labels.get(&Addr(0x0102)).unwrap()[0].name, "p_0102");

        // Case C: 2-byte zero-page instructions ignore out-of-bounds base > $00FF offsets.
        let mut state_zp_oob = AppState::new();
        state_zp_oob.origin = Addr(0x1000);
        state_zp_oob.raw_data = vec![
            0xB5, 0xFF, // $1000: LDA $FF,X
            0xA5, 0xFD, // $1002: LDA $FD
            0xB1, 0xFC, // $1004: LDA ($FC),Y
        ];
        state_zp_oob.block_types = vec![BlockType::Code; 6];
        state_zp_oob.annotations.update(Addr(0x1000), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
        state_zp_oob.annotations.update(Addr(0x1002), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0101),
                offset: -4,
                radix: OffsetRadix::Decimal,
            });
        });
        state_zp_oob.annotations.update(Addr(0x1004), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0102),
                offset: -6,
                radix: OffsetRadix::Decimal,
            });
        });

        let res_c = analyze(&state_zp_oob);
        assert_eq!(res_c.labels.get(&Addr(0x00FF)).unwrap()[0].name, "zpf_FF");
        assert_eq!(res_c.labels.get(&Addr(0x00FD)).unwrap()[0].name, "zpa_FD");
        assert_eq!(res_c.labels.get(&Addr(0x00FC)).unwrap()[0].name, "zpp_FC");
        assert!(!res_c.labels.contains_key(&Addr(0x0100)));
        assert!(!res_c.labels.contains_key(&Addr(0x0101)));
        assert!(!res_c.labels.contains_key(&Addr(0x0102)));
    }

    #[test]
    fn test_analyze_adjacent_lohi_tables_partitioned_by_user_comment_route_xrefs_without_bogus_labels()
     {
        use crate::state::{OffsetRadix, SymbolicOffset};

        // Memory layout ($1000..=$1027, 40 bytes):
        // $1000..=$1003: LoHiAddress Table 1 (2 pairs: $1017 = sub_a - 1, $101B = sub_b - 1)
        //   lo bytes ($1000, $1001): 0x17, 0x1B
        //   hi bytes ($1002, $1003): 0x10, 0x10
        // $1004..=$1007: LoHiAddress Table 2 separated ONLY by `user_line_comment` at $1004
        //   (2 pairs: $101F = sub_c - 1, $1023 = sub_d - 1)
        //   lo bytes ($1004, $1005): 0x1F, 0x23
        //   hi bytes ($1006, $1007): 0x10, 0x10
        // Note: Without sub-block splitting at $1004, a merged 8-byte table would mis-pair
        // ($1000, $1004) = $1F17, ($1001, $1005) = $231B, ($1002, $1006) = $1010, ($1003, $1007) = $1010!
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        let mut raw_data = vec![
            0x17, 0x1B, 0x10, 0x10, // $1000..=$1003 Table 1
            0x1F, 0x23, 0x10, 0x10, // $1004..=$1007 Table 2
        ];
        raw_data.extend([0xEA; 0x17 - 0x08]); // $1008..=$1016
        raw_data.push(0x60); // $1017 preceding RTS
        raw_data.extend([0xEA, 0xEA, 0xEA, 0x60]); // $1018..=$101B (`sub_a` at $1018, ends with RTS at $101B)
        raw_data.extend([0xEA, 0xEA, 0xEA, 0x60]); // $101C..=$101F (`sub_b` at $101C, ends with RTS at $101F)
        raw_data.extend([0xEA, 0xEA, 0xEA, 0x60]); // $1020..=$1023 (`sub_c` at $1020, ends with RTS at $1023)
        raw_data.extend([0xEA, 0xEA, 0xEA, 0x60]); // $1024..=$1027 (`sub_d` at $1024)
        assert_eq!(raw_data.len(), 0x28);

        let mut block_types = vec![BlockType::LoHiAddress; 8];
        block_types.extend([BlockType::Code; 0x20]);
        state.raw_data = raw_data;
        state.block_types = block_types;

        // Partition Table 1 and Table 2 SOLELY with a user line comment at $1004 (no splitter!).
        state.annotations.update(Addr(0x1004), |e| {
            e.user_line_comment = Some("Stage 2 RTS dispatch vectors".to_string());
        });

        for (site, base) in [
            (0x1000, 0x1018),
            (0x1001, 0x101C),
            (0x1004, 0x1020),
            (0x1005, 0x1024),
        ] {
            state.annotations.update(Addr(site), |e| {
                e.operand_offset = Some(SymbolicOffset {
                    base: Addr(base),
                    offset: -1,
                    radix: OffsetRadix::Decimal,
                });
            });
        }

        let res = analyze(&state);

        for (site, expected_base) in [
            (Addr(0x1000), Addr(0x1018)),
            (Addr(0x1001), Addr(0x101C)),
            (Addr(0x1004), Addr(0x1020)),
            (Addr(0x1005), Addr(0x1024)),
        ] {
            assert_eq!(
                res.cross_refs.get(&expected_base),
                Some(&vec![XRef::new(site, XRefKind::Offset)]),
                "Expected exact XRef routing {site:?} -> {expected_base:?}"
            );
            assert!(
                res.labels.contains_key(&expected_base),
                "Expected auto-label generated at {expected_base:?}"
            );
        }

        // Verify ZERO bogus auto-labels or XRefs anywhere else (neither at raw `-1` bytes nor at
        // mis-paired addresses `$1F17`, `$231B`, `$1010`).
        assert_eq!(
            res.labels.len(),
            4,
            "unexpected bogus labels: {:?}",
            res.labels
        );
        assert_eq!(
            res.cross_refs.len(),
            4,
            "unexpected bogus cross-refs: {:?}",
            res.cross_refs
        );
    }
}
