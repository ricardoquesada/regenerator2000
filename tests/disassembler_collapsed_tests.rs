use regenerator2000::disassembler::Disassembler;
use regenerator2000::state::{BlockType, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_collapsed_block_rendering() {
    let settings = DocumentSettings::default();
    let disassembler = Disassembler::new();
    let labels = BTreeMap::new();
    let origin = 0x1000;

    // 4 NOPs
    let code = vec![0xEA, 0xEA, 0xEA, 0xEA];
    let block_types = vec![BlockType::Code; 4];

    // Collapsed block from offset 1 to 2 ($1001-$1002)
    let collapsed_blocks = vec![(1, 2)];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &collapsed_blocks,
        &std::collections::BTreeSet::new(),
    );

    // Expected:
    // Line 0: NOP ($1000)
    // Line 1: Collapsed block ($1001-$1002)
    // Line 2: NOP ($1003)

    assert_eq!(lines.len(), 3);

    assert_eq!(lines[0].address, 0x1000);
    assert_eq!(lines[0].mnemonic, "nop");

    assert_eq!(lines[1].address, 0x1001);
    assert!(lines[1].mnemonic.contains("Collapsed Code block"));
    // Verify bytes are empty for collapsed block
    assert!(lines[1].bytes.is_empty());

    assert_eq!(lines[2].address, 0x1003);
    assert_eq!(lines[2].mnemonic, "nop");
}
