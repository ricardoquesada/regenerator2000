use crate::state::{Addr, AnnotationManager, BlockType, DocumentSettings, EnumDefinition, Label};
use std::collections::{BTreeMap, BTreeSet};

use super::formatter::Formatter;

/// Context containing all the data needed for disassembly.
pub struct DisassemblyContext<'a> {
    pub data: &'a [u8],
    pub block_types: &'a [BlockType],
    pub labels: &'a BTreeMap<Addr, Vec<Label>>,
    pub origin: Addr,
    pub settings: &'a DocumentSettings,
    pub annotations: &'a AnnotationManager,
    pub cross_refs: &'a BTreeMap<Addr, Vec<Addr>>,
    pub collapsed_blocks: &'a [(usize, usize)],
    pub splitters: &'a BTreeSet<Addr>,
    pub scope_ends: BTreeSet<Addr>,

    // Enums references
    pub enums: &'a BTreeMap<String, EnumDefinition>,
    pub user_global_enums: &'a BTreeMap<String, EnumDefinition>,
    pub builtin_enums: &'a BTreeMap<String, EnumDefinition>,
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
    pub label_scope_names: Option<&'a BTreeMap<Addr, String>>,
    pub current_scope_name: Option<String>,
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
        annotations: &'a AnnotationManager,
        cross_refs: &'a BTreeMap<Addr, Vec<Addr>>,
        collapsed_blocks: &'a [(usize, usize)],
        splitters: &'a BTreeSet<Addr>,
        enums: &'a BTreeMap<String, EnumDefinition>,
        user_global_enums: &'a BTreeMap<String, EnumDefinition>,
        builtin_enums: &'a BTreeMap<String, EnumDefinition>,
    ) -> Self {
        let scope_ends = annotations.scope_ends();
        Self {
            data,
            block_types,
            labels,
            origin,
            settings,
            annotations,
            cross_refs,
            collapsed_blocks,
            splitters,
            scope_ends,
            enums,
            user_global_enums,
            builtin_enums,
        }
    }

    #[must_use]
    pub fn resolve_enum_value(&self, address: Addr, value: u16) -> Option<(String, String)> {
        let entry = self.annotations.get(address)?;
        let enum_name = entry.enum_usage.as_ref()?;
        let enum_def = self
            .enums
            .get(enum_name)
            .or_else(|| self.user_global_enums.get(enum_name))
            .or_else(|| self.builtin_enums.get(enum_name))?;
        let variant = enum_def.variants.get(&value)?;
        Some((enum_name.clone(), variant.clone()))
    }

    #[must_use]
    pub fn is_virtual_splitter(&self, addr: Addr) -> bool {
        if self.splitters.contains(&addr) {
            return true;
        }
        if self.annotations.get(addr).and_then(|e| e.scope).is_some() {
            return true;
        }
        self.scope_ends.contains(&addr)
    }

    #[must_use]
    pub fn get_side_comment(&self, address: Addr, comment_prefix: &str) -> String {
        let mut comment_parts = Vec::new();

        if let Some(entry) = self.annotations.get(address) {
            if let Some(user_comment) = &entry.user_side_comment {
                comment_parts.push(user_comment.clone());
            } else if let Some(sys_comment) = &entry.system_comment {
                comment_parts.push(sys_comment.clone());
            }
        }

        if let Some(refs) = self.cross_refs.get(&address)
            && !refs.is_empty()
            && self.settings.max_xref_count > 0
        {
            comment_parts.push(format_cross_references(refs, self.settings.max_xref_count));
        }

        let separator = format!(" {comment_prefix} "); // e.g. " ; " or " // "
        comment_parts.join(&separator)
    }

    #[must_use]
    pub fn get_target_comment(&self, target_addr: Addr) -> Option<&str> {
        let is_code_target = target_addr
            .0
            .checked_sub(self.origin.0)
            .and_then(|off| self.block_types.get(off as usize))
            .is_some_and(|bt| *bt == BlockType::Code);

        if is_code_target {
            self.annotations
                .get(target_addr)
                .and_then(|e| e.system_comment.as_deref())
        } else if let Some(c) = self
            .annotations
            .get(target_addr)
            .and_then(|e| e.user_side_comment.as_deref())
        {
            Some(c)
        } else {
            self.annotations
                .get(target_addr)
                .and_then(|e| e.system_comment.as_deref())
        }
    }
}

#[must_use]
pub fn format_cross_references(refs: &[Addr], max_count: usize) -> String {
    if refs.is_empty() || max_count == 0 {
        return String::new();
    }

    let mut all_refs = refs.to_vec();
    all_refs.sort_unstable();
    all_refs.dedup();

    let refs_str: Vec<String> = all_refs
        .iter()
        .take(max_count)
        .map(|r| format!("${r:04x}"))
        .collect();

    let suffix = if all_refs.len() > max_count {
        ", ..."
    } else {
        ""
    };

    format!("x-ref: {}{}", refs_str.join(", "), suffix)
}
