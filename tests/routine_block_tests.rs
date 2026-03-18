#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use regenerator2000_core::state::{Addr, AppState, Assembler, BlockType};

#[test]
fn test_routine_block_local_symbols_64tass() {
    let mut state = AppState::new();
    state.origin = Addr(0x1000);
    // 1000: s1000: lda #$00
    // 1002: b1002: bne b1002
    // 1004: b1004: bne b1004
    // 1006: rts
    // 1007: s1007: lda #$01
    // 1009: b1009: bne b1007
    // 100b: rts
    state.raw_data = vec![
        0xA9, 0x00, // 1000: LDA #$00
        0xD0, 0xFE, // 1002: BNE $1002
        0xD0, 0xFE, // 1004: BNE $1004
        0x60, // 1006: RTS
        0xA9, 0x01, // 1007: LDA #$01
        0xD0, 0xFC, // 1009: BNE $1007
        0x60, // 100B: RTS
    ];
    state.block_types = vec![BlockType::Code; state.raw_data.len()];
    state.disassemble(); // Initial disassembly for set_block_type_region to work

    // Mark the first function as a Routine using set_block_type_region to test auto-splitter
    // 1000-1006 are the first function.
    // In initial disassembly: 0=1000, 1=1002, 2=1004, 3=1006.
    state.set_block_type_region(BlockType::Routine, Some(0), 3);

    state.settings.assembler = Assembler::Tass64;

    // First analysis to generate auto labels
    let result = regenerator2000_core::analyzer::analyze(&state);
    state.labels = result.labels;
    state.cross_refs = result.cross_refs;

    // Add manual label for entry point to avoid None
    state.labels.insert(
        Addr(0x1000),
        vec![regenerator2000_core::state::Label {
            name: "s1000".to_string(),
            label_type: regenerator2000_core::state::LabelType::Subroutine,
            kind: regenerator2000_core::state::LabelKind::Auto,
        }],
    );
    state.labels.insert(
        Addr(0x1007),
        vec![regenerator2000_core::state::Label {
            name: "s1007".to_string(),
            label_type: regenerator2000_core::state::LabelType::Subroutine,
            kind: regenerator2000_core::state::LabelKind::Auto,
        }],
    );

    state.disassemble();
    let disasm = &state.disassembly;

    // 0: s1000 .proc    (1000)
    // 1:       lda #$00 (1000)
    // 2: _l00  bne _l00 (1002)
    // 3: _l01  bne _l01 (1004)
    // 4:       rts      (1006)
    // 5:       .pend    (1006)
    // 6:       --- splitter --- (1007)
    // 7: s1007 lda #$01 (1007)
    // 8:       bne s1007 (1009)
    // 9:       rts      (100B)

    // Check entry point is in .proc
    assert_eq!(disasm[0].label, Some("s1000".to_string()));
    assert_eq!(disasm[0].mnemonic, ".proc");

    // Check instruction has NO label (suppressed)
    assert_eq!(disasm[1].label, None);

    // Check local labels (index shifted by 1)
    assert_eq!(disasm[2].label, Some("_l00".to_string()));
    assert_eq!(disasm[3].label, Some("_l01".to_string()));

    // Check operands use local labels
    assert_eq!(disasm[2].operand, "_l00");
    assert_eq!(disasm[3].operand, "_l01");

    // Check .pend
    assert_eq!(disasm[5].mnemonic, ".pend");

    // Check splitter
    assert!(state.splitters.contains(&Addr(0x1007)));
    assert_eq!(disasm[6].mnemonic, "{splitter}");

    // Check second function (not a routine)
    assert_eq!(disasm[7].label, Some("s1007".to_string()));
    assert_eq!(disasm[8].operand, "s1007");
}

#[test]
fn test_routine_block_local_referenced_from_outside() {
    let mut state = AppState::new();
    state.origin = Addr(0x1000);
    // 1000: s1000: lda #$00
    // 1002: b1002: bne b1002
    // 1004: rts
    // 1005: jmp b1002
    state.raw_data = vec![
        0xA9, 0x00, // 1000: LDA #$00
        0xD0, 0xFE, // 1002: BNE $1002
        0x60, // 1004: RTS
        0x4C, 0x02, 0x10, // 1005: JMP $1002
    ];
    state.block_types = vec![BlockType::Code; state.raw_data.len()];
    state.disassemble();

    // Mark 1000-1004 as Routine (0,1,2 in disassembly)
    state.set_block_type_region(BlockType::Routine, Some(0), 2);
    state.settings.assembler = Assembler::Tass64;

    let result = regenerator2000_core::analyzer::analyze(&state);
    state.labels = result.labels;
    state.cross_refs = result.cross_refs;

    state.labels.insert(
        Addr(0x1000),
        vec![regenerator2000_core::state::Label {
            name: "s1000".to_string(),
            label_type: regenerator2000_core::state::LabelType::Subroutine,
            kind: regenerator2000_core::state::LabelKind::Auto,
        }],
    );

    state.disassemble();
    let disasm = &state.disassembly;

    // $1002 is referenced from $1005 (outside the routine scope [1000-1004])
    // So it should NOT be local.
    // Indexes:
    // 0: s1000 .proc    (1000)
    // 1:       lda #$00 (1000)
    // 2: b1002 bne b1002 (1002)
    // 3:       rts      (1004)
    // 4:       .pend    (1004)
    // 5:       --- splitter --- (1005)
    // 6:       jmp b1002 (1005)

    assert_eq!(disasm[0].label, Some("s1000".to_string()));
    assert_eq!(disasm[2].label, Some("b1002".to_string()));
    assert_eq!(disasm[2].operand, "b1002");
    assert_eq!(disasm[4].mnemonic, ".pend");
    assert_eq!(disasm[5].mnemonic, "{splitter}");
    assert_eq!(disasm[6].operand, "s1000.b1002");
}
