use regenerator2000_core::disassembler::Disassembler;
use regenerator2000_core::state::{
    Addr, AnnotationManager, Assembler, BlockType, DocumentSettings, ImmediateFormat, Label,
    LabelKind, LabelType, OffsetRadix, SymbolicOffset,
};
use std::collections::{BTreeMap, BTreeSet};

/// Formats a single raw 6502 instruction under `settings` with empty symbol and annotation tables.
pub fn format_raw_instruction(
    settings: &DocumentSettings,
    opcode_byte: u8,
    operands: &[u8],
    address: u16,
) -> (String, String) {
    let opcodes = regenerator2000_core::cpu::get_opcodes();
    let labels = BTreeMap::new();
    let annotations = AnnotationManager::default();
    let empty_enums = BTreeMap::new();
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[usize::from(opcode_byte)].as_ref().unwrap(),
        operands,
        address: Addr(address),
        target_context: None,
        labels: &labels,
        settings,
        annotations: &annotations,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
        scope_separator: ".",
        local_prefix: None,
        enums: &empty_enums,
        user_global_enums: &empty_enums,
        builtin_enums: &empty_enums,
    };
    Disassembler::formatter(settings.assembler).format_instruction(&ctx)
}

/// Verifies Tier 1 and Tier 2 symbolic-offset formatting across all 6502 addressing modes,
/// immediate byte extractors, `.word` address tables, and `LoHiAddress`/`HiLoAddress` split tables.
pub fn assert_symbolic_offset_formatting_all_modes(
    assembler: Assembler,
    forced_abs_mnemonic: &str,
    forced_abs_operand: &str,
) {
    let settings = DocumentSettings {
        assembler,
        preserve_long_bytes: true,
        addresses_per_line: 2,
        ..Default::default()
    };
    let disassembler = Disassembler::new();
    let formatter = Disassembler::formatter(assembler);
    let byte_dir = formatter.byte_directive();
    let word_dir = formatter.word_directive();

    let mut labels = BTreeMap::new();
    for (addr, name) in [
        (0x00FB, "zp_ptr"),
        (0x00EB, "zp_tab"),
        (0x0400, "SCREEN_RAM"),
        (0x2000, "table"),
        (0x0100, "table_0100"),
        (0xC000, "vec"),
        (0xC100, "sub_C100"),
        (0xC105, "sub_C105"),
        (0xC200, "sub_C200"),
    ] {
        labels.insert(
            Addr(addr),
            vec![Label {
                name: name.to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
    }

    let mut ann = AnnotationManager::default();
    for (addr, base) in [(0x00FC, 0x00FB), (0x00DE, 0x00DD)] {
        ann.update(Addr(addr), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(base),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
    }

    let code = vec![
        0xA5, 0xFC, // $1000: lda zp_ptr + 1
        0xA5, 0xDE, // $1002: lda $dd + 1 (unlabelled ZP base, 2-digit)
        0xB5, 0xFC, // $1004: lda zp_ptr + 1,x
        0xB6, 0xFC, // $1006: ldx zp_ptr + 1,y
        0x8D, 0x28, 0x04, // $1008: sta SCREEN_RAM + 40
        0x8D, 0x28, 0x04, // $100B: sta SCREEN_RAM + $28
        0x8D, 0x28, 0x05, // $100E: sta $0500 + 40
        0xBD, 0xFF, 0x1F, // $1011: lda table - 1,x
        0xB9, 0x28, 0x04, // $1014: lda SCREEN_RAM + $28,y
        0xBD, 0xFF, 0x00, // $1017: forced-absolute $00FF <-> $0100
        0x6C, 0x02, 0xC0, // $101A: jmp (vec + 2)
        0xA1, 0xED, // $101D: lda (zp_tab + 2,x)
        0xB1, 0xFD, // $101F: lda (zp_ptr + 2),y
        0xA9, 0xFF, // $1021: lda #<(sub_C100 - 1)
        0xA0, 0xC0, // $1023: ldy #>(sub_C100 - 1)
        0xA9, 0x00, // $1025: lda #<sub_C100
        0xA9, 0xEE, // $1027: lda #<$ee
        0xFF, 0xC0, 0x00, 0xC2, // $1029..=$102C: Address
        0xFF, 0x04, 0xC0, 0xC1, // $102D..=$1030: LoHiAddress
    ];
    let mut block_types = vec![BlockType::Code; 0x29];
    block_types.extend([BlockType::Address; 4]);
    block_types.extend([BlockType::LoHiAddress; 4]);

    for (addr, base, offset, radix) in [
        (0x1008, 0x0400, 40, OffsetRadix::Decimal),
        (0x100B, 0x0400, 0x28, OffsetRadix::Hex),
        (0x100E, 0x0500, 40, OffsetRadix::Decimal),
        (0x1011, 0x2000, -1, OffsetRadix::Decimal),
        (0x1014, 0x0400, 0x28, OffsetRadix::Hex),
        (0x1017, 0x0100, -1, OffsetRadix::Decimal),
        (0x101A, 0xC000, 2, OffsetRadix::Decimal),
        (0x101D, 0x00EB, 2, OffsetRadix::Decimal),
        (0x101F, 0x00FB, 2, OffsetRadix::Decimal),
        (0x1029, 0xC100, -1, OffsetRadix::Decimal),
        (0x102D, 0xC100, -1, OffsetRadix::Decimal),
        (0x1030, 0xC105, -1, OffsetRadix::Decimal),
        (0x1021, 0xC100, -1, OffsetRadix::Decimal),
        (0x1023, 0xC100, -1, OffsetRadix::Decimal),
    ] {
        ann.update(Addr(addr), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(base),
                offset,
                radix,
            });
        });
    }
    for (addr, fmt) in [
        (0x1021, ImmediateFormat::LowByte(Addr(0xC0FF))),
        (0x1023, ImmediateFormat::HighByte(Addr(0xC0FF))),
        (0x1025, ImmediateFormat::LowByte(Addr(0xC100))),
        (0x1027, ImmediateFormat::LowByte(Addr(0x00EE))),
    ] {
        ann.update(Addr(addr), |e| {
            e.immediate_format = Some(fmt);
        });
    }

    let empty_xrefs = BTreeMap::new();
    let empty_splitters = BTreeSet::new();
    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        Addr(0x1000),
        &settings,
        &ann,
        &empty_xrefs,
        &[],
        &empty_splitters,
    );
    let by_addr = |a: u16| {
        lines
            .iter()
            .find(|l| l.address == Addr(a))
            .unwrap_or_else(|| panic!("missing line at ${a:04X} for {assembler:?}"))
    };

    for (addr, exp_mnem, exp_op) in [
        (0x1000, "lda", "zp_ptr + 1"),
        (0x1002, "lda", "$dd + 1"),
        (0x1004, "lda", "zp_ptr + 1,x"),
        (0x1006, "ldx", "zp_ptr + 1,y"),
        (0x1008, "sta", "SCREEN_RAM + 40"),
        (0x100B, "sta", "SCREEN_RAM + $28"),
        (0x100E, "sta", "$0500 + 40"),
        (0x1011, "lda", "table - 1,x"),
        (0x1014, "lda", "SCREEN_RAM + $28,y"),
        (0x1017, forced_abs_mnemonic, forced_abs_operand),
        (0x101A, "jmp", "(vec + 2)"),
        (0x101D, "lda", "(zp_tab + 2,x)"),
        (0x101F, "lda", "(zp_ptr + 2),y"),
        (0x1021, "lda", "#<(sub_C100 - 1)"),
        (0x1023, "ldy", "#>(sub_C100 - 1)"),
        (0x1025, "lda", "#<sub_C100"),
        (0x1027, "lda", "#<$ee"),
        (0x1029, word_dir, "sub_C100 - 1, sub_C200"),
        (0x102D, byte_dir, "<(sub_C100 - 1), <(sub_C105 - 1)"),
        (0x102F, byte_dir, ">(sub_C100 - 1), >(sub_C105 - 1)"),
    ] {
        let line = by_addr(addr);
        assert_eq!(
            (line.mnemonic.as_str(), line.operand.as_str()),
            (exp_mnem, exp_op),
            "mismatch at ${addr:04X} for {assembler:?}"
        );
    }
    for table_addr in [0x1029, 0x102D, 0x102F] {
        assert_eq!(by_addr(table_addr).target_address, None);
    }

    assert_unlabelled_abs_zp_hex_base(&disassembler, assembler, &labels, &ann);
    assert_hilo_split_table_offsets(&disassembler, assembler, &labels, byte_dir);
}

fn assert_unlabelled_abs_zp_hex_base(
    disassembler: &Disassembler,
    assembler: Assembler,
    labels: &BTreeMap<Addr, Vec<Label>>,
    ann: &AnnotationManager,
) {
    let settings = DocumentSettings {
        assembler,
        preserve_long_bytes: false,
        ..Default::default()
    };
    let empty_xrefs = BTreeMap::new();
    let empty_splitters = BTreeSet::new();
    let abs_zp_lines = disassembler.disassemble(
        &[0x8D, 0xDE, 0x00],
        &[BlockType::Code; 3],
        labels,
        Addr(0x2000),
        &settings,
        ann,
        &empty_xrefs,
        &[],
        &empty_splitters,
    );
    assert_eq!(
        (
            abs_zp_lines[0].mnemonic.as_str(),
            abs_zp_lines[0].operand.as_str()
        ),
        ("sta", "$00dd + 1")
    );
}

fn assert_hilo_split_table_offsets(
    disassembler: &Disassembler,
    assembler: Assembler,
    labels: &BTreeMap<Addr, Vec<Label>>,
    byte_dir: &str,
) {
    let settings = DocumentSettings {
        assembler,
        ..Default::default()
    };
    let empty_xrefs = BTreeMap::new();
    let empty_splitters = BTreeSet::new();
    let mut hilo_ann = AnnotationManager::default();
    for (addr, base) in [(0x3000, 0xC100), (0x3003, 0xC105)] {
        hilo_ann.update(Addr(addr), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(base),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
    }
    let hilo_lines = disassembler.disassemble(
        &[0xC0, 0xC1, 0xFF, 0x04],
        &[BlockType::HiLoAddress; 4],
        labels,
        Addr(0x3000),
        &settings,
        &hilo_ann,
        &empty_xrefs,
        &[],
        &empty_splitters,
    );
    assert_eq!(hilo_lines.len(), 2);
    assert_eq!(
        (&hilo_lines[0].mnemonic[..], &hilo_lines[0].operand[..]),
        (byte_dir, ">(sub_C100 - 1), >(sub_C105 - 1)")
    );
    assert_eq!(hilo_lines[0].target_address, None);
    assert_eq!(
        (&hilo_lines[1].mnemonic[..], &hilo_lines[1].operand[..]),
        (byte_dir, "<(sub_C100 - 1), <(sub_C105 - 1)")
    );
    assert_eq!(hilo_lines[1].target_address, None);
}
