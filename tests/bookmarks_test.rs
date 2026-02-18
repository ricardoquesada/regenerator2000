#[cfg(test)]
mod tests {
    use regenerator2000::commands::Command;
    use regenerator2000::state::{AppState, BlockType};

    #[test]
    fn test_bookmark_add_remove() {
        let mut app_state = AppState::new();
        app_state.origin = 0x1000;
        app_state.raw_data = vec![0xEA; 10];
        app_state.block_types = vec![BlockType::Code; 10];

        let addr = 0x1005;

        // 1. Add Bookmark
        let cmd = Command::SetBookmark {
            address: addr,
            new_name: Some("TestBookmark".to_string()),
            old_name: None,
        };
        cmd.apply(&mut app_state);

        assert!(app_state.bookmarks.contains_key(&addr));
        assert_eq!(app_state.bookmarks.get(&addr).unwrap(), "TestBookmark");

        // 2. Remove Bookmark
        let cmd_remove = Command::SetBookmark {
            address: addr,
            new_name: None,
            old_name: Some("TestBookmark".to_string()),
        };
        cmd_remove.apply(&mut app_state);

        assert!(!app_state.bookmarks.contains_key(&addr));

        // 3. Undo Remove
        cmd_remove.undo(&mut app_state);
        assert!(app_state.bookmarks.contains_key(&addr));
        assert_eq!(app_state.bookmarks.get(&addr).unwrap(), "TestBookmark");
    }

    #[test]
    fn test_bookmark_persistence() {
        let mut app_state = AppState::new();
        let mut path = std::env::temp_dir();
        path.push("test_bookmarks.regen2000proj");
        app_state.project_path = Some(path.clone());

        app_state.origin = 0x1000;
        app_state.raw_data = vec![0xEA; 10];
        app_state.block_types = vec![BlockType::Code; 10];

        // Add bookmark
        app_state.bookmarks.insert(0x1002, "MyBookmark".to_string());

        // Save
        let context = regenerator2000::state::project::ProjectSaveContext {
            cursor_address: None,
            hex_dump_cursor_address: None,
            sprites_cursor_address: None,
            right_pane_visible: None,
            charset_cursor_address: None,
            bitmap_cursor_address: None,
            sprite_multicolor_mode: false,
            charset_multicolor_mode: false,
            bitmap_multicolor_mode: false,
            hexdump_view_mode: Default::default(),
            splitters: Default::default(),
            blocks_view_cursor: None,
            bookmarks: app_state.bookmarks.clone(),
        };
        app_state.save_project(context, false).expect("Save failed");

        // Load
        let mut loaded_state = AppState::new();
        let _data = loaded_state
            .load_project(path.clone())
            .expect("Load failed");

        // Verify loaded state has bookmark?
        // Wait, `load_project` updates `loaded_state.bookmarks`?
        // Let's check `load_project`.
        // `AppState::load_project` returns `LoadedProjectData` AND updates `self` (app_state).
        // It consumes `self`? No, `&mut self`.

        assert!(loaded_state.bookmarks.contains_key(&0x1002));
        assert_eq!(loaded_state.bookmarks.get(&0x1002).unwrap(), "MyBookmark");

        // Clean up
        let _ = std::fs::remove_file(path);
    }
}
