#[cfg(test)]
mod tests {
    use regenerator2000::state::{AppState, PROJECT_FORMAT_VERSION};
    use std::collections::BTreeMap;

    fn make_test_json(include_version: Option<u32>) -> String {
        let raw_data_b64 = regenerator2000::state::encode_raw_data_to_base64(&[0xEA]).unwrap();
        let version_part = match include_version {
            Some(v) => format!("\"version\": {v},"),
            None => String::new(),
        };
        format!(
            "{{{version_part} \"origin\": 4096, \"raw_data_base64\": \"{raw_data_b64}\", \"blocks\": [{{\"start\":0,\"end\":0,\"type_\":\"Code\"}}]}}"
        )
    }

    #[test]
    fn test_load_project_without_version_field() {
        // Craft a minimal project JSON *without* a "version" key.
        // The serde default should treat it as version 1 and load successfully.
        let json = make_test_json(None);

        let mut path = std::env::temp_dir();
        path.push("test_no_version.regen2000proj");
        std::fs::write(&path, json).unwrap();

        let mut app_state = AppState::new();
        let result = app_state.load_project(path.clone());
        assert!(
            result.is_ok(),
            "Should load project without version field: {:?}",
            result.err()
        );

        // Cleanup
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_project_with_future_version() {
        // A project file with a version far in the future should be rejected.
        let json = make_test_json(Some(999));

        let mut path = std::env::temp_dir();
        path.push("test_future_version.regen2000proj");
        std::fs::write(&path, json).unwrap();

        let mut app_state = AppState::new();
        let result = app_state.load_project(path.clone());
        assert!(result.is_err(), "Should reject future version project");
        let err_msg = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error for future version"),
        };
        assert!(
            err_msg.contains("newer version"),
            "Error should mention newer version: {err_msg}"
        );

        // Cleanup
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_save_project_includes_version() {
        use regenerator2000::state::ProjectSaveContext;

        let mut app_state = AppState::new();
        let mut temp_path = std::env::temp_dir();
        temp_path.push("test_save_version.regen2000proj");
        app_state.project_path = Some(temp_path.clone());

        // Minimal data so save doesn't fail
        app_state.origin = regenerator2000::state::Addr(0x1000);
        app_state.raw_data = vec![0xEA]; // 1 NOP
        app_state.block_types = vec![regenerator2000::state::BlockType::Code];

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
                    bookmarks: BTreeMap::new(),
                },
                false,
            )
            .expect("Save failed");

        let content = std::fs::read_to_string(&temp_path).expect("Read failed");
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed["version"],
            serde_json::json!(PROJECT_FORMAT_VERSION),
            "Saved project should contain version field"
        );

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }
}
