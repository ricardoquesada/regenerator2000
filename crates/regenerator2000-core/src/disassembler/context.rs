//! Disassembly execution context and inline cross-reference comment formatting.
//!
//! This module defines [`DisassemblyContext`], which bundles all read-only project state required
//! by the disassembly pipeline, [`HandleArgs`], which carries per-iteration decoding state, and
//! [`format_cross_references`], which renders inline `"x-ref: $xxxx, $yyyy"` side comments.

use crate::state::{
    Addr, AnnotationManager, BlockType, DocumentSettings, EnumDefinition, Label, XRef,
};
use std::collections::{BTreeMap, BTreeSet};

use super::formatter::Formatter;

/// Read-only context containing all state references needed during a disassembly pass.
pub struct DisassemblyContext<'a> {
    /// Raw binary byte slice being disassembled.
    pub data: &'a [u8],
    /// Per-byte [`BlockType`] classification parallel to [`Self::data`].
    pub block_types: &'a [BlockType],
    /// Address-keyed map of user, system, and auto-generated [`Label`]s.
    pub labels: &'a BTreeMap<Addr, Vec<Label>>,
    /// Base 16-bit load address (origin) of [`Self::data`].
    pub origin: Addr,
    /// Document-level disassembly and formatting settings.
    pub settings: &'a DocumentSettings,
    /// Sparse per-address annotations (comments, immediate formats, bookmarks, scopes, enums).
    pub annotations: &'a AnnotationManager,
    /// Inverted cross-reference index mapping target [`Addr`]s to typed [`XRef`]s.
    pub cross_refs: &'a BTreeMap<Addr, Vec<XRef>>,
    /// Inclusive `(start_offset, end_offset)` byte-offset ranges of collapsed blocks.
    pub collapsed_blocks: &'a [(usize, usize)],
    /// Set of addresses with manual user block splitters.
    pub splitters: &'a BTreeSet<Addr>,
    /// Pre-computed set of lexical scope end addresses for `O(log S)` boundary checks.
    pub scope_ends: BTreeSet<Addr>,

    /// Project-local enum definitions keyed by enum name.
    pub enums: &'a BTreeMap<String, EnumDefinition>,
    /// User-level global enum definitions keyed by enum name.
    pub user_global_enums: &'a BTreeMap<String, EnumDefinition>,
    /// Built-in platform enum definitions keyed by enum name.
    pub builtin_enums: &'a BTreeMap<String, EnumDefinition>,
}

/// Per-iteration values computed in the disassembly loop and passed to each block handler.
pub struct HandleArgs<'a> {
    /// Current byte offset within [`DisassemblyContext::data`].
    pub pc: usize,
    /// Current 16-bit memory [`Addr`] (`origin + pc`).
    pub address: Addr,
    /// Active assembler [`Formatter`] implementation.
    pub formatter: &'a dyn Formatter,
    /// Primary formatted label name at `address`, if any.
    pub label_name: Option<String>,
    /// Assembled inline side comment (combining user/system side comments and cross-references).
    pub side_comment: String,
    /// Optional preceding block/line comment at `address`.
    pub line_comment: Option<String>,
    /// Optional local label display names within the current lexical scope.
    pub local_label_names: Option<&'a BTreeMap<Addr, String>>,
    /// Optional mapping of scoped addresses to their enclosing scope names.
    pub label_scope_names: Option<&'a BTreeMap<Addr, String>>,
    /// Name of the lexical scope currently enclosing `address`, if any.
    pub current_scope_name: Option<String>,
}

impl<'a> DisassemblyContext<'a> {
    /// Constructs a [`DisassemblyContext`] and pre-computes [`Self::scope_ends`] from `annotations`.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn minimal(
        data: &'a [u8],
        block_types: &'a [BlockType],
        labels: &'a BTreeMap<Addr, Vec<Label>>,
        origin: Addr,
        settings: &'a DocumentSettings,
        annotations: &'a AnnotationManager,
        cross_refs: &'a BTreeMap<Addr, Vec<XRef>>,
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

    /// Resolves `value` against the enum annotation attached to `address`, searching project-local,
    /// user-global, and built-in enum tables in precedence order.
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

    /// Returns `true` if `addr` acts as a block-splitting boundary (manual user splitter, lexical
    /// scope start, or lexical scope end).
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

    /// Builds the inline side comment for `address`, combining any user or system side comment
    /// with formatted cross-references (`"x-ref: $xxxx, ..."`).
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

    /// Retrieves a descriptive comment associated with an instruction's operand `target_addr`
    /// (such as hardware register descriptions or data-target user comments).
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

/// Formats a slice of typed [`XRef`]s into an inline assembly side comment (`"x-ref: $xxxx, $yyyy"`).
///
/// This function extracts, sorts, and deduplicates `r.from` addresses before truncating to `max_count`.
/// A source address with multiple [`XRefKind`](crate::state::XRefKind) values appears only once, which
/// preserves byte-for-byte export roundtrip compatibility.
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

    // Inline side comments keep `"x-ref: $xxxx, $yyyy"` without badges because the TUI and IR
    // navigation parsers split on `", "` and parse bare hex addresses.
    // `FindReferencesDialog` and MCP tools expose the typed `[R]`/`[W]`/`[R/W]`/`[C]`/`[J]`/`[O]` badges.
    // Extract unique source addresses so multi-kind edges from the same address do not repeat `$xxxx`.
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
