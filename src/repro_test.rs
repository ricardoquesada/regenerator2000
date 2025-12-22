#[test]
fn test_absolute_addressing_on_zp_formatting() {
    let mut app_state = AppState::new();
    // 8D A0 00 -> STA $00A0 (Answer to Life, Universe, and Everything)
    // Absolute addressing mode targeting a ZP address.
    app_state.raw_data = vec![0x8D, 0xA0, 0x00];
    app_state.origin = 0x1000;
    app_state.address_types = vec![AddressType::Code; 3];
    // Fill opcodes
    app_state.disassembler.opcodes[0x8D] = Some(crate::cpu::Opcode {
        mnemonic: "STA".to_string(),
        mode: AddressingMode::Absolute,
        size: 3,
        cycles: 4,
    });

    let labels_map = analyze(&app_state);
    let labels = labels_map.get(&0x00A0);
    assert!(labels.is_some(), "Should have a label at $00A0");
    let label = labels.unwrap().first().unwrap();

    // User wants "a00A0" because it was forced absolute / accessed absolutely.
    // Current bug: "aA0"
    assert_eq!(label.name, "a00A0");
}
