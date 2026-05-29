#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use base64::prelude::*;
    use regenerator2000_core::mcp::handler::handle_request;
    use regenerator2000_core::mcp::types::McpRequest;
    use regenerator2000_core::state::{Addr, AppState};
    use regenerator2000_tui::theme::Theme;
    use regenerator2000_tui::ui_state::UIState;
    use serde_json::json;
    use tokio::sync::oneshot;

    #[test]
    fn test_binary_main_resource() {
        let mut app_state = AppState::default();
        let origin = regenerator2000_core::state::Addr(0x0801);
        let data = vec![0x00, 0x01, 0x02, 0x03, 0xFF]; // random bytes
        app_state.load_binary(origin, data.clone()).unwrap();

        let mut ui_state = UIState::new(Theme::default());

        let (tx, _rx) = oneshot::channel(); // Dummy channel
        let req = McpRequest {
            method: "resources/read".to_string(),
            params: json!({ "uri": "binary://main" }),
            response_sender: tx,
        };

        let response = handle_request(&req, &mut app_state, &mut ui_state);

        assert!(response.result.is_some(), "Response should have a result");
        assert!(
            response.error.is_none(),
            "Response should not have an error"
        );

        let result = response.result.unwrap();
        let contents = result
            .get("contents")
            .expect("Should have contents")
            .as_array()
            .expect("Contents should be an array");
        assert_eq!(contents.len(), 1);

        let content = &contents[0];
        assert_eq!(content.get("uri").unwrap(), "binary://main");
        assert_eq!(content.get("mimeType").unwrap(), "application/octet-stream");

        let blob_b64 = content
            .get("blob")
            .expect("Should have blob")
            .as_str()
            .expect("Blob should be a string");

        // Decode and verify
        let decoded = BASE64_STANDARD
            .decode(blob_b64)
            .expect("Should decode base64");

        // Check size: 2 header bytes + 5 data bytes = 7 bytes
        assert_eq!(decoded.len(), 2 + data.len());

        // Check header (Little Endian 0x0801)
        assert_eq!(decoded[0], 0x01);
        assert_eq!(decoded[1], 0x08);

        // Check data
        assert_eq!(&decoded[2..], &data[..]);
    }

    #[test]
    fn test_get_binary_info() {
        let mut app_state = AppState::default();
        let origin = regenerator2000_core::state::Addr(0x1000);
        let data = vec![0xEA; 100]; // 100 NOPs -> very low entropy
        app_state.load_binary(origin, data.clone()).unwrap();

        let mut ui_state = UIState::new(Theme::default());

        let (tx, _rx) = oneshot::channel(); // Dummy channel
        let req = McpRequest {
            method: "tools/call".to_string(),
            params: json!({
                "name": "r2000_get_binary_info",
                "arguments": {}
            }),
            response_sender: tx,
        };

        let response = handle_request(&req, &mut app_state, &mut ui_state);

        assert!(response.result.is_some(), "Response should have a result");
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let content_arr = result
            .get("content")
            .expect("Should have content")
            .as_array()
            .expect("Content should be an array");
        assert_eq!(content_arr.len(), 1);

        let text_val = content_arr[0]
            .get("text")
            .expect("Should have text field")
            .as_str()
            .expect("text should be string");

        let info: serde_json::Value = serde_json::from_str(text_val).unwrap();
        assert_eq!(info.get("origin").unwrap(), 0x1000);
        assert_eq!(info.get("size").unwrap(), 100);
        assert!(info.get("entropy").is_some());
        let entropy_val = info.get("entropy").unwrap().as_f64().unwrap();
        assert_eq!(entropy_val, 0.0); // 100 identical NOPs has exactly 0 entropy
    }

    #[test]
    fn test_unpack_binary_disassembles_and_labels_entry_point() {
        let mut app_state = AppState::default();
        let prg_data = std::fs::read("tests/6502/c64_moving_tubes_lxt.dali.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = prg_data[2..].to_vec();
        app_state.load_binary(Addr(load_addr), raw_data).unwrap();

        let mut ui_state = UIState::new(Theme::default());
        let (tx, _rx) = oneshot::channel();
        let req = McpRequest {
            method: "tools/call".to_string(),
            params: json!({
                "name": "r2000_unpack_binary",
                "arguments": {}
            }),
            response_sender: tx,
        };

        let response = handle_request(&req, &mut app_state, &mut ui_state);
        assert!(response.result.is_some(), "Response should have a result");
        assert!(
            response.error.is_none(),
            "Response should not have an error"
        );

        // The unpacked entry point is 0x2E00 (defined in test_unpack_lxt_compressed)
        let entry_addr = Addr(0x2E00);

        // Verify the entry point is labeled as "start"
        let label = app_state.labels.get(&entry_addr);
        assert!(label.is_some(), "Entry point should have a label");
        let label_name = &label.unwrap()[0].name;
        assert_eq!(label_name, "start", "Entry point label should be 'start'");

        // The block type at entry_point should be Code
        let entry_offset = (entry_addr.0 - app_state.origin.0) as usize;
        let block_type = app_state.block_types[entry_offset];
        assert_eq!(
            block_type,
            regenerator2000_core::state::BlockType::Code,
            "Block at entry point should be Code"
        );

        // Verify the disassembly cursor was moved to the entry point
        let cursor_line = &app_state.disassembly[ui_state.cursor_index];
        assert_eq!(
            cursor_line.address, entry_addr,
            "Cursor address should match entry_addr"
        );
    }
}
