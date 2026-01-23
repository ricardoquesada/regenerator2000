#[cfg(test)]
mod tests {
    use regenerator2000::state::{
        AppState, BlockType, Label, LabelKind, LabelType, ProjectSaveContext,
    };
    use std::collections::BTreeMap;

    #[test]
    fn test_serialization_stability() {
        let mut app_state = AppState::new();
        let mut temp_path = std::env::temp_dir();
        temp_path.push("stability_test.regen2000proj");
        app_state.project_path = Some(temp_path.clone());

        // 1. Setup a complex state with multiple labels at same addresses and comments
        app_state.origin = 0x1000;
        app_state.raw_data = vec![0xEA; 100]; // 100 NOPs
        app_state.block_types = vec![BlockType::Code; 100];

        // Add labels at same address in NON-ALPHABETICAL order
        let mut labels = BTreeMap::new();
        labels.insert(
            0x1000,
            vec![
                Label {
                    name: "Z_Label".to_string(),
                    label_type: LabelType::UserDefined,
                    kind: LabelKind::User,
                },
                Label {
                    name: "A_Label".to_string(),
                    label_type: LabelType::UserDefined,
                    kind: LabelKind::User,
                },
            ],
        );
        labels.insert(
            0x1005,
            vec![Label {
                name: "M_Label".to_string(),
                label_type: LabelType::UserDefined,
                kind: LabelKind::User,
            }],
        );
        app_state.labels = labels;

        // Add comments in random order (BTreeMap will handle address order, but let's be sure)
        app_state
            .user_side_comments
            .insert(0x1005, "Comment 2".to_string());
        app_state
            .user_side_comments
            .insert(0x1000, "Comment 1".to_string());

        app_state
            .user_line_comments
            .insert(0x1005, "Line 2".to_string());
        app_state
            .user_line_comments
            .insert(0x1000, "Line 1".to_string());

        // 2. Save for the first time
        app_state
            .save_project(
                ProjectSaveContext {
                    cursor_address: None,
                    hex_dump_cursor_address: None,
                    sprites_cursor_address: None,
                    right_pane_visible: None,
                    charset_cursor_address: None,
                    bitmap_cursor_address: None,
                    sprite_multicolor_mode: false,
                    charset_multicolor_mode: false,
                    bitmap_multicolor_mode: false,
                    hexdump_view_mode: regenerator2000::state::HexdumpViewMode::default(),
                    splitters: std::collections::BTreeSet::new(),
                    blocks_view_cursor: None,
                },
                false,
            )
            .expect("First save failed");
        let first_save_content =
            std::fs::read_to_string(&temp_path).expect("Read first save failed");

        // 3. Clear state and load
        let mut app_state_2 = AppState::new();
        app_state_2
            .load_project(temp_path.clone())
            .expect("Load failed");

        // 4. Save again
        app_state_2
            .save_project(
                ProjectSaveContext {
                    cursor_address: None,
                    hex_dump_cursor_address: None,
                    sprites_cursor_address: None,
                    right_pane_visible: None,
                    charset_cursor_address: None,
                    bitmap_cursor_address: None,
                    sprite_multicolor_mode: false,
                    charset_multicolor_mode: false,
                    bitmap_multicolor_mode: false,
                    hexdump_view_mode: regenerator2000::state::HexdumpViewMode::default(),
                    splitters: std::collections::BTreeSet::new(),
                    blocks_view_cursor: None,
                },
                false,
            )
            .expect("Second save failed");
        let second_save_content =
            std::fs::read_to_string(&temp_path).expect("Read second save failed");

        // 5. Compare
        assert_eq!(
            first_save_content, second_save_content,
            "Project file content changed after load/save cycle!"
        );

        // 6. Verify label order specifically in the string if needed,
        // but assert_eq already covers it.
        // Let's check that A_Label comes before Z_Label in the JSON string.
        let a_pos = first_save_content.find("A_Label").unwrap();
        let z_pos = first_save_content.find("Z_Label").unwrap();
        assert!(
            a_pos < z_pos,
            "Labels were not sorted alphabetically in the project file!"
        );

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }
}
