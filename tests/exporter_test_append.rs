use regenerator2000::exporter::export_asm;
use regenerator2000::state::AppState;
use std::path::PathBuf;

#[test]
fn test_export_all_labels_disabled() {
    let mut state = AppState::new();
    state.origin = 0x1000;
    state.raw_data = vec![0xEA];

    // Define an external label
    state.labels.insert(
        0x0010,
        vec![regenerator2000::state::Label {
            name: "f10".to_string(),
            kind: regenerator2000::state::LabelKind::Auto,
            label_type: regenerator2000::state::LabelType::ZeroPageField,
        }],
    );

    // Disable "All Labels"
    state.settings.all_labels = false;

    // Run disassembly
    state.disassemble();

    // precise verification: disassembly should NOT verify external label definition
    // External label definitions usually look like `f10 = $10`
    // And headers like `; ZP FIELDS`
    // We iterate and ensure none of that is there.
    for line in &state.disassembly {
        if line.mnemonic.contains("ZP FIELDS") || line.mnemonic.contains("f10 =") {
            panic!("Disassembly contained external label definition but 'all_labels' is false!");
        }
    }

    // Now Export
    let file_name = "test_export_all_labels_false.asm";
    let path = PathBuf::from(file_name);
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }

    let res = export_asm(&state, &path);
    assert!(res.is_ok());

    let content = std::fs::read_to_string(&path).unwrap();
    // println!("Content:\n{}", content);

    // Must contain the label definition
    assert!(content.contains("f10 = $10"));
    assert!(content.contains("; ZP FIELDS"));

    let _ = std::fs::remove_file(&path);
}
