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

    // 1000-1006 are the first function.
    state.scopes.insert(Addr(0x1000), Addr(0x1006));
    state.splitters.insert(Addr(0x1007));

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

    // Check entry point is in .proc (now index 1 due to virtual splitter at start)
    assert_eq!(disasm[0].mnemonic, "{splitter}");
    assert_eq!(disasm[1].label, Some("s1000".to_string()));
    assert_eq!(disasm[1].mnemonic, ".block");

    // Check instruction has NO label (suppressed)
    assert_eq!(disasm[2].label, None);

    // Check local labels
    assert_eq!(disasm[3].label, Some("l00".to_string()));
    assert_eq!(disasm[4].label, Some("l01".to_string()));

    // Check operands use local labels
    assert_eq!(disasm[3].operand, "l00");
    assert_eq!(disasm[4].operand, "l01");

    // Check .pend
    assert_eq!(disasm[6].mnemonic, ".bend");

    // Check splitter
    assert!(state.splitters.contains(&Addr(0x1007)));
    assert_eq!(disasm[7].mnemonic, "{splitter}");

    // Check second function (not a routine)
    assert_eq!(disasm[8].label, Some("s1007".to_string()));
    assert_eq!(disasm[9].operand, "s1007");
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

    // Mark 1000-1004 as Scope
    state.scopes.insert(Addr(0x1000), Addr(0x1004));
    state.splitters.insert(Addr(0x1005));
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

    assert_eq!(disasm[0].mnemonic, "{splitter}");
    assert_eq!(disasm[1].label, Some("s1000".to_string()));
    assert_eq!(disasm[3].label, Some("l00".to_string()));
    assert_eq!(disasm[3].operand, "l00");
    assert_eq!(disasm[5].mnemonic, ".bend");
    assert_eq!(disasm[6].mnemonic, "{splitter}");
    assert_eq!(disasm[7].operand, "s1000.l00");
}

#[test]
fn test_routine_split_by_bytes() {
    let mut state = AppState::new();
    state.origin = Addr(0x1000);
    state.settings.assembler = Assembler::Tass64;

    // Setup code:
    // $1000: RTS
    // $1001: JMP $1050  (Jump to 3rd block)
    // $1004..$103F: NOPs
    // $1040..$104F: DATA BYTES
    // $1050: NOP (referenced from top)
    // $1051: RTS

    let mut data = vec![0xEA; 0x80]; // 128 bytes
    data[0] = 0x60; // RTS
    data[1] = 0x4C; // JMP $1050
    data[2] = 0x50;
    data[3] = 0x10;

    // 3rd block start at 1050
    data[0x50] = 0xEA; // NOP
    data[0x51] = 0xD0; // BNE $1055 (+2 bytes from 1053-ish? no, offset from 1051 + 2 = 1053. 1053+2=1055)
    data[0x52] = 0x02; // BNE +2 -> to 1055
    data[0x53] = 0xEA; // NOP
    data[0x54] = 0xEA; // NOP
    data[0x55] = 0x60; // RTS at 1055

    state.raw_data = data;
    state.block_types = vec![BlockType::Code; state.raw_data.len()];
    state.disassemble();

    // Set range as Scope initially
    let start_addr = Addr(0x1000);
    let end_addr = Addr(0x1000 + state.raw_data.len() as u16 - 1);
    state.scopes.insert(start_addr, end_addr);

    // Split with Bytes at $1040-$104F (offset 0x40 to 0x4F)
    let byte_start_idx = state
        .get_line_index_containing_address(Addr(0x1040))
        .unwrap();
    let byte_end_idx = state
        .get_line_index_containing_address(Addr(0x104F))
        .unwrap();
    state.set_block_type_region(BlockType::DataByte, Some(byte_start_idx), byte_end_idx);

    // Add an explicit label at $1000 that is a Subroutine to start the scope
    state.labels.insert(
        Addr(0x1000),
        vec![regenerator2000_core::state::Label {
            name: "s1000".to_string(),
            label_type: regenerator2000_core::state::LabelType::Subroutine,
            kind: regenerator2000_core::state::LabelKind::User,
        }],
    );

    // Re-analyze
    let result = regenerator2000_core::analyzer::analyze(&state);
    state.labels = result.labels;
    state.cross_refs = result.cross_refs;

    state.disassemble();

    // Print disassembly for debugging
    for (i, line) in state.disassembly.iter().enumerate() {
        println!(
            "{}: ${:04X} {} {} ; {}",
            i, line.address.0, line.mnemonic, line.operand, line.comment
        );
    }

    // We expect a label at $1050, and it SHOULD be local if possible,
    // OR if it's non-local, it should be formatted correctly.
    // The user says it loses local symbols and .proc preamble.
    // Let's assert the presence of .proc just before index holding 1050.

    let line_1050_idx = state
        .disassembly
        .iter()
        .position(|l| l.address == Addr(0x1050))
        .unwrap();

    // Verify .proc was emitted AT line $1000, not 1050 because they are bridged!
    let proc_line_1050 = &state.disassembly[line_1050_idx];
    assert_eq!(proc_line_1050.mnemonic, "nop");

    // The entire thing is one scope from 1000..=1055
    let line_1000_idx = state
        .disassembly
        .iter()
        .position(|l| l.address == Addr(0x1000) && l.mnemonic == ".block")
        .unwrap();
    let proc_line_1000 = &state.disassembly[line_1000_idx];
    assert_eq!(proc_line_1000.mnemonic, ".block");

    // Check if $1055 is local
    let line_1055 = state
        .disassembly
        .iter()
        .find(|l| l.address == Addr(0x1055) && !l.bytes.is_empty())
        .unwrap();
    assert!(
        line_1055.label.as_ref().unwrap().starts_with("l"),
        "Label should be local, got {:?}",
        line_1055.label
    );

    // Check if branch at $1051 uses local label
    let line_1051 = state
        .disassembly
        .iter()
        .find(|l| l.address == Addr(0x1051))
        .unwrap();
    assert_eq!(line_1051.operand, "l01"); // Second local label in the merged scope (1050 is l00, 1055 is l01)
}

#[test]
fn test_routine_block_local_symbols_ca65() {
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

    // 1000-1006 are the first function.
    state.scopes.insert(Addr(0x1000), Addr(0x1006));
    state.splitters.insert(Addr(0x1007));

    state.settings.assembler = Assembler::Ca65;

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

    // Check entry point is in .proc (now index 1 due to virtual splitter at start)
    assert_eq!(disasm[0].mnemonic, "{splitter}");
    assert_eq!(disasm[1].label, None); // ca65 emits .proc with operand
    assert_eq!(disasm[1].mnemonic, ".proc");
    assert_eq!(disasm[1].operand, "s1000");

    // Check instruction has NO label (suppressed)
    assert_eq!(disasm[2].label, None);

    // Check local labels
    assert_eq!(disasm[3].label, Some("l00".to_string()));
    assert_eq!(disasm[4].label, Some("l01".to_string()));

    // Check operands use local labels
    assert_eq!(disasm[3].operand, "l00");
    assert_eq!(disasm[4].operand, "l01");

    // Check .endproc
    assert_eq!(disasm[6].mnemonic, ".endproc");

    // Check splitter
    assert!(state.splitters.contains(&Addr(0x1007)));
    assert_eq!(disasm[7].mnemonic, "{splitter}");

    // Check second function (not a routine)
    assert_eq!(disasm[8].label, Some("s1007".to_string()));
    assert_eq!(disasm[9].operand, "s1007");
}

#[test]
fn test_routine_block_local_symbols_kickasm() {
    let mut state = AppState::new();
    state.origin = Addr(0x1000);
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

    // 1000-1006 are the first function.
    state.scopes.insert(Addr(0x1000), Addr(0x1006));
    state.splitters.insert(Addr(0x1007));

    state.settings.assembler = Assembler::Kick;

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

    // Check entry point is in scope (now index 1 due to virtual splitter at start)
    assert_eq!(disasm[0].mnemonic, "{splitter}");
    assert_eq!(disasm[1].label, Some("s1000".to_string()));
    assert_eq!(disasm[1].mnemonic, "{");

    // Check instruction has NO label (suppressed)
    assert_eq!(disasm[2].label, None);

    // Check local labels
    assert_eq!(disasm[3].label, Some("l00".to_string()));
    assert_eq!(disasm[4].label, Some("l01".to_string()));

    // Check operands use local labels
    assert_eq!(disasm[3].operand, "l00");
    assert_eq!(disasm[4].operand, "l01");

    // Check }
    assert_eq!(disasm[6].mnemonic, "}");

    // Check splitter
    assert!(state.splitters.contains(&Addr(0x1007)));
    assert_eq!(disasm[7].mnemonic, "{splitter}");

    // Check second function (not a routine)
    assert_eq!(disasm[8].label, Some("s1007".to_string()));
    assert_eq!(disasm[9].operand, "s1007");
}
