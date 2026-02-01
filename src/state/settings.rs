use super::types::{Assembler, Platform};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSettings {
    #[serde(default)]
    pub all_labels: bool, // default false
    #[serde(default = "default_true")]
    pub preserve_long_bytes: bool, // default true
    #[serde(default)]
    pub brk_single_byte: bool, // default false
    #[serde(default = "default_true")]
    pub patch_brk: bool, // default true
    #[serde(default)]
    pub platform: Platform, // default C64
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
            brk_single_byte: false,
            patch_brk: true,
            platform: Platform::default(),
            assembler: Assembler::default(),
            max_xref_count: 5,
            max_arrow_columns: 6,
            use_illegal_opcodes: false,
            text_char_limit: 40,
            addresses_per_line: 5,
            bytes_per_line: 8,
        }
    }
}
