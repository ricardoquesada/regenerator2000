use crate::state::{Addr, BlockType, DocumentSettings, Label};
use std::collections::{BTreeMap, BTreeSet};

use super::formatter::Formatter;

/// Context containing all the data needed for disassembly.
pub struct DisassemblyContext<'a> {
    pub data: &'a [u8],
    pub block_types: &'a [BlockType],
    pub labels: &'a BTreeMap<Addr, Vec<Label>>,
    pub origin: Addr,
    pub settings: &'a DocumentSettings,
    pub system_comments: &'a BTreeMap<Addr, String>,
    pub user_side_comments: &'a BTreeMap<Addr, String>,
    pub user_line_comments: &'a BTreeMap<Addr, String>,
    pub immediate_value_formats: &'a BTreeMap<Addr, crate::state::ImmediateFormat>,
    pub cross_refs: &'a BTreeMap<Addr, Vec<Addr>>,
    pub collapsed_blocks: &'a [(usize, usize)],
    pub splitters: &'a BTreeSet<Addr>,
}

/// Per-iteration values computed in the disassembly loop and passed to each handler.
pub struct HandleArgs<'a> {
    pub pc: usize,
    pub address: Addr,
    pub formatter: &'a dyn Formatter,
    pub label_name: Option<String>,
    pub side_comment: String,
    pub line_comment: Option<String>,
    pub local_label_names: Option<&'a BTreeMap<Addr, String>>,
}

impl<'a> DisassemblyContext<'a> {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn minimal(
        data: &'a [u8],
        block_types: &'a [BlockType],
        labels: &'a BTreeMap<Addr, Vec<Label>>,
        origin: Addr,
        settings: &'a DocumentSettings,
        system_comments: &'a BTreeMap<Addr, String>,
        user_side_comments: &'a BTreeMap<Addr, String>,
        user_line_comments: &'a BTreeMap<Addr, String>,
        immediate_value_formats: &'a BTreeMap<Addr, crate::state::ImmediateFormat>,
        cross_refs: &'a BTreeMap<Addr, Vec<Addr>>,
        collapsed_blocks: &'a [(usize, usize)],
        splitters: &'a BTreeSet<Addr>,
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
