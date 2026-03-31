#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use regenerator2000_core::state::{Addr, AppState};

    #[test]
    fn test_load_dis65_dynamic() {
        let mut app_state = AppState::new();

        // Create dummy binary
        let binary_data = vec![0xEA; 100]; // 100 bytes of NOP
        let crc = regenerator2000_core::utils::calculate_crc32(&binary_data);

        let temp_dir = std::env::temp_dir();
        let binary_path = temp_dir.join("test_dynamic");
        std::fs::write(&binary_path, &binary_data).unwrap();

        let dis65_json = format!(
            r###"### 6502bench SourceGen dis65 v1.0 ###
{{
  "_ContentVersion": 3,
  "FileDataLength": {len},
  "FileDataCrc32": {crc},
  "ProjectProps": {{
    "CpuName": "6502"
  }},
  "AddressMap": [
    {{
      "Offset": 0,
      "Addr": 4096
    }}
  ],
  "TypeHints": [
    {{
      "Low": 0,
      "High": 99,
      "Hint": "Code"
    }}
  ],
  "Comments": {{
    "10": "Test comment"
  }},
  "LongComments": {{
    "20": {{
      "Text": "Test long comment"
    }}
  }},
  "UserLabels": {{
    "50": {{
      "Label": "my_label",
      "Value": 4146,
      "Source": "User",
      "Type": "LocalOrGlobalAddr"
    }}
  }}
}}
"###,
            len = binary_data.len(),
            crc = crc
        );

        let dis65_path = temp_dir.join("test_dynamic.dis65");
        std::fs::write(&dis65_path, dis65_json).unwrap();

        let res = app_state.load_file(dis65_path.clone());
        assert!(
            res.is_ok(),
            "Failed to load .dis65 file: {:?}",
            res.unwrap_err()
        );

        // Verify origin
        assert_eq!(app_state.origin.0, 4096);

        // Verify labels
        let label = app_state.labels.get(&Addr(4146));
        assert!(label.is_some());
        let label = label.unwrap();
        assert_eq!(label.first().unwrap().name, "my_label");

        // Verify side comments
        let comment = app_state.user_side_comments.get(&Addr(4106)); // 4096 + 10
        assert!(comment.is_some());
        assert_eq!(comment.unwrap(), "Test comment");

        // Cleanup
        let _ = std::fs::remove_file(binary_path);
        let _ = std::fs::remove_file(dis65_path);
    }
    #[test]
    fn test_load_dis65_bom() {
        let mut app_state = AppState::new();

        let binary_data = vec![0xEA; 100];
        let crc = regenerator2000_core::utils::calculate_crc32(&binary_data);

        let temp_dir = std::env::temp_dir();
        let binary_path = temp_dir.join("test_bom");
        std::fs::write(&binary_path, &binary_data).unwrap();

        let dis65_json = format!(
            "\u{FEFF}### 6502bench SourceGen dis65 v1.0 ###\n\
{{
  \"_ContentVersion\": 3,
  \"FileDataLength\": {len},
  \"FileDataCrc32\": {crc},
  \"ProjectProps\": {{
    \"CpuName\": \"6502\"
  }},
  \"AddressMap\": [
    {{
      \"Offset\": 0,
      \"Addr\": 4096
    }}
  ],
  \"TypeHints\": [],
  \"Comments\": {{}},
  \"LongComments\": {{}},
  \"UserLabels\": {{}}
}}
",
            len = binary_data.len(),
            crc = crc
        );

        let dis65_path = temp_dir.join("test_bom.dis65");
        std::fs::write(&dis65_path, dis65_json).unwrap();

        let res = app_state.load_file(dis65_path.clone());
        assert!(
            res.is_ok(),
            "Failed to load .dis65 with BOM: {:?}",
            res.unwrap_err()
        );

        let _ = std::fs::remove_file(binary_path);
        let _ = std::fs::remove_file(dis65_path);
    }
}
