//! Disassembly execution context and inline cross-reference comment formatting.
//!
//! This module defines [`DisassemblyContext`] to hold read-only project state for the pipeline.
//! It also defines [`HandleArgs`] for per-iteration state and [`format_cross_references`] for
//! inline `"x-ref: $xxxx, $yyyy"` side comments.

use crate::state::{
    Addr, AnnotationManager, BlockType, DocumentSettings, EnumDefinition, Label, XRef,
};
use std::collections::{BTreeMap, BTreeSet};

use super::formatter::Formatter;

/// Read-only context that holds all state references for a disassembly pass.
pub struct DisassemblyContext<'a> {
    /// Raw binary byte slice to disassemble.
    pub data: &'a [u8],
    /// Per-byte [`BlockType`] classification parallel to [`Self::data`].
    pub block_types: &'a [BlockType],
    /// Address-keyed map of user, system, and auto-generated [`Label`]s.
    pub labels: &'a BTreeMap<Addr, Vec<Label>>,
    /// Base 16-bit load address (origin) of [`Self::data`].
    pub origin: Addr,
    /// Document-level disassembly and formatting settings.
    pub settings: &'a DocumentSettings,
    /// Sparse per-address annotations (comments, immediate formats, bookmarks, scopes, enums,
    /// and Tier 1/Tier 2 symbolic offsets).
    pub annotations: &'a AnnotationManager,
    /// Inverted cross-reference index that maps target [`Addr`]s to typed [`XRef`]s.
    pub cross_refs: &'a BTreeMap<Addr, Vec<XRef>>,
    /// Inclusive `(start_offset, end_offset)` byte-offset ranges of collapsed blocks.
    pub collapsed_blocks: &'a [(usize, usize)],
    /// Set of addresses with manual user block splitters.
    pub splitters: &'a BTreeSet<Addr>,
    /// Pre-computed set of lexical scope end addresses for `O(log S)` boundary checks.
    pub scope_ends: &'a BTreeSet<Addr>,

    /// Project-local enum definitions keyed by enum name.
    pub enums: &'a BTreeMap<String, EnumDefinition>,
    /// User-level global enum definitions keyed by enum name.
    pub user_global_enums: &'a BTreeMap<String, EnumDefinition>,
    /// Built-in platform enum definitions keyed by enum name.
    pub builtin_enums: &'a BTreeMap<String, EnumDefinition>,
}

/// Per-iteration values for the disassembly loop and block handlers.
pub struct HandleArgs<'a> {
    /// Current byte offset within [`DisassemblyContext::data`].
    pub pc: usize,
    /// Current 16-bit memory [`Addr`] (`origin + pc`).
    pub address: Addr,
    /// Active assembler [`Formatter`] implementation.
    pub formatter: &'a dyn Formatter,
    /// Primary formatted label name at `address`, if any.
    pub label_name: Option<String>,
    /// Combined inline side comment (user or system side comment plus cross-references).
    pub side_comment: String,
    /// Optional preceding block comment at `address`.
    pub line_comment: Option<String>,
    /// Optional local label names within the current lexical scope.
    pub local_label_names: Option<&'a BTreeMap<Addr, String>>,
    /// Optional map from scoped addresses to their enclosing scope names.
    pub label_scope_names: Option<&'a super::symbols::ScopeNameMap>,
    /// Name of the lexical scope that encloses `address`, if any.
    pub current_scope_name: Option<String>,
}

/// Resolves `value` against the enum annotation at `address`.
///
/// Searches `enums`, `user_global_enums`, and `builtin_enums` in precedence order.
#[must_use]
pub(crate) fn resolve_enum_at(
    annotations: &AnnotationManager,
    enums: &BTreeMap<String, EnumDefinition>,
    user_global_enums: &BTreeMap<String, EnumDefinition>,
    builtin_enums: &BTreeMap<String, EnumDefinition>,
    address: Addr,
    value: u16,
) -> Option<(String, String)> {
    let entry = annotations.get(address)?;
    let enum_name = entry.enum_usage.as_ref()?;
    let enum_def = enums
        .get(enum_name)
        .or_else(|| user_global_enums.get(enum_name))
        .or_else(|| builtin_enums.get(enum_name))?;
    let variant = enum_def.variants.get(&value)?;
    Some((enum_name.clone(), variant.clone()))
}

impl DisassemblyContext<'_> {
    /// Resolves `value` against the enum annotation at `address`.
    ///
    /// Searches project-local, user-global, and built-in enum tables in precedence order.
    #[must_use]
    pub fn resolve_enum_value(&self, address: Addr, value: u16) -> Option<(String, String)> {
        resolve_enum_at(
            self.annotations,
            self.enums,
            self.user_global_enums,
            self.builtin_enums,
            address,
            value,
        )
    }

    /// Returns `true` if `addr` is a manual splitter, scope start, or scope end.
    #[must_use]
    pub fn is_virtual_splitter(&self, addr: Addr) -> bool {
        self.splitters.contains(&addr)
            || self.annotations.is_scope_start(addr)
            || self.scope_ends.contains(&addr)
    }

    /// Returns `true` when `addr` starts a new sub-block inside a split byte table.
    #[must_use]
    pub fn is_split_table_subblock_boundary(&self, addr: Addr) -> bool {
        self.is_virtual_splitter(addr)
            || self
                .annotations
                .get(addr)
                .is_some_and(|e| e.user_line_comment.is_some() || e.user_side_comment.is_some())
    }

    /// Returns `true` if `addr` forces a new line inside a multi-byte data block.
    #[must_use]
    pub fn breaks_data_group(&self, addr: Addr) -> bool {
        // Ignore labels suppressed by an active Tier 2 `address_offset` so internal struct
        // fields (`struct_base + 1..N`) stay grouped cleanly unless split or commented.
        let has_unsuppressed_label = self.labels.contains_key(&addr)
            && !super::symbols::has_active_address_offset(addr, self.labels, self.annotations);
        has_unsuppressed_label || self.is_split_table_subblock_boundary(addr)
    }

    /// Builds the inline side comment for `address` from annotations and cross-references.
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

        let separator = format!(" {comment_prefix} ");
        comment_parts.join(&separator)
    }

    /// Gets a descriptive comment for an operand `target_addr`, such as a hardware register note.
    #[must_use]
    pub fn get_target_comment(&self, target_addr: Addr) -> Option<&str> {
        let entry = self.annotations.get(target_addr)?;
        let is_code_target = target_addr
            .0
            .checked_sub(self.origin.0)
            .and_then(|off| self.block_types.get(off as usize))
            .is_some_and(|bt| *bt == BlockType::Code);

        if is_code_target {
            entry.system_comment.as_deref()
        } else {
            entry
                .user_side_comment
                .as_deref()
                .or(entry.system_comment.as_deref())
        }
    }
}

/// Formats a slice of typed [`XRef`]s into an inline side comment (`"x-ref: $xxxx, $yyyy"`).
///
/// This function extracts, sorts, and deduplicates `r.from` addresses before it truncates to
/// `max_count`. Each source address appears once even when it has multiple
/// [`XRefKind`](crate::state::XRefKind) values.
///
/// Returns an empty `String` if `refs` is empty or `max_count == 0`.
///
/// # Examples
/// ```
/// use regenerator2000_core::disassembler::context::format_cross_references;
/// use regenerator2000_core::state::{Addr, XRef, XRefKind};
///
/// let refs = [
///     XRef::new(Addr(0x2000), XRefKind::Write),
///     XRef::new(Addr(0x1000), XRefKind::Read),
/// ];
/// assert_eq!(format_cross_references(&refs, 4), "x-ref: $1000, $2000");
/// assert_eq!(format_cross_references(&refs, 1), "x-ref: $1000, ...");
/// ```
#[must_use]
pub fn format_cross_references(refs: &[XRef], max_count: usize) -> String {
    if refs.is_empty() || max_count == 0 {
        return String::new();
    }

    // Inline side comments keep `"x-ref: $xxxx, $yyyy"` without badges because TUI and IR
    // navigation parsers split on `", "` and parse bare hex addresses.
    // `FindReferencesDialog` and MCP tools expose the typed `[R]`/`[W]`/`[R/W]`/`[C]`/`[J]`/`[O]` badges.
    // Deduplicate source addresses so multi-kind edges from one address do not repeat `$xxxx`.
    let mut all_refs: Vec<Addr> = refs.iter().map(|r| r.from).collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::XRefKind;

    #[test]
    fn test_format_cross_references_accepts_typed_xref_slice() {
        let refs = vec![
            XRef::new(Addr(0x2000), XRefKind::Write),
            XRef::new(Addr(0x1000), XRefKind::Read),
            XRef::new(Addr(0x1000), XRefKind::Offset),
            XRef::new(Addr(0x3000), XRefKind::Call),
        ];
        assert_eq!(
            format_cross_references(&refs, 2),
            "x-ref: $1000, $2000, ..."
        );
        assert_eq!(
            format_cross_references(&refs, 5),
            "x-ref: $1000, $2000, $3000"
        );
        assert_eq!(format_cross_references(&refs, 0), "");
        assert_eq!(format_cross_references(&[], 5), "");
    }
}
