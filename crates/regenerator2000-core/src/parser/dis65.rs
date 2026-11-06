//! 6502bench SourceGen `.dis65` project file parser.

use serde::Deserialize;
use std::collections::BTreeMap;

/// Deserialized 6502bench SourceGen `.dis65` project metadata.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Dis65Project {
    /// Schema version of the `.dis65` file.
    #[serde(rename = "_ContentVersion")]
    pub content_version: u32,
    /// Expected byte length of the companion binary file.
    #[serde(rename = "FileDataLength")]
    pub file_data_length: usize,
    /// Expected CRC-32 checksum of the companion binary file.
    #[serde(rename = "FileDataCrc32")]
    pub file_data_crc32: u32,
    /// General project properties such as CPU type.
    #[serde(rename = "ProjectProps")]
    pub project_props: ProjectProps,
    /// File-offset to CPU-address mapping entries.
    #[serde(rename = "AddressMap")]
    pub address_map: Vec<AddressMapEntry>,
    /// Code and data type hints keyed by file offset ranges.
    #[serde(rename = "TypeHints")]
    pub type_hints: Vec<TypeHintEntry>,
    /// Inline side comments keyed by decimal file offset string.
    #[serde(rename = "Comments")]
    pub comments: BTreeMap<String, String>,
    /// Multi-line block comments keyed by decimal file offset string.
    #[serde(rename = "LongComments")]
    pub long_comments: BTreeMap<String, LongCommentEntry>,
    /// User-defined symbol labels keyed by decimal file offset string.
    #[serde(rename = "UserLabels")]
    pub user_labels: BTreeMap<String, UserLabelEntry>,
}

impl Dis65Project {
    /// Converts `.dis65` type hints into initial [`crate::state::types::BlockType`] values
    /// and flow-analysis seed addresses.
    ///
    /// Single-byte `"Code"` hints (`low == high`) become flow-analysis seeds relative to `origin`.
    /// Multi-byte `"Code"` hints mark `block_types[low..=high]` as [`crate::state::types::BlockType::Code`].
    #[must_use]
    pub fn to_block_types_and_seeds(
        &self,
        raw_data_len: usize,
        origin: crate::state::types::Addr,
    ) -> (
        Vec<crate::state::types::BlockType>,
        Vec<crate::state::types::Addr>,
    ) {
        use crate::state::types::BlockType;

        let mut block_types = vec![BlockType::DataByte; raw_data_len];
        let mut seeds = Vec::new();

        for hint in &self.type_hints {
            let start = hint.low;
            let end = hint.high;
            if start > end || end >= raw_data_len {
                continue;
            }
            if hint.hint == "Code" && start == end {
                let addr = origin.wrapping_add(start as u16);
                seeds.push(addr);
            } else {
                let ty = match hint.hint.as_str() {
                    "Code" => BlockType::Code,
                    _ => BlockType::DataByte,
                };
                block_types[start..=end].fill(ty);
            }
        }
        (block_types, seeds)
    }
}

/// CPU and target properties from the `.dis65` `ProjectProps` object.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProjectProps {
    /// Target CPU name (for example, `"6502"` or `"65C02"`).
    #[serde(rename = "CpuName")]
    pub cpu_name: String,
}

/// Mapping from a binary file byte offset to a 16-bit CPU load address.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AddressMapEntry {
    /// Zero-based byte offset in the binary file.
    #[serde(rename = "Offset")]
    pub offset: usize,
    /// 16-bit CPU memory address for [`Self::offset`].
    #[serde(rename = "Addr")]
    pub addr: u16,
}

/// Inclusive file-offset range (`low..=high`) and its SourceGen block hint string.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TypeHintEntry {
    /// Inclusive start byte offset in the binary file.
    #[serde(rename = "Low")]
    pub low: usize,
    /// Inclusive end byte offset in the binary file.
    #[serde(rename = "High")]
    pub high: usize,
    /// SourceGen type hint name (such as `"Code"` or `"Data"`).
    #[serde(rename = "Hint")]
    pub hint: String,
}

/// Multi-line comment entry from the `.dis65` `LongComments` map.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LongCommentEntry {
    /// Comment body text.
    #[serde(rename = "Text")]
    pub text: String,
}

/// Symbol label definition from the `.dis65` `UserLabels` map.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UserLabelEntry {
    /// Symbol name.
    #[serde(rename = "Label")]
    pub label: String,
    /// 16-bit target address of the symbol.
    #[serde(rename = "Value")]
    pub value: u16,
    /// Origin of the label (such as `"User"`).
    #[serde(rename = "Source")]
    pub source: String,
    /// Visibility or kind of the label (such as `"Subroutine"`, `"GlobalAddr"`, or `"NonUniqueLocalAddr"`).
    #[serde(rename = "Type")]
    pub label_type: String,
}

/// Parses a `.dis65` project file string into a [`Dis65Project`].
///
/// # Errors
///
/// Returns an error if `content` is empty, lacks the `### 6502bench SourceGen dis65` header line,
/// or contains invalid JSON after the header line.
pub fn parse_dis65(content: &str) -> anyhow::Result<Dis65Project> {
    let Some(header) = content.lines().next() else {
        return Err(anyhow::anyhow!("Empty .dis65 file"));
    };

    let trimmed_header = header.trim_start_matches('\u{FEFF}').trim();
    if !trimmed_header.starts_with("### 6502bench SourceGen dis65") {
        return Err(anyhow::anyhow!("Invalid .dis65 header: {header}"));
    }

    let (_, json_content) = content
        .split_once('\n')
        .ok_or_else(|| anyhow::anyhow!("Missing JSON body in .dis65 file"))?;
    let project: Dis65Project = serde_json::from_str(json_content)?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::{Addr, BlockType};

    const SAMPLE_DIS65: &str = "\u{FEFF}### 6502bench SourceGen dis65 v1.0 ###\n\
{\n\
  \"_ContentVersion\": 5,\n\
  \"FileDataLength\": 16,\n\
  \"FileDataCrc32\": 12345678,\n\
  \"ProjectProps\": { \"CpuName\": \"6502\" },\n\
  \"AddressMap\": [{ \"Offset\": 0, \"Addr\": 4096 }],\n\
  \"TypeHints\": [\n\
    { \"Low\": 0, \"High\": 0, \"Hint\": \"Code\" },\n\
    { \"Low\": 2, \"High\": 5, \"Hint\": \"Code\" },\n\
    { \"Low\": 10, \"High\": 4, \"Hint\": \"Code\" },\n\
    { \"Low\": 0, \"High\": 99, \"Hint\": \"Code\" }\n\
  ],\n\
  \"Comments\": { \"0\": \"entry\" },\n\
  \"LongComments\": { \"2\": { \"Text\": \"header\" } },\n\
  \"UserLabels\": {\n\
    \"0\": { \"Label\": \"start\", \"Value\": 4096, \"Source\": \"User\", \"Type\": \"Subroutine\" }\n\
  }\n\
}";

    #[test]
    fn test_parse_dis65_valid_with_bom_and_inverted_hint() {
        let project = parse_dis65(SAMPLE_DIS65).expect("Valid .dis65 must parse");
        assert_eq!(project.content_version, 5);
        assert_eq!(project.file_data_length, 16);
        assert_eq!(project.project_props.cpu_name, "6502");

        // Inverted hint (Low: 10, High: 4) and out-of-bounds hint (High: 99) must be skipped without panicking.
        let (blocks, seeds) = project.to_block_types_and_seeds(16, Addr(0x1000));
        assert_eq!(seeds, vec![Addr(0x1000)]);
        assert_eq!(blocks[0], BlockType::DataByte);
        assert_eq!(blocks[1], BlockType::DataByte);
        assert_eq!(&blocks[2..=5], &[BlockType::Code; 4]);
        assert_eq!(&blocks[6..], &[BlockType::DataByte; 10]);
    }

    #[test]
    fn test_parse_dis65_errors() {
        assert!(parse_dis65("").is_err());
        assert!(parse_dis65("not a dis65 header\n{}").is_err());
        assert!(parse_dis65("### 6502bench SourceGen dis65 v1.0 ###").is_err());
    }
}
