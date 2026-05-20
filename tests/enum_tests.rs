#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use regenerator2000_core::state::{Addr, AppState, EnumDefinition, ProjectState};
use std::collections::BTreeMap;

#[test]
fn test_enum_definition_parsing() {
    let mut variants = BTreeMap::new();
    variants.insert("0x00".to_string(), "BLACK".to_string());
    variants.insert("1".to_string(), "WHITE".to_string());
    variants.insert("$02".to_string(), "RED".to_string());
    variants.insert("%00000011".to_string(), "CYAN".to_string());

    let raw_def = regenerator2000_core::state::RawEnumDefinition {
        name: "Colors".to_string(),
        description: None,
        variants,
    };

    let parsed_variants = EnumDefinition::parse_variants(raw_def.variants);
    let parsed = EnumDefinition {
        name: raw_def.name,
        description: None,
        variants: parsed_variants,
    };
    assert_eq!(parsed.name, "Colors");
    assert_eq!(parsed.variants.get(&0).unwrap(), "BLACK");
    assert_eq!(parsed.variants.get(&1).unwrap(), "WHITE");
    assert_eq!(parsed.variants.get(&2).unwrap(), "RED");
    assert_eq!(parsed.variants.get(&3).unwrap(), "CYAN");
}

#[test]
fn test_enum_resolution_and_precedence() {
    let mut app_state = AppState::new();

    // 1. Define color enum in Built-in Pool
    let mut builtin_variants = BTreeMap::new();
    builtin_variants.insert(0, "BUILTIN_BLACK".to_string());
    builtin_variants.insert(1, "BUILTIN_WHITE".to_string());
    let builtin_enum = EnumDefinition {
        name: "Colors".to_string(),
        description: None,
        variants: builtin_variants,
    };
    app_state
        .builtin_enums
        .insert("Colors".to_string(), builtin_enum);

    // Apply enum "Colors" to address $1000
    app_state
        .enum_usages
        .insert(Addr(0x1000), "Colors".to_string());

    // Should resolve using Built-in definition
    assert_eq!(
        app_state.resolve_enum_value(Addr(0x1000), 0).unwrap(),
        "BUILTIN_BLACK"
    );
    assert_eq!(
        app_state.resolve_enum_value(Addr(0x1000), 1).unwrap(),
        "BUILTIN_WHITE"
    );

    // 2. Now define color enum in Global Pool (shadowing built-in)
    let mut global_variants = BTreeMap::new();
    global_variants.insert(0, "GLOBAL_BLACK".to_string());
    global_variants.insert(2, "GLOBAL_RED".to_string());
    let global_enum = EnumDefinition {
        name: "Colors".to_string(),
        description: None,
        variants: global_variants,
    };
    app_state
        .user_global_enums
        .insert("Colors".to_string(), global_enum);

    // Global shadows Built-in:
    assert_eq!(
        app_state.resolve_enum_value(Addr(0x1000), 0).unwrap(),
        "GLOBAL_BLACK"
    );
    assert_eq!(
        app_state.resolve_enum_value(Addr(0x1000), 2).unwrap(),
        "GLOBAL_RED"
    );
    assert!(app_state.resolve_enum_value(Addr(0x1000), 1).is_none()); // Shadowed!

    // 3. Now define color enum in Project Pool (shadowing global)
    let mut project_variants = BTreeMap::new();
    project_variants.insert(0, "PROJECT_BLACK".to_string());
    project_variants.insert(3, "PROJECT_CYAN".to_string());
    let project_enum = EnumDefinition {
        name: "Colors".to_string(),
        description: None,
        variants: project_variants,
    };
    app_state.enums.insert("Colors".to_string(), project_enum);

    // Project shadows Global:
    assert_eq!(
        app_state.resolve_enum_value(Addr(0x1000), 0).unwrap(),
        "PROJECT_BLACK"
    );
    assert_eq!(
        app_state.resolve_enum_value(Addr(0x1000), 3).unwrap(),
        "PROJECT_CYAN"
    );
    assert!(app_state.resolve_enum_value(Addr(0x1000), 2).is_none()); // Shadowed!
}

#[test]
fn test_enum_embedding_on_command_apply() {
    let mut app_state = AppState::new();

    // Add an enum "VicIIColors" to Builtin Pool
    let mut builtin_variants = BTreeMap::new();
    builtin_variants.insert(0, "BLACK".to_string());
    let builtin_enum = EnumDefinition {
        name: "VicIIColors".to_string(),
        description: None,
        variants: builtin_variants,
    };
    app_state
        .builtin_enums
        .insert("VicIIColors".to_string(), builtin_enum);

    // Verify "VicIIColors" is NOT in project enums initially
    assert!(!app_state.enums.contains_key("VicIIColors"));

    // Dispatch Command to use "VicIIColors"
    let cmd = regenerator2000_core::commands::Command::SetEnumUsage {
        address: Addr(0x1000),
        new_enum: Some("VicIIColors".to_string()),
        old_enum: None,
    };
    cmd.apply(&mut app_state);

    // Verify "VicIIColors" is STILL NOT in project enums (cloned) automatically!
    assert!(!app_state.enums.contains_key("VicIIColors"));

    // But verify that we can STILL resolve the enum value successfully!
    assert_eq!(
        app_state.resolve_enum_value(Addr(0x1000), 0).unwrap(),
        "BLACK"
    );
}

#[test]
fn test_enum_project_roundtrip_serialization() {
    use regenerator2000_core::state::{
        Block, BlockType, DocumentSettings, encode_raw_data_to_base64,
    };

    let raw_bytes: Vec<u8> = vec![0xEA];
    let raw_data_base64 = encode_raw_data_to_base64(&raw_bytes).unwrap();

    // Create enum def
    let mut variants = BTreeMap::new();
    variants.insert(0, "BLACK".to_string());
    let enum_def = EnumDefinition {
        name: "Colors".to_string(),
        description: None,
        variants,
    };

    let mut enums = BTreeMap::new();
    enums.insert("Colors".to_string(), enum_def);

    let mut enum_usages = BTreeMap::new();
    enum_usages.insert(Addr(0x1000), "Colors".to_string());

    let project = ProjectState {
        version: 1,
        origin: Addr(0x1000),
        raw_data: raw_data_base64,
        blocks: vec![Block {
            start: 0,
            end: 0,
            type_: BlockType::Code,
            collapsed: false,
        }],
        labels: BTreeMap::new(),
        user_side_comments: BTreeMap::new(),
        user_line_comments: BTreeMap::new(),
        immediate_value_formats: BTreeMap::new(),
        settings: DocumentSettings::default(),
        cursor_address: None,
        hex_dump_cursor_address: None,
        sprites_cursor_address: None,
        charset_cursor_address: None,
        bitmap_cursor_address: None,
        right_pane_visible: None,
        sprite_multicolor_mode: false,
        charset_multicolor_mode: false,
        bitmap_multicolor_mode: false,
        hexdump_view_mode: regenerator2000_core::state::HexdumpViewMode::default(),
        splitters: std::collections::BTreeSet::new(),
        blocks_view_cursor: None,
        bookmarks: BTreeMap::new(),
        scopes: BTreeMap::new(),
        user_excluded_addresses: std::collections::BTreeSet::new(),
        enums,
        enum_usages,
    };

    // Serialize
    let json = serde_json::to_string(&project).unwrap();

    // Deserialize
    let deserialized: ProjectState = serde_json::from_str(&json).unwrap();
    assert!(deserialized.enums.contains_key("Colors"));
    assert_eq!(
        deserialized
            .enums
            .get("Colors")
            .unwrap()
            .variants
            .get(&0)
            .unwrap(),
        "BLACK"
    );
    assert_eq!(
        deserialized.enum_usages.get(&Addr(0x1000)).unwrap(),
        "Colors"
    );
}

#[test]
fn test_assembler_enum_formatting() {
    use regenerator2000_core::disassembler::Disassembler;
    use regenerator2000_core::state::Assembler;

    let mut variants = BTreeMap::new();
    variants.insert(0, "BLACK".to_string());
    variants.insert(1, "WHITE".to_string());
    let enum_def = EnumDefinition {
        name: "Colors".to_string(),
        description: None,
        variants,
    };

    // --- 1. 64tass ---
    let tass = Disassembler::create_formatter(Assembler::Tass64);
    assert_eq!(
        tass.format_enum_reference("Colors", "BLACK"),
        "Colors.BLACK"
    );
    let tass_block = tass.format_enum_definition(&enum_def);
    assert_eq!(tass_block, "Colors = {\n    BLACK: $00,\n    WHITE: $01\n}");

    // --- 2. KickAssembler ---
    let kick = Disassembler::create_formatter(Assembler::Kick);
    assert_eq!(
        kick.format_enum_reference("Colors", "BLACK"),
        "Colors.BLACK"
    );
    let kick_block = kick.format_enum_definition(&enum_def);
    assert_eq!(
        kick_block,
        ".enum Colors {\n    BLACK = $00,\n    WHITE = $01\n}"
    );

    // --- 3. ca65 ---
    let ca65 = Disassembler::create_formatter(Assembler::Ca65);
    assert_eq!(
        ca65.format_enum_reference("Colors", "BLACK"),
        "Colors::BLACK"
    );
    let ca65_block = ca65.format_enum_definition(&enum_def);
    assert_eq!(
        ca65_block,
        ".enum Colors\n    BLACK = $00\n    WHITE = $01\n.endenum"
    );

    // --- 4. ACME ---
    let acme = Disassembler::create_formatter(Assembler::Acme);
    assert_eq!(
        acme.format_enum_reference("Colors", "BLACK"),
        "Colors_BLACK"
    );
    let acme_block = acme.format_enum_definition(&enum_def);
    assert_eq!(acme_block, "Colors_BLACK = $00\nColors_WHITE = $01");
}

#[test]
fn test_disassembly_enum_operand_formatting() {
    use regenerator2000_core::state::{Assembler, BlockType};

    let mut app_state = AppState::new();
    app_state.origin = Addr(0x1000);
    app_state.raw_data = vec![0xA9, 0x00]; // LDA #$00
    app_state.block_types = vec![BlockType::Code; 2];

    // Create enum def in AppState
    let mut variants = BTreeMap::new();
    variants.insert(0, "BLACK".to_string());
    let enum_def = EnumDefinition {
        name: "Colors".to_string(),
        description: None,
        variants,
    };
    app_state.enums.insert("Colors".to_string(), enum_def);

    // Apply enum to address $1000
    app_state
        .enum_usages
        .insert(Addr(0x1000), "Colors".to_string());

    // --- 1. 64tass ---
    app_state.settings.assembler = Assembler::Tass64;
    app_state.disassemble();
    assert_eq!(app_state.disassembly[0].mnemonic, "lda");
    assert_eq!(app_state.disassembly[0].operand, "#Colors.BLACK");

    // --- 2. ca65 ---
    app_state.settings.assembler = Assembler::Ca65;
    app_state.disassemble();
    assert_eq!(app_state.disassembly[0].mnemonic, "lda");
    assert_eq!(app_state.disassembly[0].operand, "#Colors::BLACK");
}

#[test]
fn test_disassembly_data_enum_formatting() {
    use regenerator2000_core::state::{Assembler, BlockType};

    let mut app_state = AppState::new();
    app_state.origin = Addr(0x1000);
    app_state.raw_data = vec![0x00, 0x00, 0x01]; // .byte $00, $00 (word low), $01 (word high)
    app_state.block_types = vec![
        BlockType::DataByte,
        BlockType::DataWord,
        BlockType::DataWord,
    ];

    let mut variants = BTreeMap::new();
    variants.insert(0, "BLACK".to_string());
    variants.insert(256, "WHITE_WORD".to_string()); // $0100 = 256
    let enum_def = EnumDefinition {
        name: "Colors".to_string(),
        description: None,
        variants,
    };
    app_state.enums.insert("Colors".to_string(), enum_def);

    // Apply enum to data byte at $1000 and word at $1001
    app_state
        .enum_usages
        .insert(Addr(0x1000), "Colors".to_string());
    app_state
        .enum_usages
        .insert(Addr(0x1001), "Colors".to_string());

    // --- Test ca65 ---
    app_state.settings.assembler = Assembler::Ca65;
    app_state.disassemble();

    // ca65 byte
    assert_eq!(app_state.disassembly[0].mnemonic, ".byte");
    assert_eq!(app_state.disassembly[0].operand, "Colors::BLACK");

    // ca65 word
    assert_eq!(app_state.disassembly[1].mnemonic, ".word");
    assert_eq!(app_state.disassembly[1].operand, "Colors::WHITE_WORD");
}
