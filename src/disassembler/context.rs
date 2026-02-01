use crate::state::{BlockType, DocumentSettings, Label};
use std::collections::{BTreeMap, BTreeSet};

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
    pub collapsed_blocks: &'a [(usize, usize)],
    pub splitters: &'a BTreeSet<u16>,
}

impl<'a> DisassemblyContext<'a> {
    #[allow(clippy::too_many_arguments)]
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
            collapsed_blocks,
            splitters,
        }
    }
}
