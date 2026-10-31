//! Project file (`.regen2000proj`) serialization, payload compression, and block run-length encoding.
//!
//! This module defines the on-disk [`ProjectState`] schema, gzip + Base64 binary payload helpers
//! ([`encode_raw_data_to_base64`] and [`decode_raw_data_from_base64`]), and conversion functions
//! between per-byte [`BlockType`] vectors and run-length encoded [`Block`] spans
//! ([`compress_block_types`] and [`expand_blocks`]).

use super::settings::DocumentSettings;
use super::types::{
    Addr, BlockType, EnumDefinition, HexdumpViewMode, LabelKind, LabelType, System,
};
use base64::{Engine as _, engine::general_purpose};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};

/// Current project file format version.
/// Increment this when making breaking changes to the project file format,
/// and add a corresponding migration in `AppState::migrate_project()`.
pub const PROJECT_FORMAT_VERSION: u32 = 1;

fn default_project_version() -> u32 {
    1
}

/// A symbolic label attached to an [`Addr`] in [`AppState::labels`](super::app_state::AppState::labels).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    /// Symbolic identifier rendered in disassembly output.
    pub name: String,
    /// Semantic classification of the label (subroutine, branch target, pointer, etc.).
    pub label_type: LabelType,
    /// Provenance of the label (`User`, `Auto`, or `System`).
    pub kind: LabelKind,
}

/// A run-length encoded contiguous region of bytes sharing a single [`BlockType`] and collapsed state.
///
/// Unlike [`BlockItem::Block`](super::app_state::BlockItem::Block) (which stores 16-bit [`Addr`] bounds),
/// `start` and `end` here are **inclusive zero-based byte offsets** relative to `origin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Inclusive starting byte offset relative to `origin`.
    pub start: usize,
    /// Inclusive ending byte offset relative to `origin`.
    pub end: usize,
    /// Interpretation [`BlockType`] applied to the byte range `start..=end`.
    pub type_: BlockType,
    /// Whether the block is collapsed into a single summary line in the disassembly view.
    #[serde(default)]
    pub collapsed: bool,
}

/// On-disk JSON representation of a saved `.regen2000proj` project file.
///
/// Uses [`BTreeMap`] and [`BTreeSet`] for all address- and name-keyed collections
/// to guarantee deterministic serialization order across save/load cycles.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectState {
    /// Schema format version (defaults to `1` for legacy project files without a `version` field).
    #[serde(default = "default_project_version")]
    pub version: u32,
    /// Load address (origin) of the binary in 16-bit 6502 memory.
    pub origin: Addr,
    /// Gzip-compressed, Base64-encoded raw binary bytes.
    #[serde(rename = "raw_data_base64")]
    pub raw_data: String,
    /// Run-length encoded [`Block`] spans covering the binary payload.
    pub blocks: Vec<Block>,
    /// Persisted user and auto-analyzed [`Label`]s keyed by [`Addr`].
    #[serde(default)]
    pub labels: BTreeMap<Addr, Vec<Label>>,
    /// Persisted user annotations (comments, immediate formats, bookmarks, enums, and lexical scopes).
    #[serde(default, flatten)]
    pub annotations: super::annotations::AnnotationManager,
    /// Document-level disassembly and target platform settings.
    #[serde(default)]
    pub settings: DocumentSettings,
    /// Saved disassembly cursor [`Addr`].
    #[serde(default)]
    pub cursor_address: Option<Addr>,
    /// Saved Hex Dump view cursor [`Addr`].
    #[serde(default)]
    pub hex_dump_cursor_address: Option<Addr>,
    /// Saved Sprites view cursor [`Addr`].
    #[serde(default)]
    pub sprites_cursor_address: Option<Addr>,
    /// Saved Charset view cursor [`Addr`].
    #[serde(default)]
    pub charset_cursor_address: Option<Addr>,
    /// Name of the active right-hand companion pane, if visible.
    #[serde(default)]
    pub right_pane_visible: Option<String>,
    /// Whether multicolor mode is enabled in the Sprites view.
    #[serde(default)]
    pub sprite_multicolor_mode: bool,
    /// Whether multicolor mode is enabled in the Charset view.
    #[serde(default)]
    pub charset_multicolor_mode: bool,
    /// Saved Bitmap view cursor [`Addr`].
    #[serde(default)]
    pub bitmap_cursor_address: Option<Addr>,
    /// Whether multicolor mode is enabled in the Bitmap view.
    #[serde(default)]
    pub bitmap_multicolor_mode: bool,
    /// Character encoding mode for the Hex Dump view (`PETSCII` or `Screencode`).
    #[serde(default)]
    pub hexdump_view_mode: HexdumpViewMode,
    /// Addresses of user-defined manual block splitters.
    #[serde(default)]
    pub splitters: BTreeSet<Addr>,
    /// Selected item index in the Blocks View list.
    #[serde(default)]
    pub blocks_view_cursor: Option<usize>,
    /// Project-local enum definitions keyed by enum name.
    #[serde(default)]
    pub enums: BTreeMap<String, EnumDefinition>,
    /// Per-project user-excluded addresses: always excluded from symbolic
    /// analysis, independent of the `exclude_well_known_labels` setting.
    /// Defaults to empty so existing project files load without errors.
    #[serde(default)]
    pub user_excluded_addresses: BTreeSet<Addr>,
}

/// Transient UI restoration metadata and load-time diagnostics returned when loading a binary or project file.
#[derive(Debug)]
pub struct LoadedProjectData {
    /// Restored disassembly cursor [`Addr`].
    pub cursor_address: Option<Addr>,
    /// Restored Hex Dump view cursor [`Addr`].
    pub hex_dump_cursor_address: Option<Addr>,
    /// Restored Sprites view cursor [`Addr`].
    pub sprites_cursor_address: Option<Addr>,
    /// Restored right-hand pane identifier, if any.
    pub right_pane_visible: Option<String>,
    /// Restored Charset view cursor [`Addr`].
    pub charset_cursor_address: Option<Addr>,
    /// Restored Bitmap view cursor [`Addr`].
    pub bitmap_cursor_address: Option<Addr>,
    /// Restored Sprites view multicolor flag.
    pub sprite_multicolor_mode: bool,
    /// Restored Charset view multicolor flag.
    pub charset_multicolor_mode: bool,
    /// Restored Bitmap view multicolor flag, if specified by the project or loader.
    pub bitmap_multicolor_mode: Option<bool>,
    /// Restored Hex Dump character encoding mode.
    pub hexdump_view_mode: HexdumpViewMode,
    /// Restored selected item index in the Blocks View.
    pub blocks_view_cursor: Option<usize>,
    /// High-entropy warning value if the loaded binary appears packed/compressed.
    pub entropy_warning: Option<f32>,
    /// Suggested entry point [`Addr`] detected by the file parser or BASIC `SYS` stub.
    pub suggested_entry_point: Option<Addr>,
    /// Suggested target [`System`] inferred from the container or load address.
    pub suggested_system: Option<System>,
    /// Name of the detected binary packer from the packer registry, if any.
    pub detected_packer: Option<String>,
}

/// Snapshot of transient UI cursor and view settings captured when saving a `.regen2000proj` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSaveContext {
    /// Current disassembly cursor [`Addr`].
    pub cursor_address: Option<Addr>,
    /// Current Hex Dump view cursor [`Addr`].
    pub hex_dump_cursor_address: Option<Addr>,
    /// Current Sprites view cursor [`Addr`].
    pub sprites_cursor_address: Option<Addr>,
    /// Active right-hand pane identifier, if visible.
    pub right_pane_visible: Option<String>,
    /// Current Charset view cursor [`Addr`].
    pub charset_cursor_address: Option<Addr>,
    /// Current Bitmap view cursor [`Addr`].
    pub bitmap_cursor_address: Option<Addr>,
    /// Whether Sprites view multicolor mode is enabled.
    pub sprite_multicolor_mode: bool,
    /// Whether Charset view multicolor mode is enabled.
    pub charset_multicolor_mode: bool,
    /// Whether Bitmap view multicolor mode is enabled.
    pub bitmap_multicolor_mode: bool,
    /// Active Hex Dump character encoding mode.
    pub hexdump_view_mode: HexdumpViewMode,
    /// Manual block splitter addresses.
    pub splitters: BTreeSet<Addr>,
    /// Selected item index in the Blocks View list.
    pub blocks_view_cursor: Option<usize>,
}

/// Encode raw byte data to a base64 string.
///
/// # Errors
/// Returns an error if compression fails.
pub fn encode_raw_data_to_base64(data: &[u8]) -> anyhow::Result<String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    let compressed_data = encoder.finish()?;
    Ok(general_purpose::STANDARD.encode(compressed_data))
}

/// Decode a base64 string to raw byte data.
///
/// # Errors
/// Returns an error if decoding or decompression fails.
pub fn decode_raw_data_from_base64(data: &str) -> anyhow::Result<Vec<u8>> {
    let decoded_compressed = general_purpose::STANDARD.decode(data)?;
    let mut decoder = GzDecoder::new(&decoded_compressed[..]);
    let mut raw_data = Vec::new();
    decoder.read_to_end(&mut raw_data)?;
    Ok(raw_data)
}

/// Compresses a slice of per-byte [`BlockType`]s and inclusive collapsed `(start_offset, end_offset)`
/// ranges into contiguous [`Block`] spans.
///
/// `collapsed_ranges` may be unsorted, adjacent, or overlapping (with inverted `start > end`
/// pairs ignored). Runs in `O(N)` when `collapsed_ranges` is empty and `O(N + C log C)` otherwise.
#[must_use]
pub fn compress_block_types(
    types: &[BlockType],
    collapsed_ranges: &[(usize, usize)],
) -> Vec<Block> {
    if types.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = 0;
    let mut current_type = types[0];

    // Fast-path when no blocks are collapsed (common case).
    if collapsed_ranges.is_empty() {
        for (i, &t) in types.iter().enumerate().skip(1) {
            if t != current_type {
                ranges.push(Block {
                    start,
                    end: i - 1,
                    type_: current_type,
                    collapsed: false,
                });
                start = i;
                current_type = t;
            }
        }
        ranges.push(Block {
            start,
            end: types.len() - 1,
            type_: current_type,
            collapsed: false,
        });
        return ranges;
    }

    // Sort valid collapsed ranges by (start, end) so we can advance a single cursor in O(N + C log C).
    let mut sorted_collapsed: Vec<(usize, usize)> = collapsed_ranges
        .iter()
        .copied()
        .filter(|&(s, e)| s <= e)
        .collect();
    sorted_collapsed.sort_unstable_by_key(|&(s, e)| (s, e));

    // Monotonic single-cursor membership check for monotonically increasing `idx`.
    // Even if a wider range `(10, 14)` encloses a narrower range `(11, 12)`, `range_cursor`
    // stays anchored on `(10, 14)` until `idx == 15`, at which point the `while` loop advances
    // past both `(10, 14)` and `(11, 12)` because both have `end < 15`.
    let mut range_cursor = 0;
    let mut is_collapsed = |idx: usize| -> bool {
        while range_cursor < sorted_collapsed.len() && sorted_collapsed[range_cursor].1 < idx {
            range_cursor += 1;
        }
        range_cursor < sorted_collapsed.len() && idx >= sorted_collapsed[range_cursor].0
    };

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

/// Expands run-length encoded [`Block`] spans into a per-byte [`BlockType`] vector of length `len`
/// (defaulting uncovered indices to [`BlockType::Code`]) and a list of inclusive collapsed `(start, end)` offset ranges.
///
/// Any [`Block`] whose `end` exceeds `len - 1` is clamped to `len - 1`.
#[must_use]
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

    // T3.1
    #[test]
    fn test_compress_block_types_unsorted_collapsed_ranges_landmine_e() {
        let types = vec![BlockType::DataByte; 20];
        let unsorted = vec![(12, 15), (2, 5)];
        let sorted = vec![(2, 5), (12, 15)];

        let blocks_unsorted = compress_block_types(&types, &unsorted);
        let blocks_sorted = compress_block_types(&types, &sorted);

        assert_eq!(blocks_unsorted, blocks_sorted);
        assert_eq!(
            blocks_unsorted,
            vec![
                Block {
                    start: 0,
                    end: 1,
                    type_: BlockType::DataByte,
                    collapsed: false,
                },
                Block {
                    start: 2,
                    end: 5,
                    type_: BlockType::DataByte,
                    collapsed: true,
                },
                Block {
                    start: 6,
                    end: 11,
                    type_: BlockType::DataByte,
                    collapsed: false,
                },
                Block {
                    start: 12,
                    end: 15,
                    type_: BlockType::DataByte,
                    collapsed: true,
                },
                Block {
                    start: 16,
                    end: 19,
                    type_: BlockType::DataByte,
                    collapsed: false,
                },
            ]
        );
    }

    // T3.2
    #[test]
    fn test_compress_block_types_adjacent_and_overlapping_collapsed_ranges() {
        let types = vec![BlockType::DataByte; 16];
        // Adjacent (0..=3, 4..=7), overlapping (10..=14, 11..=12, 12..=15), and duplicate (0..=3)
        let collapsed = vec![(12, 15), (0, 3), (11, 12), (4, 7), (10, 14), (0, 3)];

        let blocks = compress_block_types(&types, &collapsed);

        // Verify against reference per-byte linear scan
        for (idx, _) in types.iter().enumerate() {
            let expected_collapsed = collapsed.iter().any(|(s, e)| idx >= *s && idx <= *e);
            let actual_block = blocks
                .iter()
                .find(|b| idx >= b.start && idx <= b.end)
                .unwrap();
            assert_eq!(
                actual_block.collapsed, expected_collapsed,
                "Mismatch at index {idx}"
            );
        }
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
            "e_00FF",
            "ExternalJump in ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::AbsoluteAddress.format_label(0xA0),
            "a_00A0",
            "AbsoluteAddress in ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Field.format_label(0x10),
            "f_0010",
            "Field in ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Pointer.format_label(0xFB),
            "p_00FB",
            "Pointer in ZP should use 4 digits"
        );

        // Test zero page addresses with types that should use 2 digits
        assert_eq!(
            LabelType::ZeroPageField.format_label(0xFF),
            "zpf_FF",
            "ZeroPageField in ZP should use 2 digits"
        );
        assert_eq!(
            LabelType::ZeroPagePointer.format_label(0xFB),
            "zpp_FB",
            "ZeroPagePointer in ZP should use 2 digits"
        );
        assert_eq!(
            LabelType::Jump.format_label(0x10),
            "j_10",
            "Jump in ZP should use 2 digits"
        );
        assert_eq!(
            LabelType::Subroutine.format_label(0x20),
            "s_20",
            "Subroutine in ZP should use 2 digits"
        );

        // Test non-zero page addresses (all should use 4 digits)
        assert_eq!(
            LabelType::Jump.format_label(0x1000),
            "j_1000",
            "Jump outside ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Subroutine.format_label(0xC000),
            "s_C000",
            "Subroutine outside ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Field.format_label(0x1234),
            "f_1234",
            "Field outside ZP should use 4 digits"
        );
        assert_eq!(
            LabelType::Pointer.format_label(0xD020),
            "p_D020",
            "Pointer outside ZP should use 4 digits"
        );
    }

    #[test]
    fn test_enum_definition_conversions_and_serialization() {
        use super::super::types::RawEnumDefinition;

        let mut raw_variants = BTreeMap::new();
        raw_variants.insert("1".to_string(), "ON".to_string());
        raw_variants.insert("0x10".to_string(), "START".to_string());
        raw_variants.insert("$20".to_string(), "STOP".to_string());

        let raw = RawEnumDefinition {
            name: "status".to_string(),
            description: Some("System status flags".to_string()),
            variants: raw_variants,
        };

        let def = EnumDefinition::from(raw);
        assert_eq!(def.name, "status");
        assert_eq!(def.description, Some("System status flags".to_string()));
        assert_eq!(def.variants.get(&1), Some(&"ON".to_string()));
        assert_eq!(def.variants.get(&0x10), Some(&"START".to_string()));
        assert_eq!(def.variants.get(&0x20), Some(&"STOP".to_string()));

        // Convert back
        let raw_back = RawEnumDefinition::from(def);
        assert_eq!(raw_back.name, "status");
        assert_eq!(
            raw_back.description,
            Some("System status flags".to_string())
        );
        assert_eq!(raw_back.variants.get("0x01"), Some(&"ON".to_string()));
        assert_eq!(raw_back.variants.get("0x10"), Some(&"START".to_string()));
        assert_eq!(raw_back.variants.get("0x20"), Some(&"STOP".to_string()));

        // Serialize Raw to TOML
        let toml_str = toml::to_string_pretty(&raw_back).unwrap();
        assert!(toml_str.contains("description = \"System status flags\""));
        assert!(toml_str.contains("0x10 = \"START\""));
    }
}
