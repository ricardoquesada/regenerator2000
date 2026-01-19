#[cfg(test)]
mod tests {
    use regenerator2000::state::{AppState, BlockType, ProjectSaveContext};

    #[test]
    fn test_save_and_restore_cursor() {
        // 1. Setup initial state
        let mut app_state = AppState::new();
        app_state.origin = 0x1000;
        let start_cursor_addr = 0x1005;
        let start_hex_cursor_addr = 0x1010;

        // Dummy raw data
        let raw_bytes: Vec<u8> = vec![0xEA; 32]; // Enough bytes for a few rows
        app_state.raw_data = raw_bytes.clone();
        app_state.block_types = vec![BlockType::Code; 32];
        app_state.disassemble(); // Must disassemble to populate disassembly for save

        // 2. Save project with specific cursor
        let mut path = std::env::temp_dir();
        path.push("test_cursor_persist.regen2000proj");
        app_state.project_path = Some(path.clone());

        app_state
            .save_project(
                ProjectSaveContext {
                    cursor_address: Some(start_cursor_addr),
                    hex_dump_cursor_address: Some(start_hex_cursor_addr),
                    sprites_cursor_address: None,
                    right_pane_visible: None,
                    charset_cursor_address: None,
                    sprite_multicolor_mode: false,
                    charset_multicolor_mode: false,
                    petscii_mode: regenerator2000::state::PetsciiMode::default(),
                    splitters: std::collections::BTreeSet::new(),
                    blocks_view_cursor: None,
                },
                false,
            )
            .expect("Failed to save project");

        // 3. Create fresh app state and load
        let mut loaded_state = AppState::new();
        let loaded_data = loaded_state
            .load_project(path.clone())
            .expect("Failed to load project");
        let loaded_cursor = loaded_data.cursor_address;
        let loaded_hex_cursor = loaded_data.hex_dump_cursor_address;

        // 4. Verify cursor address is returned
        assert_eq!(
            loaded_cursor,
            Some(start_cursor_addr),
            "Cursor address should be restored"
        );

        assert_eq!(
            loaded_hex_cursor,
            Some(start_hex_cursor_addr),
            "Hex cursor address should be restored"
        );

        // 5. Test loading legacy project (without cursor_address)
        // Manually create JSON without cursor_address (or hex_cursor_address)
        let legacy_raw_data =
            regenerator2000::state::encode_raw_data_to_base64(&raw_bytes).unwrap();
        let json = format!(
            r#"{{
            "origin": 4096,
            "raw_data_base64": "{}",
            "blocks": [],
            "labels": {{}},
            "settings": {{
                "all_labels": false,
                "use_w_prefix": true,
                "brk_single_byte": false,
                "patch_brk": false,
                "platform": "Commodore64",
                "assembler": "Tass64",
                "max_xref_count": 5
            }}
        }}"#,
            legacy_raw_data
        );
        // removed "cursor_address" and "hex_dump_cursor_address" fields

        let mut leg_path = std::env::temp_dir();
        leg_path.push("test_legacy.regen2000proj");
        std::fs::write(&leg_path, json).unwrap();

        let mut leg_state = AppState::new();
        let leg_data = leg_state.load_project(leg_path.clone()).unwrap();
        let leg_cursor = leg_data.cursor_address;
        let leg_hex_cursor = leg_data.hex_dump_cursor_address;

        assert_eq!(
            leg_cursor, None,
            "Legacy project should return None for cursor"
        );
        assert_eq!(
            leg_hex_cursor, None,
            "Legacy project should return None for hex cursor"
        );

        // Cleanup
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(leg_path);
    }
}
