use crate::cpu::{Opcode, get_opcodes};
use crate::state::{Addr, Assembler, BlockType, DocumentSettings, Label};
use std::collections::{BTreeMap, BTreeSet};

pub mod context;
pub mod formatter;
pub mod formatter_64tass;
pub mod formatter_acme;
pub mod formatter_ca65;
pub mod formatter_kickasm;
pub mod handlers;

use crate::state::LabelKind;
use formatter::Formatter;
use formatter_64tass::TassFormatter;
use formatter_acme::AcmeFormatter;
use formatter_ca65::Ca65Formatter;
use formatter_kickasm::KickAsmFormatter;

pub const LABEL_COLUMN_WIDTH: usize = 20;

#[must_use]
pub fn resolve_label<'a>(
    labels: &'a [Label],
    _address: u16,
    _settings: &DocumentSettings,
) -> Option<&'a Label> {
    if labels.is_empty() {
        return None;
    }

    // Filter and Sort
    // We want to pick ONE label.
    // Precedence:
    // 1. User
    // 2. System
    // 3. Auto

    // Priority Score (Higher is better)
    let get_priority = |k: &LabelKind| -> u8 {
        match k {
            LabelKind::User => 100,
            LabelKind::System => 50,
            LabelKind::Auto => 0,
        }
    };

    let mut best_label: Option<&Label> = None;

    for label in labels {
        if let Some(curr) = best_label {
            // Priority Check
            let curr_prio = get_priority(&curr.kind);
            let new_prio = get_priority(&label.kind);

            if new_prio > curr_prio {
                best_label = Some(label);
            } else if new_prio == curr_prio {
                // Tie-break with name (stability)
                // We prefer alphabetically smaller names to be deterministic
                if label.name < curr.name {
                    best_label = Some(label);
                }
            }
        } else {
            best_label = Some(label);
        }
    }
    best_label
}

#[must_use]
pub fn resolve_label_name(
    address: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    settings: &DocumentSettings,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    label_routine_names: Option<&BTreeMap<Addr, String>>,
    current_routine_name: Option<&str>,
) -> Option<String> {
    // 1. Local label names (from formatter, e.g. _l01)
    if let Some(names) = local_label_names
        && let Some(name) = names.get(&address)
    {
        return Some(name.clone());
    }

    // 2. Standard label resolution
    let base_name = labels
        .get(&address)
        .and_then(|v| resolve_label(v, address.0, settings).map(|l| l.name.clone()))?;

    // 3. Routine scoping
    if let Some(routine_names) = label_routine_names
        && let Some(routine_name) = routine_names.get(&address)
    {
        let same_routine = current_routine_name.is_some_and(|curr| curr == routine_name);
        if !same_routine && &base_name != routine_name {
            return Some(format!("{}.{}", routine_name, base_name));
        }
    }

    Some(base_name)
}

pub use context::DisassemblyContext;
pub use context::HandleArgs;

#[derive(Debug, Clone)]
pub struct DisassemblyLine {
    pub address: Addr,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operand: String,
    pub comment: String,
    pub line_comment: Option<String>,
    #[allow(dead_code)]
    pub label: Option<String>,
    pub opcode: Option<Opcode>,
    pub show_bytes: bool,
    pub target_address: Option<Addr>,
    pub external_label_address: Option<Addr>,
    pub is_collapsed: bool,
}

impl DisassemblyLine {
    #[must_use]
    pub fn get_sub_index_for_address(
        &self,
        app_state: &crate::state::app_state::AppState,
        target_addr: u16,
    ) -> usize {
        // Calculate visual index for target_addr within this line.
        // Order:
        // 1. Labels [offset 1..N]
        // 2. Comments (not addressable by jump usually, but occupy sub-indices)
        // 3. Instruction (Base address)

        let mut sub_index = 0;

        // 1. Labels inside multi-byte instructions
        if self.bytes.len() > 1 {
            for offset in 1..self.bytes.len() {
                let mid_addr = self.address.wrapping_add(offset as u16);
                if let Some(l) = app_state.labels.get(&mid_addr) {
                    if mid_addr.0 == target_addr {
                        return sub_index;
                    }
                    sub_index += l.len();
                }
            }
        }

        // 2. Line comment
        if let Some(comment) = &self.line_comment {
            sub_index += comment.lines().count();
        }

        // 2.5. Long label on its own line
        if let Some(label) = &self.label
            && label.len() >= LABEL_COLUMN_WIDTH
        {
            sub_index += 1;
        }

        // 3. Instruction
        sub_index
    }
}

pub struct Disassembler {
    pub opcodes: [Option<Opcode>; 256],
}

impl Default for Disassembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Disassembler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            opcodes: get_opcodes(),
        }
    }

    #[must_use]
    pub fn create_formatter(assembler: Assembler) -> Box<dyn Formatter> {
        match assembler {
            Assembler::Tass64 => Box::new(TassFormatter),
            Assembler::Acme => Box::new(AcmeFormatter),
            Assembler::Ca65 => Box::new(Ca65Formatter),
            Assembler::Kick => Box::new(KickAsmFormatter),
        }
    }

    #[must_use]
    pub fn compute_local_label_names(
        &self,
        ctx: &DisassemblyContext,
        start_pc: usize,
        end_pc: usize,
        formatter: &dyn Formatter,
    ) -> BTreeMap<Addr, String> {
        let mut local_names = BTreeMap::new();
        let mut local_count = 0;

        let start_addr = ctx.origin.wrapping_add(start_pc as u16);
        let end_addr = ctx.origin.wrapping_add(end_pc as u16);

        // Iterate through all addresses in the routine block
        let mut current_pc = start_pc;
        while current_pc <= end_pc {
            // Check all addresses covered by this opcode (if any)
            let bytes_consumed = if let Some(opcode) = &self.opcodes[ctx.data[current_pc] as usize]
            {
                opcode.size as usize
            } else {
                1
            };

            for offset in 0..bytes_consumed {
                let check_pc = current_pc + offset;
                if check_pc > end_pc {
                    break;
                }
                let current_addr = ctx.origin.wrapping_add(check_pc as u16);

                // Entry point (first address of the block) is NOT local
                if check_pc == start_pc {
                    continue;
                }

                // Check if there are any labels at this address
                if let Some(_labels) = ctx.labels.get(&current_addr) {
                    // Rule: NOT local if referenced from outside
                    let mut referenced_from_outside = false;
                    if let Some(refs) = ctx.cross_refs.get(&current_addr) {
                        for &ref_addr in refs {
                            if ref_addr < start_addr || ref_addr > end_addr {
                                referenced_from_outside = true;
                                break;
                            }
                        }
                    }

                    if !referenced_from_outside {
                        // All labels at this address treated as a single local label
                        // and named using the formatter's local label name logic.
                        if let Some(name) = formatter.format_local_label(local_count) {
                            local_names.insert(current_addr, name);
                            local_count += 1;
                        }
                    }
                }
            }
            current_pc += bytes_consumed;
        }

        local_names
    }

    #[must_use]
    pub fn compute_routine_names(
        &self,
        ctx: &DisassemblyContext,
        _formatter: &dyn Formatter,
    ) -> BTreeMap<Addr, String> {
        let mut routine_names = BTreeMap::new();
        let mut pc = 0;
        while pc < ctx.data.len() {
            if pc < ctx.block_types.len() && ctx.block_types[pc] == BlockType::Routine {
                let start_pc = pc;
                let address = ctx.origin.wrapping_add(pc as u16);

                // Resolve label name for the routine entry point
                if let Some(v) = ctx.labels.get(&address)
                    && let Some(label) = resolve_label(v, address.0, ctx.settings)
                {
                    let name = label.name.clone();

                    // Find end of block (exclusive of next block type or splitter)
                    let mut end = pc;
                    while end + 1 < ctx.data.len()
                        && end + 1 < ctx.block_types.len()
                        && ctx.block_types[end + 1] == BlockType::Routine
                        && !ctx
                            .splitters
                            .contains(&ctx.origin.wrapping_add((end + 1) as u16))
                    {
                        end += 1;
                    }

                    // Map all addresses in this block to the routine name
                    for i in start_pc..=end {
                        let addr = ctx.origin.wrapping_add(i as u16);
                        routine_names.insert(addr, name.clone());
                    }
                    pc = end + 1;
                } else {
                    pc += 1;
                }
            } else {
                pc += 1;
            }
        }
        routine_names
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn disassemble(
        &self,
        data: &[u8],
        block_types: &[BlockType],
        labels: &BTreeMap<Addr, Vec<Label>>,
        origin: Addr,
        settings: &DocumentSettings,
        system_comments: &BTreeMap<Addr, String>,
        user_side_comments: &BTreeMap<Addr, String>,
        user_line_comments: &BTreeMap<Addr, String>,
        immediate_value_formats: &BTreeMap<Addr, crate::state::ImmediateFormat>,
        cross_refs: &BTreeMap<Addr, Vec<Addr>>,
        collapsed_blocks: &[(usize, usize)],
        splitters: &BTreeSet<Addr>,
    ) -> Vec<DisassemblyLine> {
        let ctx = DisassemblyContext {
            data,
            block_types,
            labels,
            origin,
            settings,
            system_comments,
            user_side_comments,
            user_line_comments,
            immediate_value_formats,
            cross_refs,
            collapsed_blocks,
            splitters,
        };
        self.disassemble_ctx(&ctx)
    }

    #[must_use]
    pub fn disassemble_ctx(&self, ctx: &DisassemblyContext) -> Vec<DisassemblyLine> {
        let formatter = Self::create_formatter(ctx.settings.assembler);

        let mut lines = Vec::new();
        let mut pc = 0;
        let mut current_local_names: Option<BTreeMap<Addr, String>> = None;
        let mut current_scope_end_pc: Option<usize> = None;
        let mut current_routine_name: Option<String> = None;

        // Compute routine names for scoping
        let supports_routines = formatter.supports_routines();
        let label_routine_names = if supports_routines {
            self.compute_routine_names(ctx, formatter.as_ref())
        } else {
            BTreeMap::new()
        };

        while pc < ctx.data.len() {
            let address = ctx.origin.wrapping_add(pc as u16);

            // Check if we just exited a Routine scope
            if let Some(end) = current_scope_end_pc
                && pc > end
            {
                if let Some(pend) = formatter.format_routine_end() {
                    lines.push(DisassemblyLine {
                        address: ctx.origin.wrapping_add(pc as u16),
                        bytes: vec![],
                        mnemonic: pend,
                        operand: String::new(),
                        comment: String::new(),
                        line_comment: None,
                        label: None,
                        opcode: None,
                        show_bytes: false,
                        target_address: None,
                        external_label_address: None,
                        is_collapsed: false,
                    });
                }
                current_scope_end_pc = None;
                current_local_names = None;
                current_routine_name = None;
            }

            // Render splitter if exists at this address
            if ctx.splitters.contains(&address) {
                lines.push(DisassemblyLine {
                    address,
                    bytes: vec![],
                    mnemonic: "{splitter}".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                });
            }

            // Flags for routine start logic
            let mut suppress_label = false;
            let mut suppress_line_comment = false;

            // Check if we are starting a NEW Routine scope
            if supports_routines
                && ctx.block_types.get(pc) == Some(&BlockType::Routine)
                && current_scope_end_pc.is_none()
            {
                // Entering a new Routine block range
                let mut end = pc;
                while end + 1 < ctx.data.len()
                    && ctx.block_types.get(end + 1) == Some(&BlockType::Routine)
                    && !ctx
                        .splitters
                        .contains(&ctx.origin.wrapping_add((end + 1) as u16))
                {
                    end += 1;
                }



                current_scope_end_pc = Some(end);
                current_local_names =
                    Some(self.compute_local_label_names(ctx, pc, end, formatter.as_ref()));

                // Emit routine start if formatter supports it
                let label_name = self
                    .get_label_name(address, ctx.labels, formatter.as_ref(), ctx.settings)
                    .unwrap_or_else(|| crate::state::LabelType::Subroutine.format_label(address.0));

                if let Some((label, mnemonic, operand)) =
                    formatter.format_routine_start(&label_name)
                {

                    current_routine_name = Some(label_name.clone());

                    let line_comment = ctx.user_line_comments.get(&address).cloned();

                    lines.push(DisassemblyLine {
                        address,
                        bytes: vec![],
                        mnemonic,
                        operand: operand.unwrap_or_default(),
                        comment: String::new(),
                        line_comment,
                        label,
                        opcode: None,
                        show_bytes: false,
                        target_address: None,
                        external_label_address: None,
                        is_collapsed: false,
                    });

                    // Requirement (b): hide the label in the instruction
                    suppress_label = true;
                    suppress_line_comment = true;
                }
            }

            // Check for collapsed block
            if let Some((_start, end)) = ctx.collapsed_blocks.iter().find(|(s, _)| *s == pc) {
                let start_addr = ctx.origin.wrapping_add(pc as u16);
                let end_addr = ctx.origin.wrapping_add(*end as u16);
                let block_type = ctx.block_types.get(pc).unwrap_or(&BlockType::Code);

                lines.push(DisassemblyLine {
                    address: start_addr,
                    bytes: vec![], // No bytes shown
                    mnemonic: format!(
                        "{} Collapsed {} block from ${:04X}-${:04X}",
                        formatter.comment_prefix(),
                        block_type,
                        start_addr,
                        end_addr
                    ),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: true,
                });

                pc = *end + 1;
                continue;
            }

            let address = ctx.origin.wrapping_add(pc as u16);

            // Resolve label name (checking local names first if in a scope)
            let label_name = if suppress_label {
                None
            } else if let Some(local_names) = &current_local_names {
                if let Some(local_name) = local_names.get(&address) {
                    Some(local_name.clone())
                } else {
                    self.get_label_name(address, ctx.labels, formatter.as_ref(), ctx.settings)
                }
            } else {
                self.get_label_name(address, ctx.labels, formatter.as_ref(), ctx.settings)
            };

            let side_comment = self.get_side_comment(address, ctx, formatter.comment_prefix());
            let line_comment = if suppress_line_comment {
                None
            } else {
                ctx.user_line_comments.get(&address).cloned()
            };

            let current_type = ctx.block_types.get(pc).copied().unwrap_or(BlockType::Code);

            let args = HandleArgs {
                pc,
                address,
                formatter: formatter.as_ref(),
                label_name,
                side_comment,
                line_comment,
                local_label_names: current_local_names.as_ref(),
                label_routine_names: Some(&label_routine_names),
                current_routine_name: current_routine_name.clone(),
            };

            let (bytes_consumed, new_lines) = match current_type {
                BlockType::Code | BlockType::Routine => self.handle_code(ctx, args),
                BlockType::DataByte => self.handle_data_byte(ctx, args),
                BlockType::DataWord => self.handle_data_word(ctx, args),
                BlockType::Address => self.handle_address(ctx, args),
                BlockType::PetsciiText => self.handle_petscii_text(ctx, args),
                BlockType::ScreencodeText => self.handle_screencode_text(ctx, args),
                BlockType::LoHiAddress => handlers::handle_lohi_address(ctx, args),
                BlockType::HiLoAddress => handlers::handle_hilo_address(ctx, args),
                BlockType::LoHiWord => handlers::handle_lohi_word(ctx, args),
                BlockType::HiLoWord => handlers::handle_hilo_word(ctx, args),
                BlockType::ExternalFile => self.handle_external_file(ctx, args),
                BlockType::Undefined => handlers::handle_undefined_byte(ctx, args),
            };
            lines.extend(new_lines);
            pc += bytes_consumed;
        }

        lines
    }

    fn get_arrow_target_address(
        &self,
        opcode: &Opcode,
        bytes: &[u8],
        address: Addr,
    ) -> Option<Addr> {
        use crate::cpu::AddressingMode;

        if !opcode.is_flow_control_with_target() {
            return None;
        }

        match opcode.mode {
            AddressingMode::Absolute => {
                if bytes.len() >= 3 {
                    Some(Addr(u16::from(bytes[2]) << 8 | u16::from(bytes[1])))
                } else {
                    None
                }
            }
            AddressingMode::Relative => {
                if bytes.len() >= 2 {
                    let offset = bytes[1] as i8;
                    Some(address.wrapping_add(2).wrapping_add(offset as u16))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Returns the address referenced by the instruction, if any.
    /// This is used for looking up comments and X-Refs.
    /// Unlike `get_flow_target_address`, this returns a value for memory access instructions like STA, LDA, etc.
    fn get_referenced_address(&self, opcode: &Opcode, bytes: &[u8], address: Addr) -> Option<Addr> {
        use crate::cpu::AddressingMode;

        match opcode.mode {
            AddressingMode::Absolute | AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => {
                if bytes.len() >= 3 {
                    Some(Addr(u16::from(bytes[2]) << 8 | u16::from(bytes[1])))
                } else {
                    None
                }
            }
            AddressingMode::ZeroPage | AddressingMode::ZeroPageX | AddressingMode::ZeroPageY => {
                if bytes.len() >= 2 {
                    Some(Addr(u16::from(bytes[1])))
                } else {
                    None
                }
            }
            AddressingMode::Relative => {
                if bytes.len() >= 2 {
                    let offset = bytes[1] as i8;
                    // Branch target
                    Some(address.wrapping_add(2).wrapping_add(offset as u16))
                } else {
                    None
                }
            }
            AddressingMode::Indirect => {
                if bytes.len() >= 3 {
                    Some(Addr(u16::from(bytes[2]) << 8 | u16::from(bytes[1])))
                } else {
                    None
                }
            }
            // For IndirectX/Y, we could argue it references the Zero Page address given.
            AddressingMode::IndirectX | AddressingMode::IndirectY => {
                if bytes.len() >= 2 {
                    Some(Addr(u16::from(bytes[1])))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn get_label_name(
        &self,
        address: Addr,
        labels: &BTreeMap<Addr, Vec<Label>>,
        formatter: &dyn Formatter,
        settings: &DocumentSettings,
    ) -> Option<String> {
        labels.get(&address).and_then(|v| {
            resolve_label(v, address.0, settings).map(|l| formatter.format_label(&l.name))
        })
    }

    fn get_side_comment(
        &self,
        address: Addr,
        ctx: &DisassemblyContext,
        comment_prefix: &str,
    ) -> String {
        let mut comment_parts = Vec::new();

        if let Some(user_comment) = ctx.user_side_comments.get(&address) {
            comment_parts.push(user_comment.clone());
        } else if let Some(sys_comment) = ctx.system_comments.get(&address) {
            comment_parts.push(sys_comment.clone());
        }

        if let Some(refs) = ctx.cross_refs.get(&address)
            && !refs.is_empty()
            && ctx.settings.max_xref_count > 0
        {
            comment_parts.push(format_cross_references(refs, ctx.settings.max_xref_count));
        }

        let separator = format!(" {comment_prefix} "); // e.g. " ; " or " // "
        comment_parts.join(&separator)
    }

    fn handle_code(
        &self,
        ctx: &DisassemblyContext,
        args: HandleArgs,
    ) -> (usize, Vec<DisassemblyLine>) {
        let HandleArgs {
            pc,
            address,
            formatter,
            label_name,
            mut side_comment,
            line_comment,
            local_label_names,
            label_routine_names,
            current_routine_name,
        } = args;
        let data = ctx.data;
        let block_types = ctx.block_types;
        let labels = ctx.labels;
        let settings = ctx.settings;
        let system_comments = ctx.system_comments;
        let user_side_comments = ctx.user_side_comments;
        let immediate_value_formats = ctx.immediate_value_formats;
        let opcode_byte = data[pc];
        let opcode_opt = &self.opcodes[opcode_byte as usize];

        if let Some(opcode) = opcode_opt
            && (!opcode.illegal || settings.use_illegal_opcodes)
        {
            let mut bytes = vec![opcode_byte];

            // Special handling for BRK
            // BRK ($00) normally takes 1 byte, but consumes 2 (signature byte).
            if opcode.mnemonic == "BRK" && !settings.brk_single_byte && pc + 1 < data.len() {
                let mut collision = false;
                if let Some(t) = block_types.get(pc + 1)
                    && *t != BlockType::Code
                    && *t != BlockType::Routine
                {
                    collision = true;
                }

                if !collision {
                    if settings.patch_brk {
                        // "Patch BRK": BRK (1 byte) then .byte (1 byte)
                        let byte_val = data[pc + 1];
                        return (
                            2,
                            vec![
                                DisassemblyLine {
                                    address,
                                    bytes: vec![opcode_byte],
                                    mnemonic: formatter.format_mnemonic(opcode.mnemonic),
                                    operand: String::new(),
                                    comment: side_comment,
                                    line_comment,
                                    label: label_name,
                                    opcode: Some(opcode.clone()),
                                    show_bytes: true,
                                    target_address: None,
                                    external_label_address: None,
                                    is_collapsed: false,
                                },
                                DisassemblyLine {
                                    address: address.wrapping_add(1),
                                    bytes: vec![byte_val],
                                    mnemonic: formatter.byte_directive().to_string(),
                                    operand: formatter.format_byte(byte_val),
                                    comment: String::new(),
                                    line_comment: None,
                                    label: self.get_label_name(
                                        address.wrapping_add(1),
                                        labels,
                                        formatter,
                                        settings,
                                    ),
                                    opcode: None,
                                    show_bytes: true,
                                    target_address: None,
                                    external_label_address: None,
                                    is_collapsed: false,
                                },
                            ],
                        );
                    } else {
                        // Default: BRK #$ signature
                        let byte_val = data[pc + 1];
                        let operand_str = format!("#{}", formatter.format_byte(byte_val));

                        return (
                            2,
                            vec![DisassemblyLine {
                                address,
                                bytes: vec![opcode_byte, byte_val],
                                mnemonic: formatter.format_mnemonic(opcode.mnemonic),
                                operand: operand_str,
                                comment: side_comment,
                                line_comment,
                                label: label_name,
                                opcode: Some(opcode.clone()),
                                show_bytes: true,
                                target_address: None,
                                external_label_address: None,
                                is_collapsed: false,
                            }],
                        );
                    }
                }
            }

            // Check if we have enough bytes
            if pc + opcode.size as usize <= data.len() {
                let mut collision = false;
                for i in 1..opcode.size {
                    if let Some(t) = block_types.get(pc + i as usize)
                        && *t != BlockType::Code
                        && *t != BlockType::Routine
                    {
                        collision = true;
                        break;
                    }
                }

                if !collision {
                    for i in 1..opcode.size {
                        bytes.push(data[pc + i as usize]);
                    }

                    // Append referenced address comment if any
                    // Use get_referenced_address for comments, NOT get_arrow_target_address
                    if let Some(target_addr) = self.get_referenced_address(opcode, &bytes, address)
                    {
                        // Calculate origin to check if target is within our data block
                        let origin = address.wrapping_sub(pc as u16);
                        let target_idx = target_addr.offset_from(origin);

                        // Check if target is known code
                        let mut is_code_target = false;
                        if target_idx < data.len()
                            && let Some(bt) = block_types.get(target_idx)
                            && (*bt == BlockType::Code || *bt == BlockType::Routine)
                        {
                            is_code_target = true;
                        }

                        // Should we show the user comment?
                        // If it's code, NO (avoids propagation in loops).
                        // If it's data/unknown, YES.
                        let target_comment = if is_code_target {
                            // Even if we suppress user comments for code, we might want system comments (e.g. KERNAL)
                            // But usually KERNAL/System targets won't be in our 'data' block types loop unless we disassembled the whole memory.
                            // If they are outside (target_idx >= len), is_code_target is false, so we show them (correct for external system calls).
                            // If they are INSIDE and marked as Code, we suppress user comments (to fix the bug).
                            system_comments.get(&target_addr)
                        } else if let Some(c) = user_side_comments.get(&target_addr) {
                            Some(c)
                        } else {
                            system_comments.get(&target_addr)
                        };

                        if let Some(target_comment) = target_comment {
                            if !side_comment.is_empty() {
                                side_comment.push_str("; ");
                            }
                            side_comment.push_str(target_comment);
                        }
                    }

                    let target_context = match opcode.mode {
                        crate::cpu::AddressingMode::ZeroPage => {
                            Some(crate::state::LabelType::ZeroPageAbsoluteAddress)
                        }
                        crate::cpu::AddressingMode::ZeroPageX => {
                            Some(crate::state::LabelType::ZeroPageField)
                        }
                        crate::cpu::AddressingMode::ZeroPageY => {
                            Some(crate::state::LabelType::ZeroPageField)
                        }
                        crate::cpu::AddressingMode::Relative => {
                            Some(crate::state::LabelType::Branch)
                        }
                        crate::cpu::AddressingMode::Absolute => {
                            if opcode.mnemonic == "JSR" {
                                Some(crate::state::LabelType::Subroutine)
                            } else if opcode.mnemonic == "JMP" {
                                Some(crate::state::LabelType::Jump)
                            } else {
                                Some(crate::state::LabelType::AbsoluteAddress)
                            }
                        }
                        crate::cpu::AddressingMode::AbsoluteX => {
                            Some(crate::state::LabelType::Field)
                        }
                        crate::cpu::AddressingMode::AbsoluteY => {
                            Some(crate::state::LabelType::Field)
                        }
                        crate::cpu::AddressingMode::Indirect => {
                            Some(crate::state::LabelType::Pointer)
                        }
                        crate::cpu::AddressingMode::IndirectX => {
                            Some(crate::state::LabelType::ZeroPagePointer)
                        }
                        crate::cpu::AddressingMode::IndirectY => {
                            Some(crate::state::LabelType::ZeroPagePointer)
                        }
                        _ => None,
                    };

                    let ctx = crate::disassembler::formatter::FormatContext {
                        opcode,
                        operands: &bytes[1..],
                        address,
                        target_context,
                        labels,
                        settings,
                        immediate_value_formats,
                        local_label_names,
                        label_routine_names,
                        current_routine_name: current_routine_name.as_deref(),
                    };
                    let (mnemonic, operand_str) = formatter.format_instruction(&ctx);

                    let target_address = self.get_arrow_target_address(opcode, &bytes, address);

                    return (
                        opcode.size as usize,
                        vec![DisassemblyLine {
                            address,
                            bytes,
                            mnemonic,
                            operand: operand_str,
                            comment: side_comment,
                            line_comment,
                            label: label_name,
                            opcode: Some(opcode.clone()),
                            show_bytes: true,
                            target_address,
                            external_label_address: None,
                            is_collapsed: false,
                        }],
                    );
                }
            }
        }

        // Fallthrough / Invalid instruction
        let mut side_comment_final = "Invalid or partial instruction".to_string();
        if !side_comment.is_empty() {
            side_comment_final = format!("{side_comment}; {side_comment_final}");
        }
        (
            1,
            vec![DisassemblyLine {
                address,
                bytes: vec![opcode_byte],
                mnemonic: formatter.byte_directive().to_string(),
                operand: formatter.format_byte(opcode_byte),
                comment: side_comment_final,
                line_comment,
                label: label_name,
                opcode: None,
                show_bytes: true,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            }],
        )
    }
    fn handle_data_byte(
        &self,
        ctx: &DisassemblyContext,
        args: HandleArgs,
    ) -> (usize, Vec<DisassemblyLine>) {
        let HandleArgs {
            pc,
            address: _,
            formatter,
            label_name,
            side_comment,
            line_comment,
            local_label_names: _,
            ..
        } = args;
        let data = ctx.data;
        let block_types = ctx.block_types;
        let labels = ctx.labels;
        let origin = ctx.origin;
        let splitters = ctx.splitters;
        let settings = ctx.settings;
        let user_line_comments = ctx.user_line_comments;
        let address = origin.wrapping_add(pc as u16);
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.bytes_per_line {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            // Stop if type changes
            if block_types.get(current_pc) != Some(&BlockType::DataByte) {
                break;
            }

            // Stop if splitter exists (except start)
            if count > 0 && splitters.contains(&current_address) {
                break;
            }

            // Stop if label exists (except for the first byte)
            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            // Stop if line comment exists (except start)
            if count > 0 && user_line_comments.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];
            bytes.push(b);
            operands.push(formatter.format_byte(b));
            count += 1;
        }

        (
            count,
            vec![DisassemblyLine {
                address,
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment: side_comment,
                line_comment,
                label: label_name,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            }],
        )
    }

    fn handle_data_word(
        &self,
        ctx: &DisassemblyContext,
        args: HandleArgs,
    ) -> (usize, Vec<DisassemblyLine>) {
        let HandleArgs {
            pc,
            address,
            formatter,
            label_name,
            side_comment,
            line_comment,
            local_label_names: _,
            ..
        } = args;
        let data = ctx.data;
        let block_types = ctx.block_types;
        let labels = ctx.labels;
        let origin = ctx.origin;
        let splitters = ctx.splitters;
        let settings = ctx.settings;
        let user_line_comments = ctx.user_line_comments;
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0; // Number of words

        while pc + (count * 2) + 1 < data.len() && count < settings.addresses_per_line {
            let current_pc_start = pc + (count * 2);
            let current_address = origin.wrapping_add(current_pc_start as u16);
            let next_address = current_address.wrapping_add(1);

            if block_types.get(current_pc_start) != Some(&BlockType::DataWord)
                || block_types.get(current_pc_start + 1) != Some(&BlockType::DataWord)
            {
                break;
            }

            if count > 0 {
                // Check if splitter exists at start of this word
                if splitters.contains(&current_address) {
                    break;
                }
            }

            // Check if splitter exists in the middle of this word
            if splitters.contains(&next_address) {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            // Stop if line comment exists (except start)
            // Note: DataWord is 2 bytes. We check if comment is at the start of the word.
            if count > 0 && user_line_comments.contains_key(&current_address) {
                break;
            }

            let low = data[current_pc_start];
            let high = data[current_pc_start + 1];
            let val = u16::from(high) << 8 | u16::from(low);

            bytes.push(low);
            bytes.push(high);
            operands.push(formatter.format_address(Addr(val)));
            count += 1;
        }

        if count > 0 {
            (
                count * 2,
                vec![DisassemblyLine {
                    address,
                    bytes,
                    mnemonic: formatter.word_directive().to_string(),
                    operand: operands.join(", "),
                    comment: side_comment,
                    line_comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                }],
            )
        } else {
            // Fallback for partial word
            self.handle_partial_data(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
                None,
                "Word",
            )
        }
    }

    fn handle_external_file(
        &self,
        ctx: &DisassemblyContext,
        args: HandleArgs,
    ) -> (usize, Vec<DisassemblyLine>) {
        let HandleArgs {
            pc,
            address: _,
            formatter,
            label_name,
            side_comment,
            line_comment,
            local_label_names: _,
            ..
        } = args;
        let data = ctx.data;
        let block_types = ctx.block_types;
        let labels = ctx.labels;
        let origin = ctx.origin;
        let splitters = ctx.splitters;
        let settings = ctx.settings;
        let user_line_comments = ctx.user_line_comments;
        let address = origin.wrapping_add(pc as u16);
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.bytes_per_line {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            // Stop if type changes
            if block_types.get(current_pc) != Some(&BlockType::ExternalFile) {
                break;
            }

            // Stop if splitter exists (except start)
            if count > 0 && splitters.contains(&current_address) {
                break;
            }

            // Stop if label exists (except for the first byte)
            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            if count > 0 && user_line_comments.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];
            bytes.push(b);
            operands.push(formatter.format_byte(b));
            count += 1;
        }

        (
            count,
            vec![DisassemblyLine {
                address,
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment: side_comment,
                line_comment,
                label: label_name,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            }],
        )
    }

    fn handle_address(
        &self,
        ctx: &DisassemblyContext,
        args: HandleArgs,
    ) -> (usize, Vec<DisassemblyLine>) {
        let HandleArgs {
            pc,
            address,
            formatter,
            label_name,
            mut side_comment,
            line_comment,
            local_label_names,
            label_routine_names,
            current_routine_name,
        } = args;
        let data = ctx.data;
        let block_types = ctx.block_types;
        let labels = ctx.labels;
        let origin = ctx.origin;
        let system_comments = ctx.system_comments;
        let user_side_comments = ctx.user_side_comments;
        let settings = ctx.settings;
        let user_line_comments = ctx.user_line_comments;
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + (count * 2) + 1 < data.len() && count < settings.addresses_per_line {
            let current_pc_start = pc + (count * 2);
            let current_address = origin.wrapping_add(current_pc_start as u16);

            if block_types.get(current_pc_start) != Some(&BlockType::Address)
                || block_types.get(current_pc_start + 1) != Some(&BlockType::Address)
            {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            if count > 0 && user_line_comments.contains_key(&current_address) {
                break;
            }

            let low = data[current_pc_start];
            let high = data[current_pc_start + 1];
            let val = u16::from(high) << 8 | u16::from(low);

            // START: Append comment for the address value
            let target_comment = if let Some(c) = user_side_comments.get(&val) {
                Some(c)
            } else {
                system_comments.get(&val)
            };

            if let Some(target_comment) = target_comment {
                if !side_comment.is_empty() {
                    side_comment.push_str("; ");
                }
                side_comment.push_str(target_comment);
            }
            // END: Append comment

            bytes.push(low);
            bytes.push(high);

            let operand = resolve_label_name(
                Addr(val),
                labels,
                settings,
                local_label_names,
                label_routine_names,
                current_routine_name.as_deref(),
            )
            .unwrap_or_else(|| formatter.format_address(Addr(val)));
            operands.push(operand);

            count += 1;
        }

        if count > 0 {
            (
                count * 2,
                vec![DisassemblyLine {
                    address,
                    bytes,
                    mnemonic: formatter.word_directive().to_string(),
                    operand: operands.join(", "),
                    comment: side_comment,
                    line_comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                }],
            )
        } else {
            self.handle_partial_data(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
                None,
                "Address",
            )
        }
    }

    fn handle_petscii_text(
        &self,
        ctx: &DisassemblyContext,
        args: HandleArgs,
    ) -> (usize, Vec<DisassemblyLine>) {
        use crate::disassembler::formatter::TextFragment;

        let HandleArgs {
            pc,
            address,
            formatter,
            label_name,
            side_comment,
            line_comment,
            local_label_names: _,
            ..
        } = args;
        let data = ctx.data;
        let block_types = ctx.block_types;
        let labels = ctx.labels;
        let origin = ctx.origin;
        let settings = ctx.settings;
        let splitters = ctx.splitters;
        let user_line_comments = ctx.user_line_comments;

        let mut fragments = Vec::new();
        let mut current_literal = String::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.text_char_limit {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            if block_types.get(current_pc) != Some(&BlockType::PetsciiText) {
                break;
            }

            if count > 0 && splitters.contains(&current_address) {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            if count > 0 && user_line_comments.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];
            // Check if "printable" ASCII-ish from 0x20 to 0x7E
            if (0x20..=0x7E).contains(&b) {
                let c = b as char;
                current_literal.push(c);
            } else {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b));
            }

            count += 1;
        }

        if !current_literal.is_empty() {
            fragments.push(TextFragment::Text(current_literal));
        }

        let is_start = pc == 0 || block_types.get(pc - 1) != Some(&BlockType::PetsciiText);
        let next_pc = pc + count;
        let is_end =
            next_pc >= data.len() || block_types.get(next_pc) != Some(&BlockType::PetsciiText);

        if count > 0 {
            // Check for assembler type based on formatter directives
            let formatted_lines = formatter.format_text(&fragments, is_start, is_end);
            let mut disassembly_lines = Vec::new();

            // Find the first line that emits bytes to attach the label to.
            // For 64tass, this avoids attaching labels to .encode directives where they are not allowed.
            let first_byte_line_index = formatted_lines
                .iter()
                .position(|(_, _, has_bytes)| *has_bytes)
                .unwrap_or(0);

            // We need to attach bytes to lines.
            // Since we merged everything into one line (usually), we attach ALL bytes to that line ?
            // Or if format_text returning multiple lines (header/footer), we attach to the main one.
            // The formatter returns Vec<(mnemonic, operand, has_bytes)>.

            // We need to collect all bytes consumed
            let mut all_bytes = Vec::new();
            for i in 0..count {
                all_bytes.push(data[pc + i]);
            }

            for (i, (mnemonic, operand, has_bytes)) in formatted_lines.iter().enumerate() {
                let line_bytes = if *has_bytes {
                    all_bytes.clone()
                } else {
                    Vec::new()
                };
                let line_label = if i == first_byte_line_index {
                    label_name.clone()
                } else {
                    None
                };
                let (line_side_comment, line_line_comment) = if i == first_byte_line_index {
                    (side_comment.clone(), line_comment.clone())
                } else {
                    (String::new(), None)
                };

                disassembly_lines.push(DisassemblyLine {
                    address,
                    bytes: line_bytes,
                    mnemonic: mnemonic.clone(),
                    operand: operand.clone(),
                    comment: line_side_comment,
                    line_comment: line_line_comment,
                    label: line_label,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                });
            }

            (count, disassembly_lines)
        } else {
            // Fallback to byte if no valid chunk (should not happen if block_type is text and len > 0)
            self.handle_partial_data(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
                None,
                "Text",
            )
        }
    }

    fn handle_screencode_text(
        &self,
        ctx: &DisassemblyContext,
        args: HandleArgs,
    ) -> (usize, Vec<DisassemblyLine>) {
        use crate::disassembler::formatter::TextFragment;

        let HandleArgs {
            pc,
            address,
            formatter,
            label_name,
            side_comment,
            line_comment,
            local_label_names: _,
            ..
        } = args;
        let data = ctx.data;
        let block_types = ctx.block_types;
        let labels = ctx.labels;
        let origin = ctx.origin;
        let settings = ctx.settings;
        let splitters = ctx.splitters;
        let user_line_comments = ctx.user_line_comments;

        let mut fragments = Vec::new();
        let mut current_literal = String::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.text_char_limit {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            if block_types.get(current_pc) != Some(&BlockType::ScreencodeText) {
                break;
            }

            if count > 0 && splitters.contains(&current_address) {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            if count > 0 && user_line_comments.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];

            // bytes >= $5f should be treated as bytes. Not as screencodes.
            if b >= 0x5f {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b));
                count += 1;
                continue;
            }

            // Map Screen Code to ASCII
            // 0-31 (@..left arrow) -> 64..95
            // 32-63 (space..?) -> 32..63
            let ascii = if b < 32 {
                b + 64
            } else if b < 64 {
                b
            } else if b < 96 {
                b + 32
            } else {
                // Extended/Reverse codes -> raw byte
                b
            };

            if (0x20..=0x7E).contains(&ascii) {
                let c = ascii as char;
                current_literal.push(c);
            } else {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b)); // Use ORIGINAL byte
            }

            count += 1;
        }

        if !current_literal.is_empty() {
            fragments.push(TextFragment::Text(current_literal));
        }

        let is_start = pc == 0 || block_types.get(pc - 1) != Some(&BlockType::ScreencodeText);
        let next_pc = pc + count;
        let is_end =
            next_pc >= data.len() || block_types.get(next_pc) != Some(&BlockType::ScreencodeText);

        if count > 0 {
            let mut all_formatted_parts = Vec::new();

            if is_start {
                let pre_lines = formatter.format_screencode_pre();
                for (m, o) in pre_lines {
                    all_formatted_parts.push((m, o, false));
                }
            }

            let body_lines = formatter.format_screencode(&fragments);
            all_formatted_parts.extend(body_lines);

            if is_end {
                let post_lines = formatter.format_screencode_post();
                for (m, o) in post_lines {
                    all_formatted_parts.push((m, o, false));
                }
            }

            let mut disassembly_lines = Vec::new();

            // Find the first line that emits bytes to attach the label to (e.g. the body .text line).
            let first_byte_line_index = all_formatted_parts
                .iter()
                .position(|(_, _, has_bytes)| *has_bytes)
                .unwrap_or(0);

            // Collect all consumed bytes for line association
            let mut all_bytes = Vec::new();
            for i in 0..count {
                all_bytes.push(data[pc + i]);
            }

            for (i, (mnemonic, operand, has_bytes)) in all_formatted_parts.iter().enumerate() {
                // Attach bytes only to marked lines by the formatter (usually the body lines).
                // Attach label and comment only to the very first line of the entire block.
                let line_bytes = if *has_bytes {
                    all_bytes.clone()
                } else {
                    Vec::new()
                };
                let line_label = if i == first_byte_line_index {
                    label_name.clone()
                } else {
                    None
                };
                let (line_side_comment, line_line_comment) = if i == first_byte_line_index {
                    (side_comment.clone(), line_comment.clone())
                } else {
                    (String::new(), None)
                };

                disassembly_lines.push(DisassemblyLine {
                    address,
                    bytes: line_bytes,
                    mnemonic: mnemonic.clone(),
                    operand: operand.clone(),
                    comment: line_side_comment,
                    line_comment: line_line_comment,
                    label: line_label,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                });
            }

            (count, disassembly_lines)
        } else {
            self.handle_partial_data(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
                None,
                "Screencode",
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_partial_data(
        &self,
        pc: usize,
        data: &[u8],
        address: Addr,
        formatter: &dyn Formatter,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
        _local_label_names: Option<&BTreeMap<Addr, String>>,
        type_name: &str,
    ) -> (usize, Vec<DisassemblyLine>) {
        if pc < data.len() {
            let b = data[pc];
            let mut side_comment_final = format!("Partial {type_name}");
            if !side_comment.is_empty() {
                side_comment_final = format!("{side_comment}; {side_comment_final}");
            }
            (
                1,
                vec![DisassemblyLine {
                    address,
                    bytes: vec![b],
                    mnemonic: formatter.byte_directive().to_string(),
                    operand: formatter.format_byte(b),
                    comment: side_comment_final,
                    line_comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                }],
            )
        } else {
            // Should not happen if loop condition is correct
            (
                0,
                vec![DisassemblyLine {
                    address,
                    bytes: vec![],
                    mnemonic: "???".to_string(),
                    operand: String::new(),
                    comment: "Error: Out of bounds".to_string(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                }],
            )
        }
    }
}

#[must_use]
pub fn format_cross_references(refs: &[Addr], max_count: usize) -> String {
    if refs.is_empty() || max_count == 0 {
        return String::new();
    }

    let mut all_refs = refs.to_vec();
    all_refs.sort_unstable();
    all_refs.dedup();

    let refs_str: Vec<String> = all_refs
        .iter()
        .take(max_count)
        .map(|r| format!("${r:04x}"))
        .collect();

    let suffix = if all_refs.len() > max_count {
        ", ..."
    } else {
        ""
    };

    format!("x-ref: {}{}", refs_str.join(", "), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cross_references() {
        // Test truncation
        let refs = vec![Addr(0x2000), Addr(0x3000), Addr(0x4000)];
        let output = format_cross_references(&refs, 2);
        assert_eq!(output, "x-ref: $2000, $3000, ...");

        // Test no truncation
        let output_full = format_cross_references(&refs, 5);
        assert_eq!(output_full, "x-ref: $2000, $3000, $4000");

        // Test deduplication
        let refs_dup = vec![Addr(0x2000), Addr(0x2000)];
        let output_dup = format_cross_references(&refs_dup, 2);
        assert_eq!(output_dup, "x-ref: $2000");
    }
}
