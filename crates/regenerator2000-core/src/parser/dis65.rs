use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Dis65Project {
    #[serde(rename = "_ContentVersion")]
    pub content_version: u32,
    #[serde(rename = "FileDataLength")]
    pub file_data_length: usize,
    #[serde(rename = "FileDataCrc32")]
    pub file_data_crc32: u32,
    #[serde(rename = "ProjectProps")]
    pub project_props: ProjectProps,
    #[serde(rename = "AddressMap")]
    pub address_map: Vec<AddressMapEntry>,
    #[serde(rename = "TypeHints")]
    pub type_hints: Vec<TypeHintEntry>,
    #[serde(rename = "Comments")]
    pub comments: BTreeMap<String, String>,
    #[serde(rename = "LongComments")]
    pub long_comments: BTreeMap<String, LongCommentEntry>,
    #[serde(rename = "UserLabels")]
    pub user_labels: BTreeMap<String, UserLabelEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectProps {
    #[serde(rename = "CpuName")]
    pub cpu_name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddressMapEntry {
    #[serde(rename = "Offset")]
    pub offset: usize,
    #[serde(rename = "Addr")]
    pub addr: u16,
}

#[derive(Debug, Deserialize)]
pub struct TypeHintEntry {
    #[serde(rename = "Low")]
    pub low: usize,
    #[serde(rename = "High")]
    pub high: usize,
    #[serde(rename = "Hint")]
    pub hint: String,
}

#[derive(Debug, Deserialize)]
pub struct LongCommentEntry {
    #[serde(rename = "Text")]
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct UserLabelEntry {
    #[serde(rename = "Label")]
    pub label: String,
    #[serde(rename = "Value")]
    pub value: u16,
    #[serde(rename = "Source")]
    pub source: String,
    #[serde(rename = "Type")]
    pub label_type: String,
}

pub fn parse_dis65(content: &str) -> anyhow::Result<Dis65Project> {
    let mut lines = content.lines();
    if let Some(header) = lines.next() {
        let trimmed_header = header.trim_start_matches('\u{FEFF}').trim();
        if !trimmed_header.starts_with("### 6502bench SourceGen dis65") {
            return Err(anyhow::anyhow!("Invalid .dis65 header: {}", header));
        }
    } else {
        return Err(anyhow::anyhow!("Empty .dis65 file"));
    }

    let first_newline = content.find('\n').unwrap_or(0);
    let json_content = &content[first_newline..];
    let project: Dis65Project = serde_json::from_str(json_content)?;
    Ok(project)
}
