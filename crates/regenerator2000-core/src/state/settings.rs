//! Per-document disassembly and target machine configuration settings.

use super::types::{Assembler, System};
use serde::{Deserialize, Serialize};

/// Per-document configuration settings for disassembly, analysis, and formatting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSettings {
    /// Emit external label and enum definitions at the top of the disassembly listing.
    #[serde(default)]
    pub all_labels: bool, // default false
    /// Keep 16-bit absolute addressing for instructions that have a zero-page equivalent.
    #[serde(default = "default_true")]
    pub preserve_long_bytes: bool, // default true
    /// Decode `BRK` (`$00`) as a 1-byte instruction instead of a 2-byte instruction.
    #[serde(default = "default_true")]
    pub brk_single_byte: bool, // default true
    /// Emit a `.byte $00` directive instead of the `BRK` mnemonic.
    #[serde(default)]
    pub patch_brk: bool, // default false
    /// Target machine architecture (defaults to [`System::C64`]).
    #[serde(default, alias = "platform")]
    pub system: System, // default C64
    /// Target assembler syntax formatter (defaults to [`Assembler::Tass64`]).
    #[serde(default)]
    pub assembler: Assembler, // default Tass64
    /// Maximum number of cross-reference addresses to show in inline side comments.
    #[serde(default = "default_max_xref")]
    pub max_xref_count: usize, // default 5
    /// Maximum number of control-flow arrow columns to render in the disassembly gutter.
    #[serde(default = "default_max_arrow_columns")]
    pub max_arrow_columns: usize, // default 6
    /// Decode undocumented 6502 opcodes as instructions instead of raw data bytes.
    #[serde(default)]
    pub use_illegal_opcodes: bool, // default false
    /// Maximum number of characters to emit on a single text directive line.
    #[serde(default = "default_text_char_limit")]
    pub text_char_limit: usize, // default 40
    /// Maximum number of 16-bit addresses to emit on a single address directive line.
    #[serde(default = "default_addresses_per_line")]
    pub addresses_per_line: usize, // default 5
    /// Maximum number of raw bytes to emit on a single byte directive line.
    #[serde(default = "default_bytes_per_line")]
    pub bytes_per_line: usize, // default 8
    /// Optional platform feature flags that control which system labels load from assets.
    #[serde(default)]
    pub enabled_features: std::collections::HashMap<String, bool>,
    /// User-supplied project description notes.
    #[serde(default)]
    pub description: String,
    /// Run automatic code and cross-reference analysis after loading a file.
    #[serde(default = "default_true")]
    pub auto_analyze: bool, // default true
    /// Load built-in platform hardware and ROM comments from system assets.
    #[serde(default = "default_true", alias = "show_platform_comments")]
    pub show_system_comments: bool, // default true
    /// Minimum run length of identical bytes required to emit a fill directive (`0` disables fill runs).
    #[serde(default = "default_fill_run_threshold")]
    pub fill_run_threshold: usize, // default 8 (0 = disabled)
    /// Exclude well-known platform addresses from automatic label generation during analysis.
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
