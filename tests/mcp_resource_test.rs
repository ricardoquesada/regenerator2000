#[cfg(test)]
mod tests {
    use base64::prelude::*;
    use regenerator2000::mcp::handler::handle_request;
    use regenerator2000::mcp::types::McpRequest;
    use regenerator2000::state::AppState;
    use regenerator2000::theme::Theme;
    use regenerator2000::ui_state::UIState;
    use serde_json::json;
    use tokio::sync::oneshot;

    #[test]
    fn test_binary_main_resource() {
        let mut app_state = AppState::default();
        let origin = 0x0801;
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
}
