#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use regenerator2000_core::mcp::handler::handle_request;
    use regenerator2000_core::mcp::types::McpRequest;
    use regenerator2000_core::state::{Addr, AppState};
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
        assert!(!app_state.labels.contains_key(&Addr(0x1000)));
        assert!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.user_side_comment.as_ref())
                .is_none()
        );

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

        let comment = app_state
            .annotations
            .get(Addr(0x1000))
            .and_then(|e| e.user_side_comment.as_deref())
            .unwrap();
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

    #[test]
    fn test_disassemble_tool() {
        let mut app_state = AppState::default();
        let origin = regenerator2000_core::state::Addr(0x1000);
        // JMP $1005 (4C 05 10), then NOPs
        let data = vec![0x4C, 0x05, 0x10, 0xEA, 0xEA, 0xEA, 0x60];
        app_state.load_binary(origin, data).unwrap();
        app_state.block_types = vec![regenerator2000_core::state::BlockType::Undefined; 7];

        let mut ui_state = UIState::new(Theme::default());

        let (tx, _) = oneshot::channel();
        let req = McpRequest {
            method: "tools/call".to_string(),
            params: json!({
                "name": "r2000_disassemble",
                "arguments": { "address": 0x1000 }
            }),
            response_sender: tx,
        };

        let response = handle_request(&req, &mut app_state, &mut ui_state);
        assert!(
            response.result.is_some(),
            "Disassemble tool call failed: {:?}",
            response.error
        );

        // Verify side effects: Code block type was set at 0x1000..0x1003 and 0x1005..0x1007
        // (addresses $1000, $1001, $1002 are Code, $1003 is Undefined, $1005, $1006 are Code)
        assert_eq!(
            app_state.block_types[0],
            regenerator2000_core::state::BlockType::Code
        );
        assert_eq!(
            app_state.block_types[1],
            regenerator2000_core::state::BlockType::Code
        );
        assert_eq!(
            app_state.block_types[2],
            regenerator2000_core::state::BlockType::Code
        );
        assert_eq!(
            app_state.block_types[3],
            regenerator2000_core::state::BlockType::Undefined
        );
        assert_eq!(
            app_state.block_types[4],
            regenerator2000_core::state::BlockType::Undefined
        );
        assert_eq!(
            app_state.block_types[5],
            regenerator2000_core::state::BlockType::Code
        );
        assert_eq!(
            app_state.block_types[6],
            regenerator2000_core::state::BlockType::Code
        );

        // Auto-generated labels should have been placed at $1005 (jump target)
        assert!(
            app_state
                .labels
                .contains_key(&regenerator2000_core::state::Addr(0x1005))
        );
    }

    #[test]
    fn test_batch_execute_symbolic_offsets_and_immediate_format() {
        let mut app_state = AppState::new();
        app_state.system_config.default_is_unexplored = false;
        let origin = Addr(0x1000);
        // $1000: STA $FC       (85 FC)
        // $1002: LDA #$FF      (A9 FF) -> LowByte($C0FF) -> #<(sub_C100 - 1)
        // $1004: LDY #$C0      (A0 C0) -> HighByte($C0FF) -> #>(sub_C100 - 1)
        // $1006..=$1009: LoHiAddress RTS table for $C0FF, $C0FF (FF FF C0 C0)
        let data = vec![
            0x85, 0xFC, 0xA9, 0xFF, 0xA0, 0xC0, 0xFF, 0xFF, 0xC0, 0xC0, 0x60,
        ];
        app_state.load_binary(origin, data).unwrap();

        let mut ui_state = UIState::new(Theme::default());

        // 1. Verify `tools/list` schema exposes `r2000_set_symbolic_offset`,
        // `r2000_set_immediate_format.properties.offset`, and
        // `r2000_get_symbols.properties.kind` enum containing `"derived"`.
        let (tx_list, _) = oneshot::channel();
        let list_req = McpRequest {
            method: "tools/list".to_string(),
            params: json!({}),
            response_sender: tx_list,
        };
        let list_resp = handle_request(&list_req, &mut app_state, &mut ui_state);
        assert!(list_resp.error.is_none());
        let tools = list_resp.result.unwrap()["tools"]
            .as_array()
            .unwrap()
            .clone();

        let sym_offset_tool = tools
            .iter()
            .find(|t| t["name"] == "r2000_set_symbolic_offset")
            .expect("r2000_set_symbolic_offset must be registered in tools/list");
        assert!(
            sym_offset_tool["inputSchema"]["properties"]["offset"].is_object(),
            "r2000_set_symbolic_offset must expose offset property"
        );
        assert!(
            sym_offset_tool["inputSchema"]["properties"]["base"].is_object(),
            "r2000_set_symbolic_offset must expose base property"
        );

        let imm_tool = tools
            .iter()
            .find(|t| t["name"] == "r2000_set_immediate_format")
            .expect("r2000_set_immediate_format must be registered");
        assert_eq!(
            imm_tool["inputSchema"]["properties"]["offset"]["type"],
            "integer"
        );

        let get_symbols_tool = tools
            .iter()
            .find(|t| t["name"] == "r2000_get_symbols")
            .expect("r2000_get_symbols must be registered");
        let kind_enum = get_symbols_tool["inputSchema"]["properties"]["kind"]["enum"]
            .as_array()
            .expect("r2000_get_symbols.properties.kind.enum must be an array");
        assert!(
            kind_enum.iter().any(|v| v == "derived"),
            "r2000_get_symbols kind enum must include 'derived'"
        );

        // 2. Dispatch `r2000_batch_execute` combining `r2000_set_data_type`, `r2000_set_label_name`,
        // `r2000_set_symbolic_offset`, and `r2000_set_immediate_format`.
        let calls = vec![
            json!({
                "name": "r2000_set_data_type",
                "arguments": {
                    "start_address": 0x1006,
                    "end_address": 0x1009,
                    "data_type": "lo_hi_address"
                }
            }),
            json!({
                "name": "r2000_set_label_name",
                "arguments": { "address": 0xC100, "name": "sub_C100" }
            }),
            json!({
                "name": "r2000_set_label_name",
                "arguments": { "address": 0x00FB, "name": "zp_ptr" }
            }),
            json!({
                "name": "r2000_set_label_name",
                "arguments": { "address": 0x00FC, "name": "zp_ptr + 1" }
            }),
            json!({
                "name": "r2000_set_symbolic_offset",
                "arguments": {
                    "start_address": 0x1006,
                    "end_address": 0x1009,
                    "offset": -1
                }
            }),
            json!({
                "name": "r2000_set_immediate_format",
                "arguments": {
                    "address": 0x1002,
                    "format": "low_byte",
                    "target_address": 0xC100,
                    "offset": -1
                }
            }),
            json!({
                "name": "r2000_set_immediate_format",
                "arguments": {
                    "address": 0x1004,
                    "format": "high_byte",
                    "target_address": 0xC0FF,
                    "offset": -1
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
            response.error.is_none(),
            "batch execute failed: {:?}",
            response.error
        );
        let text_result = response.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        let batch_items: Vec<serde_json::Value> = serde_json::from_str(&text_result).unwrap();
        assert_eq!(batch_items.len(), 7);
        for (idx, item) in batch_items.iter().enumerate() {
            assert_eq!(
                item["status"], "success",
                "batch item {idx} failed: {item:?}"
            );
        }

        // Verify disassembled operands reflect all symbolic offsets.
        let find_op = |addr: u16| -> String {
            app_state
                .disassembly
                .iter()
                .find(|l| l.address == Addr(addr) && !l.bytes.is_empty())
                .unwrap()
                .operand
                .clone()
        };
        assert_eq!(find_op(0x1000), "zp_ptr + 1");
        assert_eq!(find_op(0x1002), "#<(sub_C100 - 1)");
        assert_eq!(find_op(0x1004), "#>(sub_C100 - 1)");
        assert_eq!(find_op(0x1006), "<(sub_C100 - 1), <(sub_C100 - 1)");
        assert_eq!(find_op(0x1008), ">(sub_C100 - 1), >(sub_C100 - 1)");
    }
}
