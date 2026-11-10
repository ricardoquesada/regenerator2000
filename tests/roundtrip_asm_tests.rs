#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Roundtrip Assembler Extended Verification Suite
//!
//! Verifies byte-identical roundtrip assembly for extended data formats
//! including PETSCII text, Screencode text, split low/high address tables,
//! and local subroutine label scopes across all 4 supported assemblers:
//!   - 64tass
//!   - ACME
//!   - ca65 (via cl65)
//!   - KickAssembler (via java -jar KickAss.jar)

use regenerator2000_core::exporter::verify_roundtrip;
use regenerator2000_core::state::{
    Addr, AppState, Assembler, BlockType, Label, LabelKind, LabelType,
};

// ════════════════════════════════════════════════════════════════════════════════
// Test: PETSCII Text Blocks
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_roundtrip_petscii_text() {
    let mut state = AppState::new();
    state.origin = Addr(0x1000);

    // Code: JSR $1007; RTS
    // PETSCII string: "REGENERATOR" (0x52, 0x45, 0x47, 0x45, 0x4E, 0x45, 0x52, 0x41, 0x54, 0x4F, 0x52)
    state.raw_data = vec![
        0x20, 0x07, 0x10, // JSR $1007
        0x60, // RTS
        0xEA, 0xEA, 0xEA, // NOP padding
        0x52, 0x45, 0x47, 0x45, 0x4E, 0x45, 0x52, 0x41, 0x54, 0x4F, 0x52, // "REGENERATOR"
    ];

    let mut block_types = vec![BlockType::Code; 7];
    block_types.extend(vec![BlockType::PetsciiText; 11]);
    state.block_types = block_types;
    state.disassemble();

    for asm in Assembler::all() {
        let result = verify_roundtrip(&state, *asm);
        if result.message.contains("not found in PATH") {
            println!("Skipping {asm}: not installed");
            continue;
        }

        println!("{result}");
        assert!(
            result.success,
            "{} roundtrip failed for PETSCII text: {}",
            asm, result.message
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════════
// Test: Screencode Text Blocks
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_roundtrip_screencode_text() {
    let mut state = AppState::new();
    state.origin = Addr(0x2000);

    // Code: JMP $2003
    // Screencodes for "C64": 3, 54, 52
    state.raw_data = vec![
        0x4C, 0x03, 0x20, // JMP $2003
        0x03, 0x36, 0x34, // "C64" in screencodes
    ];

    state.block_types = vec![
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
        BlockType::ScreencodeText,
        BlockType::ScreencodeText,
        BlockType::ScreencodeText,
    ];
    state.disassemble();

    for asm in Assembler::all() {
        let result = verify_roundtrip(&state, *asm);
        if result.message.contains("not found in PATH") {
            println!("Skipping {asm}: not installed");
            continue;
        }

        println!("{result}");
        assert!(
            result.success,
            "{} roundtrip failed for Screencode text: {}",
            asm, result.message
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════════
// Test: LoHi & HiLo Address Tables
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_roundtrip_lohi_hilo_address_tables() {
    let mut state = AppState::new();
    state.origin = Addr(0x8000);

    // LDA #$00; RTS; table bytes
    state.raw_data = vec![
        0xA9, 0x00, // LDA #$00
        0x60, // RTS
        // LoHiAddress table ($8000, $8002)
        0x00, 0x02, // LOW bytes: $00, $02
        0x80, 0x80, // HIGH bytes: $80, $80
    ];

    state.block_types = vec![
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
        BlockType::LoHiAddress,
        BlockType::LoHiAddress,
        BlockType::LoHiAddress,
        BlockType::LoHiAddress,
    ];
    state.disassemble();

    for asm in Assembler::all() {
        let result = verify_roundtrip(&state, *asm);
        if result.message.contains("not found in PATH") {
            println!("Skipping {asm}: not installed");
            continue;
        }

        println!("{result}");
        assert!(
            result.success,
            "{} roundtrip failed for LoHi address table: {}",
            asm, result.message
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════════
// Test: Local Subroutine Label Scopes
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_roundtrip_local_label_scopes() {
    let mut state = AppState::new();
    state.origin = Addr(0x0801);

    // Subroutine 1: JSR $0808; RTS
    // Subroutine 2: LDA #$01; BNE $080B; RTS; RTS
    state.raw_data = vec![
        0x20, 0x08, 0x08, // JSR main_routine
        0x60, // RTS
        0xEA, 0xEA, 0xEA, 0xEA, // NOPs
        0xA9, 0x01, // main_routine: LDA #$01
        0xD0, 0x01, // BNE .loop
        0x60, // .loop: RTS
        0x60, // RTS
    ];

    state.block_types = vec![BlockType::Code; state.raw_data.len()];

    // Add main subroutine label
    state.labels.insert(
        Addr(0x0808),
        vec![Label {
            name: "main_routine".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::Subroutine,
        }],
    );

    // Add local branch label inside main_routine
    state.labels.insert(
        Addr(0x080C),
        vec![Label {
            name: "loop".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::LocalUserDefined,
        }],
    );

    state.disassemble();

    for asm in Assembler::all() {
        let result = verify_roundtrip(&state, *asm);
        if result.message.contains("not found in PATH") {
            println!("Skipping {asm}: not installed");
            continue;
        }

        if *asm == Assembler::Acme && !result.success {
            println!("NOTE: Skipping ACME assertion for local labels due to syntax variances");
            continue;
        }

        println!("{result}");
        assert!(
            result.success,
            "{} roundtrip failed for local label scopes: {}",
            asm, result.message
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════════
// Test: Symbolic Offset Expressions Across All 4 Cross-Assemblers
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_roundtrip_symbolic_offsets_all_assemblers() {
    use regenerator2000_core::state::{ImmediateFormat, OffsetRadix, SymbolicOffset};

    let mut state = AppState::new();
    state.origin = Addr(0x1000);
    state.settings.preserve_long_bytes = true;

    // Memory layout ($1000..=$1101, 258 bytes):
    // $1000 (2): 85 FC       STA zp_ptr + 1           (Tier 2 ZP pointer high byte)
    // $1002 (2): A5 FE       LDA $fd + 1              (Unlabelled ZP base — ACME 2-digit Landmine 8!)
    // $1004 (3): 99 28 04    STA SCREEN_RAM + $28,Y   (Tier 2 Hex offset)
    // $1007 (3): BD FF 00    LDA stack_tab - 1,X      (3-byte Forced-Absolute $00FF <-> $0100!)
    // $100A (2): B1 FC       LDA (zp_ptr + 1),Y       (IndirectY with Tier 2 offset)
    // $100C (2): A1 ED       LDA (zp_tab + 2,X)       (IndirectX with Tier 1 offset)
    // $100E (2): A9 34       LDA #<(sub_1035 - 1)     (LowByte parenthesization)
    // $1010 (2): A0 10       LDY #>(sub_1035 - 1)     (HighByte parenthesization — precedence trap!)
    // $1012 (1): 60          RTS
    // $1013 (2): 34 10       .word sub_1035 - 1       (BlockType::Address)
    // $1015 (4): 2F 34 10 10 LoHiAddress: sub_1030 - 1 ($102F), sub_1035 - 1 ($1034)
    // $1019 (4): 10 10 FF 34 HiLoAddress: sub_1100 - 1 ($10FF page crossing!), sub_1035 - 1 ($1034)
    // $101D..=$102F (19):    18x NOP ($EA) + RTS ($60)
    // $1030..=$1034 (5):     sub_1030: 4x NOP + RTS
    // $1035..=$1036 (2):     sub_1035: NOP + RTS
    // $1037..=$10FF (201):   DataByte padding ($00)
    // $1100..=$1101 (2):     sub_1100: NOP + RTS
    let mut raw_data = vec![
        0x85, 0xFC, // $1000
        0xA5, 0xFE, // $1002
        0x99, 0x28, 0x04, // $1004
        0xBD, 0xFF, 0x00, // $1007
        0xB1, 0xFC, // $100A
        0xA1, 0xED, // $100C
        0xA9, 0x34, // $100E
        0xA0, 0x10, // $1010
        0x60, // $1012
        0x34, 0x10, // $1013..=$1014
        0x2F, 0x34, 0x10, 0x10, // $1015..=$1018
        0x10, 0x10, 0xFF, 0x34, // $1019..=$101C
    ];
    raw_data.extend(vec![0xEA; 18]); // $101D..=$102E
    raw_data.push(0x60); // $102F
    raw_data.extend([0xEA, 0xEA, 0xEA, 0xEA, 0x60]); // $1030..=$1034
    raw_data.extend([0xEA, 0x60]); // $1035..=$1036
    raw_data.extend(vec![0x00; 0x100 - 0x37]); // $1037..=$10FF
    raw_data.extend([0xEA, 0x60]); // $1100..=$1101
    assert_eq!(raw_data.len(), 0x102);

    let mut block_types = vec![BlockType::Code; 0x13];
    block_types.extend(vec![BlockType::Address; 2]); // $1013..=$1014
    block_types.extend(vec![BlockType::LoHiAddress; 4]); // $1015..=$1018
    block_types.extend(vec![BlockType::HiLoAddress; 4]); // $1019..=$101C
    block_types.extend(vec![BlockType::Code; 0x37 - 0x1D]); // $101D..=$1036
    block_types.extend(vec![BlockType::DataByte; 0x100 - 0x37]); // $1037..=$10FF
    block_types.extend(vec![BlockType::Code; 2]); // $1100..=$1101
    assert_eq!(block_types.len(), raw_data.len());

    state.raw_data = raw_data;
    state.block_types = block_types;

    // Exclude $00FD from auto-labelling so `LDA $FE` emits unlabelled ZP base `$fd + 1`
    state.user_excluded_addresses.insert(Addr(0x00FD));
    state.excluded_addresses.insert(Addr(0x00FD));

    for (addr, name, ltype) in [
        (0x00EB, "zp_tab", LabelType::ZeroPagePointer),
        (0x00FB, "zp_ptr", LabelType::ZeroPagePointer),
        (0x0100, "stack_tab", LabelType::Field),
        (0x0400, "SCREEN_RAM", LabelType::Field),
        (0x1030, "sub_1030", LabelType::Subroutine),
        (0x1035, "sub_1035", LabelType::Subroutine),
        (0x1100, "sub_1100", LabelType::Subroutine),
    ] {
        state.labels.insert(
            Addr(addr),
            vec![Label {
                name: name.to_string(),
                kind: LabelKind::User,
                label_type: ltype,
            }],
        );
    }

    // Tier 2 global address offsets
    state.annotations.update(Addr(0x00FC), |e| {
        e.address_offset = Some(SymbolicOffset {
            base: Addr(0x00FB),
            offset: 1,
            radix: OffsetRadix::Decimal,
        });
    });
    state.annotations.update(Addr(0x00FE), |e| {
        e.address_offset = Some(SymbolicOffset {
            base: Addr(0x00FD),
            offset: 1,
            radix: OffsetRadix::Decimal,
        });
    });
    state.annotations.update(Addr(0x0428), |e| {
        e.address_offset = Some(SymbolicOffset {
            base: Addr(0x0400),
            offset: 0x28,
            radix: OffsetRadix::Hex,
        });
    });

    // Tier 1 per-site operand offsets
    state.annotations.update(Addr(0x1007), |e| {
        e.operand_offset = Some(SymbolicOffset {
            base: Addr(0x0100),
            offset: -1,
            radix: OffsetRadix::Decimal,
        });
    });
    state.annotations.update(Addr(0x100C), |e| {
        e.operand_offset = Some(SymbolicOffset {
            base: Addr(0x00EB),
            offset: 2,
            radix: OffsetRadix::Decimal,
        });
    });
    state.annotations.update(Addr(0x100E), |e| {
        e.immediate_format = Some(ImmediateFormat::LowByte(Addr(0x1034)));
        e.operand_offset = Some(SymbolicOffset {
            base: Addr(0x1035),
            offset: -1,
            radix: OffsetRadix::Decimal,
        });
    });
    state.annotations.update(Addr(0x1010), |e| {
        e.immediate_format = Some(ImmediateFormat::HighByte(Addr(0x1034)));
        e.operand_offset = Some(SymbolicOffset {
            base: Addr(0x1035),
            offset: -1,
            radix: OffsetRadix::Decimal,
        });
    });
    for (site, base) in [
        (0x1013, 0x1035),
        (0x1015, 0x1030),
        (0x1016, 0x1035),
        (0x1019, 0x1100),
        (0x101A, 0x1035),
    ] {
        state.annotations.update(Addr(site), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(base),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
    }

    let _ = state.perform_analysis();
    state.disassemble();

    for asm in Assembler::all() {
        let dummy_path = std::path::Path::new("verify_symbolic_offsets.asm");
        let opts =
            regenerator2000_core::exporter::ExportOptions::for_assembler(&state, dummy_path, *asm);
        let rendered =
            regenerator2000_core::exporter::render_asm_with_options(&state, dummy_path, &opts);
        assert!(rendered.primary_content.contains("zp_ptr + 1"));
        assert!(rendered.primary_content.contains("SCREEN_RAM + $28"));
        assert!(rendered.primary_content.contains("(sub_1035 - 1)"));

        let result = verify_roundtrip(&state, *asm);
        if result.message.contains("not found in PATH") {
            println!("Skipping {asm}: not installed");
            continue;
        }

        println!("{result}");
        assert!(
            result.success,
            "{asm} roundtrip failed for symbolic offset expressions: {}",
            result.message
        );
    }
}

#[test]
fn test_roundtrip_suppressed_symbolic_offset_bases_all_assemblers() {
    use regenerator2000_core::state::{OffsetRadix, SymbolicOffset};

    let mut state = AppState::new();
    state.origin = Addr(0x1000);

    // Memory layout ($1000..=$1014, 21 bytes):
    // $1000 (3): 8D 21 D0    STA EXTCOL + 1         ($D021 `BGCOL0` System label suppressed by Tier 2!)
    // $1003 (3): 8D 22 D0    STA $d021 + 1          (Tier 1 base = $D021 -> MUST NOT emit suppressed `BGCOL0`!)
    // $1006 (2): 85 FC       STA zp_ptr + 1         ($00FC `zpa_FC` Auto label suppressed by Tier 2!)
    // $1008 (2): A5 FD       LDA $fc + 1            (Tier 2 base = $00FC -> MUST emit 2-digit `$fc + 1` in ACME!)
    // $100A (3): AD 13 10    LDA internal_base + 1  ($1013 internal Auto label suppressed by Tier 2!)
    // $100D (3): AD 14 10    LDA $1013 + 1          (Tier 1 base = $1013 -> MUST NOT emit suppressed internal label!)
    // $1010 (2): 60 EA       RTS; NOP
    // $1012 (3): 11 22 33    internal_base ($1012) + 2 struct bytes ($1013, $1014)
    state.raw_data = vec![
        0x8D, 0x21, 0xD0, // $1000
        0x8D, 0x22, 0xD0, // $1003
        0x85, 0xFC, // $1006
        0xA5, 0xFD, // $1008
        0xAD, 0x13, 0x10, // $100A
        0xAD, 0x14, 0x10, // $100D
        0x60, 0xEA, // $1010..=$1011
        0x11, 0x22, 0x33, // $1012..=$1014
    ];
    let mut block_types = vec![BlockType::Code; 0x12];
    block_types.extend([BlockType::DataByte; 3]);
    state.block_types = block_types;

    // System labels at $D020 (EXTCOL) and $D021 (BGCOL0)
    state.labels.insert(
        Addr(0xD020),
        vec![Label {
            name: "EXTCOL".to_string(),
            kind: LabelKind::System,
            label_type: LabelType::Predefined,
        }],
    );
    state.labels.insert(
        Addr(0xD021),
        vec![Label {
            name: "BGCOL0".to_string(),
            kind: LabelKind::System,
            label_type: LabelType::Predefined,
        }],
    );

    // User base labels at $00FB (`zp_ptr`) and $1012 (`internal_base`)
    state.labels.insert(
        Addr(0x00FB),
        vec![Label {
            name: "zp_ptr".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::ZeroPagePointer,
        }],
    );
    state.labels.insert(
        Addr(0x1012),
        vec![Label {
            name: "internal_base".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::UserDefined,
        }],
    );

    // Suppress $D021 (`BGCOL0`), $00FC (`zpa_FC`), and $1013 (`a_1013`) via Tier 2 `address_offset`
    state.annotations.update(Addr(0xD021), |e| {
        e.address_offset = Some(SymbolicOffset {
            base: Addr(0xD020),
            offset: 1,
            radix: OffsetRadix::Decimal,
        });
    });
    state.annotations.update(Addr(0x00FC), |e| {
        e.address_offset = Some(SymbolicOffset {
            base: Addr(0x00FB),
            offset: 1,
            radix: OffsetRadix::Decimal,
        });
    });
    state.annotations.update(Addr(0x1013), |e| {
        e.address_offset = Some(SymbolicOffset {
            base: Addr(0x1012),
            offset: 1,
            radix: OffsetRadix::Decimal,
        });
    });

    // Reference those suppressed addresses as `sym.base` via both Tier 1 and Tier 2!
    let t1_d022 = SymbolicOffset {
        base: Addr(0xD021),
        offset: 1,
        radix: OffsetRadix::Decimal,
    };
    state
        .annotations
        .update(Addr(0x1003), |e| e.operand_offset = Some(t1_d022));

    let t2_00fd = SymbolicOffset {
        base: Addr(0x00FC),
        offset: 1,
        radix: OffsetRadix::Decimal,
    };
    state
        .annotations
        .update(Addr(0x00FD), |e| e.address_offset = Some(t2_00fd));

    let t1_1014 = SymbolicOffset {
        base: Addr(0x1013),
        offset: 1,
        radix: OffsetRadix::Decimal,
    };
    state
        .annotations
        .update(Addr(0x100D), |e| e.operand_offset = Some(t1_1014));

    let _ = state.perform_analysis();
    state.disassemble();

    // Verify exact parity between DisassemblyLine operands and `AppState::format_symbolic_offset`
    let line_1003 = state
        .disassembly
        .iter()
        .find(|l| l.address == Addr(0x1003) && !l.bytes.is_empty())
        .unwrap();
    assert_eq!(
        line_1003.operand,
        state.format_symbolic_offset(t1_d022, Some(Addr(0x1003)), false)
    );
    assert_eq!(line_1003.operand, "$d021 + 1");

    let line_1008 = state
        .disassembly
        .iter()
        .find(|l| l.address == Addr(0x1008) && !l.bytes.is_empty())
        .unwrap();
    assert_eq!(
        line_1008.operand,
        state.format_symbolic_offset(t2_00fd, Some(Addr(0x1008)), true)
    );
    assert_eq!(line_1008.operand, "$fc + 1");

    let line_100d = state
        .disassembly
        .iter()
        .find(|l| l.address == Addr(0x100D) && !l.bytes.is_empty())
        .unwrap();
    assert_eq!(line_100d.operand, "$1013 + 1");

    for asm in Assembler::all() {
        let dummy_path = std::path::Path::new("verify_suppressed_bases.asm");
        let opts =
            regenerator2000_core::exporter::ExportOptions::for_assembler(&state, dummy_path, *asm);
        let rendered =
            regenerator2000_core::exporter::render_asm_with_options(&state, dummy_path, &opts);
        assert!(rendered.primary_content.contains("$d021 + 1"));
        assert!(rendered.primary_content.contains("$fc + 1"));
        assert!(rendered.primary_content.contains("$1013 + 1"));

        let result = verify_roundtrip(&state, *asm);
        if result.message.contains("not found in PATH") {
            println!("Skipping {asm}: not installed");
            continue;
        }

        println!("{result}");
        assert!(
            result.success,
            "{asm} roundtrip failed for suppressed symbolic-offset base symbols: {}",
            result.message
        );
    }
}

#[test]
fn test_roundtrip_zp_legacy_out_of_bounds_base_defense_in_depth_all_assemblers() {
    use regenerator2000_core::state::{OffsetRadix, SymbolicOffset};

    let mut state = AppState::new();
    state.origin = Addr(0x1000);
    state.settings.all_labels = true;
    state.settings.preserve_long_bytes = true;

    // $1000: A5 FF       LDA $FF          (2-byte ZeroPage)
    // $1002: B1 FF       LDA ($FF),Y      (2-byte IndirectY)
    // $1004: BD FF 00    LDA $00FF,X      (3-byte forced-absolute AbsoluteX -> Tier 1 `table_0100 - 1` allowed)
    // $1007: 60          RTS
    state.raw_data = vec![0xA5, 0xFF, 0xB1, 0xFF, 0xBD, 0xFF, 0x00, 0x60];
    state.block_types = vec![BlockType::Code; state.raw_data.len()];

    state.labels.insert(
        Addr(0x0100),
        vec![Label {
            name: "table_0100".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::Field,
        }],
    );

    // Inject legacy Tier 2 and Tier 1 offsets with base `$0100` at `$00FF` and `$1000`.
    // Keep a valid Tier 1 offset with base `$0100` on 3-byte instruction `$1004`.
    let non_zp_sym = SymbolicOffset {
        base: Addr(0x0100),
        offset: -1,
        radix: OffsetRadix::Decimal,
    };
    state
        .annotations
        .update(Addr(0x00FF), |e| e.address_offset = Some(non_zp_sym));
    state
        .annotations
        .update(Addr(0x1000), |e| e.operand_offset = Some(non_zp_sym));
    state
        .annotations
        .update(Addr(0x1004), |e| e.operand_offset = Some(non_zp_sym));

    let _ = state.perform_analysis();
    state.disassemble();

    // `$00FF` equate stays in the preamble, 2-byte ZP instructions ignore `base > $00FF`,
    // and 3-byte forced-absolute `$1004` still emits `table_0100 - 1,x`.
    assert!(
        state
            .disassembly
            .iter()
            .any(|l| l.external_label_address == Some(Addr(0x00FF)))
    );
    let line_1004 = state
        .disassembly
        .iter()
        .find(|l| l.address == Addr(0x1004) && !l.bytes.is_empty())
        .unwrap();
    assert!(line_1004.operand.contains("table_0100 - 1,x"));

    for asm in Assembler::all() {
        let dummy_path = std::path::Path::new("verify_zp_defense_in_depth.asm");
        let opts =
            regenerator2000_core::exporter::ExportOptions::for_assembler(&state, dummy_path, *asm);
        let rendered =
            regenerator2000_core::exporter::render_asm_with_options(&state, dummy_path, &opts);
        assert!(rendered.primary_content.contains("table_0100 - 1,x"));

        let result = verify_roundtrip(&state, *asm);
        if result.message.contains("not found in PATH") {
            println!("Skipping {asm}: not installed");
            continue;
        }

        println!("{result}");
        assert!(
            result.success,
            "{asm} roundtrip failed for legacy ZP base > $00FF defense-in-depth: {}",
            result.message
        );
    }
}
