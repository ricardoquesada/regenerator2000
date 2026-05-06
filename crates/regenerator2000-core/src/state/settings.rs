use super::types::{Assembler, System};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSettings {
    #[serde(default)]
    pub all_labels: bool, // default false
    #[serde(default = "default_true")]
    pub preserve_long_bytes: bool, // default true
    #[serde(default = "default_true")]
    pub brk_single_byte: bool, // default true
    #[serde(default)]
    pub patch_brk: bool, // default false
    #[serde(default, alias = "platform")]
    pub system: System, // default C64
    #[serde(default)]
    pub assembler: Assembler, // default Tass64
    #[serde(default = "default_max_xref")]
    pub max_xref_count: usize, // default 5
    #[serde(default = "default_max_arrow_columns")]
    pub max_arrow_columns: usize, // default 6
    #[serde(default)]
    pub use_illegal_opcodes: bool, // default false
    #[serde(default = "default_text_char_limit")]
    pub text_char_limit: usize, // default 40
    #[serde(default = "default_addresses_per_line")]
    pub addresses_per_line: usize, // default 5
    #[serde(default = "default_bytes_per_line")]
    pub bytes_per_line: usize, // default 8
    #[serde(default)]
    pub enabled_features: std::collections::HashMap<String, bool>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub auto_analyze: bool, // default true
    #[serde(default = "default_true", alias = "show_platform_comments")]
    pub show_system_comments: bool, // default true
    #[serde(default = "default_fill_run_threshold")]
    pub fill_run_threshold: usize, // default 8 (0 = disabled)
    #[serde(default = "default_true", alias = "exclude_comments_from_well_known")]
    pub exclude_well_known_labels: bool, // default true
}

fn default_text_char_limit() -> usize {
    40
}

fn default_addresses_per_line() -> usize {
    5
}

fn default_bytes_per_line() -> usize {
    8
}

fn default_fill_run_threshold() -> usize {
    8
}

fn default_true() -> bool {
    true
}

fn default_max_xref() -> usize {
    5
}

fn default_max_arrow_columns() -> usize {
    6
}

impl Default for DocumentSettings {
    fn default() -> Self {
        Self {
            all_labels: false,
            preserve_long_bytes: true,
            brk_single_byte: true,
            patch_brk: false,
            system: System::default(),
            assembler: Assembler::default(),
            max_xref_count: 5,
            max_arrow_columns: 6,
            use_illegal_opcodes: false,
            text_char_limit: 40,
            addresses_per_line: 5,
            bytes_per_line: 8,
            enabled_features: std::collections::HashMap::new(),
            description: String::new(),
            auto_analyze: true,
            show_system_comments: true,
            fill_run_threshold: 8,
            exclude_well_known_labels: true,
        }
    }
}
