#[cfg(test)]
mod tests {
    use regenerator2000::mcp::handler::handle_request;
    use regenerator2000::mcp::types::McpRequest;
    use regenerator2000::state::AppState;
    use regenerator2000::theme::Theme;
    use regenerator2000::ui_state::UIState;
    use serde_json::json;
    use tokio::sync::oneshot;

    #[test]
    fn test_disasm_selected() {
        let mut app_state = AppState::default();
        // Populate with some data
        let origin = 0x1000;
        let data = vec![0xA9, 0x00, 0xAA, 0xCA, 0xE8]; // LDA #$00, TAX, DEX, INX
        app_state.load_binary(origin, data).unwrap();
        // Disassemble (triggered by load_binary)

        // Setup UI State
        let mut ui_state = UIState::new(Theme::default());

        // Select range: Cursor at index 2 (0x1002, TAX), Selection start at index 3 (0x1003, CA)
        // Indices in disassembly:
        // 0: 1000 LDA #$00 (2 bytes)
        // 1: 1002 TAX (1 byte)
        // 2: 1003 DEX (1 byte)
        // 3: 1004 INX (1 byte)

        // Let's verify disassembly structure first
        assert_eq!(app_state.disassembly.len(), 4);
        assert_eq!(app_state.disassembly[0].address, 0x1000);
        assert_eq!(app_state.disassembly[1].address, 0x1002);
        assert_eq!(app_state.disassembly[2].address, 0x1003);
        assert_eq!(app_state.disassembly[3].address, 0x1004); // E8

        // Select 1002-1003
        ui_state.cursor_index = 1; // 1002
        ui_state.selection_start = Some(2); // 1003

        // Create Request (Tool Call)
        let (tx, _r) = oneshot::channel();
        let req = McpRequest {
            method: "tools/call".to_string(),
            params: json!({ "name": "r2000_read_selected_disasm", "arguments": {} }),
            response_sender: tx,
        };

        // Handle Request
        let response = handle_request(&req, &mut app_state, &ui_state);

        // Verify Response
        assert!(
            response.result.is_some(),
            "Tool call failed: {:?}",
            response.error
        );
        let result = response.result.unwrap();
        let content = result.get("content").unwrap().as_array().unwrap();
        let text = content[0].get("text").unwrap().as_str().unwrap();

        println!("Selected Text:\n{}", text);

        assert!(text.contains("1002"));
        assert!(text.contains("tax"));
        assert!(text.contains("1003"));
        assert!(text.contains("dex"));
        assert!(!text.contains("1000")); // Should not contain previous
        assert!(!text.contains("1004")); // Should not contain next
    }

    #[test]
    fn test_hexdump_selected() {
        let mut app_state = AppState::default();
        let origin = 0x1000;
        // Data: 32 bytes (2 rows)
        let data: Vec<u8> = (0..32).collect();
        app_state.load_binary(origin, data).unwrap();

        let mut ui_state = UIState::new(Theme::default());

        // Select row 0 only (0x1000-0x100F)
        ui_state.hex_cursor_index = 0;
        ui_state.hex_selection_start = Some(0);

        let (tx, _) = oneshot::channel();
        let req = McpRequest {
            method: "tools/call".to_string(),
            params: json!({ "name": "r2000_read_selected_hexdump", "arguments": {} }),
            response_sender: tx,
        };

        let response = handle_request(&req, &mut app_state, &ui_state);

        assert!(
            response.result.is_some(),
            "Tool call failed: {:?}",
            response.error
        );
        let result = response.result.unwrap();
        let content = result.get("content").unwrap().as_array().unwrap();
        let text = content[0].get("text").unwrap().as_str().unwrap();

        println!("Hexdump Text:\n{}", text);

        assert!(text.contains("1000:"));
        assert!(!text.contains("1010:")); // Should not contain row 2

        // Select rows 0 and 1
        ui_state.hex_selection_start = Some(1);
        let (tx2, _) = oneshot::channel();
        let req2 = McpRequest {
            method: "tools/call".to_string(),
            params: json!({ "name": "r2000_read_selected_hexdump", "arguments": {} }),
            response_sender: tx2,
        };

        let response2 = handle_request(&req2, &mut app_state, &ui_state);
        let result2 = response2.result.unwrap();
        let text2 = result2.get("content").unwrap().as_array().unwrap()[0]
            .get("text")
            .unwrap()
            .as_str()
            .unwrap();

        println!("Hexdump Text 2:\n{}", text2);
        assert!(text2.contains("1000:"));
        assert!(text2.contains("1010:"));
    }
}
