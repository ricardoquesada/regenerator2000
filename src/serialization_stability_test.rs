#[cfg(test)]
mod tests {
    use crate::state::{AppState, BlockType, Label, LabelKind, LabelType};
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
            .save_project(None, None, None, None, None)
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
            .save_project(None, None, None, None, None)
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
        // Check if "A_Label" comes before "Z_Label" in the string output
        let a_pos = first_save_content.find("A_Label").expect("A_Label missing");
        let z_pos = first_save_content.find("Z_Label").expect("Z_Label missing");
        
        assert!(a_pos < z_pos, "Labels should be sorted alphabetically in JSON");

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }
}
