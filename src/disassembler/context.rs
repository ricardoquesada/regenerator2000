use crate::state::{BlockType, DocumentSettings, Label};
use std::collections::{BTreeMap, BTreeSet};

use super::formatter::Formatter;

/// Context containing all the data needed for disassembly.
pub struct DisassemblyContext<'a> {
    pub data: &'a [u8],
    pub block_types: &'a [BlockType],
    pub labels: &'a BTreeMap<u16, Vec<Label>>,
    pub origin: u16,
    pub settings: &'a DocumentSettings,
    pub system_comments: &'a BTreeMap<u16, String>,
    pub user_side_comments: &'a BTreeMap<u16, String>,
    pub user_line_comments: &'a BTreeMap<u16, String>,
    pub immediate_value_formats: &'a BTreeMap<u16, crate::state::ImmediateFormat>,
    pub cross_refs: &'a BTreeMap<u16, Vec<u16>>,
    pub analysis_hints: &'a BTreeMap<u16, String>,
    pub collapsed_blocks: &'a [(usize, usize)],
    pub splitters: &'a BTreeSet<u16>,
}

/// Per-iteration values computed in the disassembly loop and passed to each handler.
pub struct HandleArgs<'a> {
    pub pc: usize,
    pub address: u16,
    pub formatter: &'a dyn Formatter,
    pub label_name: Option<String>,
    pub side_comment: String,
    pub line_comment: Option<String>,
}

impl<'a> DisassemblyContext<'a> {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn minimal(
        data: &'a [u8],
        block_types: &'a [BlockType],
        labels: &'a BTreeMap<u16, Vec<Label>>,
        origin: u16,
        settings: &'a DocumentSettings,
        system_comments: &'a BTreeMap<u16, String>,
        user_side_comments: &'a BTreeMap<u16, String>,
        user_line_comments: &'a BTreeMap<u16, String>,
        immediate_value_formats: &'a BTreeMap<u16, crate::state::ImmediateFormat>,
        cross_refs: &'a BTreeMap<u16, Vec<u16>>,
        analysis_hints: &'a BTreeMap<u16, String>,
        collapsed_blocks: &'a [(usize, usize)],
        splitters: &'a BTreeSet<u16>,
    ) -> Self {
        Self {
            data,
            block_types,
            labels,
            origin,
            settings,
            system_comments,
            user_side_comments,
            user_line_comments,
            immediate_value_formats,
            cross_refs,
            analysis_hints,
            collapsed_blocks,
            splitters,
        }
    }
}
