use super::settings::DocumentSettings;
use super::types::{BlockType, HexdumpViewMode, ImmediateFormat, LabelKind, LabelType};
use base64::{Engine as _, engine::general_purpose};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub label_type: LabelType,
    pub kind: LabelKind,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Block {
    pub start: usize,
    pub end: usize,
    pub type_: BlockType,
    #[serde(default)]
    pub collapsed: bool,
}

// Note: We use BTreeMap instead of HashMap for all address-keyed collections
// to ensure deterministic serialization order. This guarantees that the
// project file content remains stable across save/load cycles.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectState {
    pub origin: u16,
    #[serde(rename = "raw_data_base64")]
    pub raw_data: String,
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub labels: BTreeMap<u16, Vec<Label>>,
    #[serde(default, alias = "user_comments")]
    pub user_side_comments: BTreeMap<u16, String>,
    #[serde(default)]
    pub user_line_comments: BTreeMap<u16, String>,
    #[serde(default)]
    pub settings: DocumentSettings,
    #[serde(default)]
    pub immediate_value_formats: BTreeMap<u16, ImmediateFormat>,
    #[serde(default)]
    pub cursor_address: Option<u16>,
    #[serde(default)]
    pub hex_dump_cursor_address: Option<u16>,
    #[serde(default)]
    pub sprites_cursor_address: Option<u16>,
    #[serde(default)]
    pub charset_cursor_address: Option<u16>,
    #[serde(default)]
    pub right_pane_visible: Option<String>,
    #[serde(default)]
    pub sprite_multicolor_mode: bool,
    #[serde(default)]
    pub charset_multicolor_mode: bool,
    #[serde(default)]
    pub bitmap_cursor_address: Option<u16>,
    #[serde(default)]
    pub bitmap_multicolor_mode: bool,
    #[serde(default)]
    pub hexdump_view_mode: HexdumpViewMode,
    #[serde(default)]
    pub splitters: BTreeSet<u16>,
    #[serde(default)]
    pub blocks_view_cursor: Option<usize>,
}

pub struct LoadedProjectData {
    pub cursor_address: Option<u16>,
    pub hex_dump_cursor_address: Option<u16>,
    pub sprites_cursor_address: Option<u16>,
    pub right_pane_visible: Option<String>,
    pub charset_cursor_address: Option<u16>,
    pub bitmap_cursor_address: Option<u16>,
    pub sprite_multicolor_mode: bool,
    pub charset_multicolor_mode: bool,
    pub bitmap_multicolor_mode: Option<bool>,
    pub hexdump_view_mode: HexdumpViewMode,
    pub blocks_view_cursor: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSaveContext {
    pub cursor_address: Option<u16>,
    pub hex_dump_cursor_address: Option<u16>,
    pub sprites_cursor_address: Option<u16>,
    pub right_pane_visible: Option<String>,
    pub charset_cursor_address: Option<u16>,
    pub bitmap_cursor_address: Option<u16>,
    pub sprite_multicolor_mode: bool,
    pub charset_multicolor_mode: bool,
    pub bitmap_multicolor_mode: bool,
    pub hexdump_view_mode: HexdumpViewMode,
    pub splitters: BTreeSet<u16>,
    pub blocks_view_cursor: Option<usize>,
}

pub fn encode_raw_data_to_base64(data: &[u8]) -> anyhow::Result<String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    let compressed_data = encoder.finish()?;
    Ok(general_purpose::STANDARD.encode(compressed_data))
}

pub fn decode_raw_data_from_base64(data: &str) -> anyhow::Result<Vec<u8>> {
    let decoded_compressed = general_purpose::STANDARD.decode(data)?;
    let mut decoder = GzDecoder::new(&decoded_compressed[..]);
    let mut raw_data = Vec::new();
    decoder.read_to_end(&mut raw_data)?;
    Ok(raw_data)
}

pub fn compress_block_types(
    types: &[BlockType],
    collapsed_ranges: &[(usize, usize)],
) -> Vec<Block> {
    if types.is_empty() {
        return Vec::new();
    }

    let is_collapsed =
        |idx: usize| -> bool { collapsed_ranges.iter().any(|(s, e)| idx >= *s && idx <= *e) };

    let mut ranges = Vec::new();
    let mut start = 0;
    let mut current_type = types[0];
    let mut current_collapsed = is_collapsed(0);

    for (i, t) in types.iter().enumerate().skip(1) {
        let collapsed = is_collapsed(i);
        if *t != current_type || collapsed != current_collapsed {
            ranges.push(Block {
                start,
                end: i - 1,
                type_: current_type,
                collapsed: current_collapsed,
            });
            start = i;
            current_type = *t;
            current_collapsed = collapsed;
        }
    }

    // Last range
    ranges.push(Block {
        start,
        end: types.len() - 1,
        type_: current_type,
        collapsed: current_collapsed,
    });

    ranges
}

pub fn expand_blocks(ranges: &[Block], len: usize) -> (Vec<BlockType>, Vec<(usize, usize)>) {
    let mut types = vec![BlockType::Code; len];
    let mut collapsed_ranges = Vec::new();

    for range in ranges {
        let end = range.end.min(len - 1);
        if range.start <= end {
            if range.collapsed {
                collapsed_ranges.push((range.start, end));
            }
            types[range.start..=end].fill(range.type_);
        }
    }

    (types, collapsed_ranges)
}

#[cfg(test)]
mod serialization_tests {
    use super::*;

    #[test]
    fn test_compress_block_types() {
        let types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::DataByte,
            BlockType::DataByte,
            BlockType::Code,
        ];
        let empty_collapsed: Vec<(usize, usize)> = Vec::new();
        let ranges = compress_block_types(&types, &empty_collapsed);
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[0].end, 1);
        assert_eq!(ranges[0].type_, BlockType::Code);
        assert!(!ranges[0].collapsed);

        assert_eq!(ranges[1].start, 2);
        assert_eq!(ranges[1].end, 3);
        assert_eq!(ranges[1].type_, BlockType::DataByte);

        assert_eq!(ranges[2].start, 4);
        assert_eq!(ranges[2].end, 4);
        assert_eq!(ranges[2].type_, BlockType::Code);
    }

    #[test]
    fn test_expand_blocks() {
        let ranges = vec![
            Block {
                start: 0,
                end: 1,
                type_: BlockType::Code,
                collapsed: false,
            },
            Block {
                start: 2,
                end: 3,
                type_: BlockType::DataByte,
                collapsed: true,
            },
            Block {
                start: 4,
                end: 4,
                type_: BlockType::Code,
                collapsed: false,
            },
        ];
        let (types, collapsed) = expand_blocks(&ranges, 5);
        assert_eq!(types.len(), 5);
        assert_eq!(types[0], BlockType::Code);
        assert_eq!(types[1], BlockType::Code);
        assert_eq!(types[2], BlockType::DataByte);
        assert_eq!(types[3], BlockType::DataByte);
        assert_eq!(types[4], BlockType::Code);

        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0], (2, 3));
    }

    #[test]
    fn test_encode_decode_raw_data() {
        let data: Vec<u8> = (0..100).collect();
        let encoded = encode_raw_data_to_base64(&data).unwrap();
        // Base64 string should not contain spaces
        assert!(!encoded.contains(' '));

        let decoded = decode_raw_data_from_base64(&encoded).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_label_type_format_label() {
        // Test zero page addresses with types that should use 4 digits
        assert_eq!(
            LabelType::ExternalJump.format_label(0xFF),
            "e00FF",
            "ExternalJump in ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::AbsoluteAddress.format_label(0xA0),
            "a00A0",
            "AbsoluteAddress in ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Field.format_label(0x10),
            "f0010",
            "Field in ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Pointer.format_label(0xFB),
            "p00FB",
            "Pointer in ZP should use 4 digits"
        );

        // Test zero page addresses with types that should use 2 digits
        assert_eq!(
            LabelType::ZeroPageField.format_label(0xFF),
            "fFF",
            "ZeroPageField in ZP should use 2 digits"
        );
        assert_eq!(
            LabelType::ZeroPagePointer.format_label(0xFB),
            "pFB",
            "ZeroPagePointer in ZP should use 2 digits"
        );
        assert_eq!(
            LabelType::Jump.format_label(0x10),
            "j10",
            "Jump in ZP should use 2 digits"
        );
        assert_eq!(
            LabelType::Subroutine.format_label(0x20),
            "s20",
            "Subroutine in ZP should use 2 digits"
        );

        // Test non-zero page addresses (all should use 4 digits)
        assert_eq!(
            LabelType::Jump.format_label(0x1000),
            "j1000",
            "Jump outside ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Subroutine.format_label(0xC000),
            "sC000",
            "Subroutine outside ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Field.format_label(0x1234),
            "f1234",
            "Field outside ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Pointer.format_label(0xD020),
            "pD020",
            "Pointer outside ZP should use 4 digits"
        );
    }
}
