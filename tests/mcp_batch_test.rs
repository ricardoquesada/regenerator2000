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
    fn test_batch_execute() {
        let mut app_state = AppState::default();
        let origin = 0x1000;
        let data = vec![0xEA; 10]; // NOPs
        app_state.load_binary(origin, data).unwrap();

        // Initial state check
        assert!(app_state.labels.get(&0x1000).is_none());
        assert!(app_state.user_side_comments.get(&0x1000).is_none());

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
                "name": "r2000_set_side_comment",
                "arguments": {
                    "address": 0x1000,
                    "comment": "Loop Entry"
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

        println!("Batch Result JSON:\n{}", text_result);

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
}
