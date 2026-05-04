#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use regenerator2000_core::mcp::handler::handle_request;
    use regenerator2000_core::mcp::types::McpRequest;
    use regenerator2000_core::state::AppState;
    use regenerator2000_tui::theme::Theme;
    use regenerator2000_tui::ui_state::UIState;
    use serde_json::json;
    use tokio::sync::oneshot;

    #[test]
    fn test_batch_execute() {
        let mut app_state = AppState::default();
        let origin = regenerator2000_core::state::Addr(0x1000);
        let data = vec![0xEA; 10]; // NOPs
        app_state.load_binary(origin, data).unwrap();

        // Initial state check
        assert!(!app_state.labels.contains_key(&0x1000));
        assert!(!app_state.user_side_comments.contains_key(&0x1000));

        let mut ui_state = UIState::new(Theme::default());

        let calls = vec![
            json!({
                "name": "r2000_set_label_name",
                "arguments": {
                    "address": 0x1000,
                    "name": "start_loop"
                }
            }),
            json!({
                "name": "r2000_set_comment",
                "arguments": {
                    "address": 0x1000,
                    "comment": "Loop Entry",
                    "type": "side"
                }
            }),
        ];

        let (tx, _) = oneshot::channel();
        let req = McpRequest {
            method: "tools/call".to_string(),
            params: json!({
                "name": "r2000_batch_execute",
                "arguments": { "calls": calls }
            }),
            response_sender: tx,
        };

        let response = handle_request(&req, &mut app_state, &mut ui_state);

        assert!(
            response.result.is_some(),
            "Batch tool call failed: {:?}",
            response.error
        );

        let result = response.result.unwrap();
        let content = result.get("content").unwrap().as_array().unwrap();
        let text_result = content[0].get("text").unwrap().as_str().unwrap();

        println!("Batch Result JSON:\n{text_result}");

        // Parse the inner JSON result
        let batch_results: serde_json::Value = serde_json::from_str(text_result).unwrap();
        let batch_array = batch_results.as_array().unwrap();

        assert_eq!(batch_array.len(), 2);
        assert_eq!(batch_array[0]["status"], "success");
        assert_eq!(batch_array[1]["status"], "success");

        // Verify side effects
        // Since get returns a Vec<Label> (because multiple labels can exist at one address? No, map is Address -> Vec<Label> presumably?
        // Let's check state::AppState definition if needed. But mcp_handler says:
        /*
            let label = crate::state::Label { ... };
            let command = crate::commands::Command::SetLabel {
                address,
                new_label: Some(vec![label]),
                ...
            };
        */
        // So `app_state.labels.get(&address)` likely returns `&Vec<Label>`.
        let labels = app_state.labels.get(&0x1000).unwrap();
        assert_eq!(labels[0].name, "start_loop");

        let comment = app_state.user_side_comments.get(&0x1000).unwrap();
        assert_eq!(comment, "Loop Entry");
    }

    #[test]
    fn test_set_label_rejects_duplicate_name() {
        let mut app_state = AppState::default();
        let origin = regenerator2000_core::state::Addr(0x1000);
        let data = vec![0xEA; 10]; // NOPs
        app_state.load_binary(origin, data).unwrap();

        let mut ui_state = UIState::new(Theme::default());

        // Set a label at $1000
        let (tx, _) = oneshot::channel();
        let req = McpRequest {
            method: "tools/call".to_string(),
            params: json!({
                "name": "r2000_set_label_name",
                "arguments": { "address": 0x1000, "name": "my_label" }
            }),
            response_sender: tx,
        };
        let response = handle_request(&req, &mut app_state, &mut ui_state);
        assert!(response.result.is_some(), "First label set should succeed");

        // Try to set the same label name at a different address — should fail
        let (tx2, _) = oneshot::channel();
        let req2 = McpRequest {
            method: "tools/call".to_string(),
            params: json!({
                "name": "r2000_set_label_name",
                "arguments": { "address": 0x1002, "name": "my_label" }
            }),
            response_sender: tx2,
        };
        let response2 = handle_request(&req2, &mut app_state, &mut ui_state);
        assert!(
            response2.error.is_some(),
            "Duplicate label name at different address should return error"
        );
        let err = response2.error.unwrap();
        assert!(
            err.message.contains("already exists"),
            "Error message should mention 'already exists', got: {}",
            err.message
        );

        // Setting the same label at the SAME address should succeed (rename/overwrite)
        let (tx3, _) = oneshot::channel();
        let req3 = McpRequest {
            method: "tools/call".to_string(),
            params: json!({
                "name": "r2000_set_label_name",
                "arguments": { "address": 0x1000, "name": "my_label" }
            }),
            response_sender: tx3,
        };
        let response3 = handle_request(&req3, &mut app_state, &mut ui_state);
        assert!(
            response3.result.is_some(),
            "Re-setting the same label at the same address should succeed"
        );
    }
}
