//! Symbol precedence resolution, two-tier [`SymbolicOffset`] formatting, lexical scope mapping,
//! and operand target extraction.
//!
//! This module resolves the highest-priority [`Label`] or relational [`SymbolicOffset`] expression
//! for an effective 6502 address across five strict precedence tiers:
//!
//! 1. **Tier 1 (`operand_offset` on `site_addr`)**: Per-instruction or per-table-entry override
//!    (such as `LDA table - 1,X` or `.byte <(sub_C100 - 1)`).
//! 2. **Explicit [`LabelKind::User`] at `effective_addr`**: User labels take priority when a
//!    hand-edited project defines both a user label and a Tier 2 offset.
//! 3. **Tier 2 (`address_offset` on `effective_addr`)**: Global pointer high-byte or struct-field
//!    binding (such as `$00FC` formatted as `zp_ptr + 1` or `$0428` formatted as `SCREEN_RAM + 40`).
//! 4. **[`LabelKind::System`] at `effective_addr`**: Platform ROM or hardware symbol.
//! 5. **[`LabelKind::Auto`] at `effective_addr`**: Analyzer-generated symbol (`s_`, `j_`, `f_`, `zpa_`).
//!
//! It also maps lexical symbol scopes, applies target-assembler local label prefixes, and extracts
//! referenced addresses from decoded MOS 6502/6510 opcodes.

use crate::cpu::Opcode;
use crate::disassembler::context::DisassemblyContext;
use crate::disassembler::formatter::Formatter;
use crate::state::{
    Addr, AnnotationManager, DocumentSettings, Label, LabelKind, LabelType, OffsetParentheses,
    SymbolicOffset,
};
use std::collections::BTreeMap;

/// Sparse map of non-overlapping lexical scope intervals `[start, end]` to scope names.
///
/// Stores one `(end, name)` entry per scope (`O(S)` space) and resolves enclosing scope names
/// in `O(log S)` time without per-byte `String` allocations.
#[derive(Debug, Clone, Default)]
pub struct ScopeNameMap(BTreeMap<Addr, (Addr, String)>);

impl ScopeNameMap {
    /// Inserts an inclusive lexical scope range `start..=end` with `name`.
    pub fn insert_range(&mut self, start: Addr, end: Addr, name: String) {
        self.0.insert(start, (end, name));
    }

    /// Returns the enclosing scope name for `addr` in `O(log S)` time, if `addr` lies in a scope.
    #[must_use]
    pub fn get(&self, addr: Addr) -> Option<&str> {
        self.0
            .range(..=addr)
            .next_back()
            .and_then(|(_, (end, name))| (addr <= *end).then_some(name.as_str()))
    }

    /// Returns `true` if the map contains no lexical scopes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Prepends `local_prefix` to `name` when `label_type` is [`LabelType::LocalUserDefined`]
/// and `name` does not already start with `local_prefix`.
#[must_use]
pub(crate) fn apply_local_prefix(
    name: &str,
    label_type: LabelType,
    local_prefix: Option<&str>,
) -> String {
    if label_type == LabelType::LocalUserDefined
        && let Some(p) = local_prefix
        && !name.starts_with(p)
    {
        format!("{p}{name}")
    } else {
        name.to_string()
    }
}

/// Resolves the single label with the highest priority at an address.
///
/// Precedence order:
///
/// 1. [`LabelKind::User`]
/// 2. [`LabelKind::System`]
/// 3. [`LabelKind::Auto`]
///
/// When two labels have the same [`LabelKind`], the alphabetically smaller name wins.
#[must_use]
pub fn resolve_label<'a>(
    labels: &'a [Label],
    _address: u16,
    _settings: &DocumentSettings,
) -> Option<&'a Label> {
    labels.iter().min_by_key(|label| {
        let rank = match label.kind {
            LabelKind::User => 0u8,
            LabelKind::System => 1,
            LabelKind::Auto => 2,
        };
        (rank, &label.name)
    })
}

/// Resolves the symbol name for `address` across local labels, prefixes, and lexical scopes.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn resolve_label_name(
    address: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    settings: &DocumentSettings,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    label_scope_names: Option<&ScopeNameMap>,
    current_scope_name: Option<&str>,
    scope_separator: &str,
    local_prefix: Option<&str>,
) -> Option<String> {
    // 1. Check formatter-generated local label names (for example, `l00`).
    let base_name = if let Some(name) = local_label_names.and_then(|names| names.get(&address)) {
        name.clone()
    } else {
        // 2. Fall back to standard label resolution and local prefixing.
        let v = labels.get(&address)?;
        let l = resolve_label(v, address.0, settings)?;
        apply_local_prefix(&l.name, l.label_type, local_prefix)
    };

    // 3. Prepend enclosing scope name when referenced from outside that scope.
    if let Some(scope_name) = label_scope_names.and_then(|scopes| scopes.get(address)) {
        let same_scope = current_scope_name.is_some_and(|curr| curr == scope_name);
        if !same_scope && base_name != scope_name {
            return Some(format!("{scope_name}{scope_separator}{base_name}"));
        }
    }

    Some(base_name)
}

/// Checks Tier 1 (`operand_offset`) on `site_addr` against `effective_addr`.
#[must_use]
fn tier1_symbolic_offset(
    site_addr: Addr,
    effective_addr: Addr,
    annotations: &AnnotationManager,
) -> Option<SymbolicOffset> {
    annotations
        .get(site_addr)
        .and_then(|e| e.operand_offset)
        .filter(|sym| sym.matches_target(effective_addr))
}

/// Checks Tier 2 (`address_offset`) on `effective_addr` and enforces zero-page base bounds.
#[must_use]
fn tier2_symbolic_offset(
    effective_addr: Addr,
    annotations: &AnnotationManager,
) -> Option<SymbolicOffset> {
    annotations
        .get(effective_addr)
        .and_then(|e| e.address_offset)
        .filter(|sym| {
            sym.matches_target(effective_addr)
                && (effective_addr.0 > 0x00FF || sym.base.0 <= 0x00FF)
        })
}

/// Returns `true` when `addr` carries a valid Tier 2 `address_offset` (`matches_target(addr)`)
/// and is not shadowed by an explicit [`LabelKind::User`] label at `addr`.
///
/// This predicate drives preamble equate suppression (`blocks.rs`), body/mid-instruction label
/// suppression (`pipeline.rs`, `disassembler/mod.rs`), and chained `sym.base` hex fallback.
#[must_use]
pub(crate) fn has_active_address_offset(
    addr: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    annotations: &AnnotationManager,
) -> bool {
    let has_explicit_user_label = labels
        .get(&addr)
        .is_some_and(|v| v.iter().any(|l| l.kind == LabelKind::User));
    !has_explicit_user_label && tier2_symbolic_offset(addr, annotations).is_some()
}

/// Resolves the active [`SymbolicOffset`] for an operand at `site_addr` targeting `effective_addr`.
///
/// Checks Tier 1 (`operand_offset` on `site_addr`) before falling back to Tier 2
/// (`address_offset` on `effective_addr`). Only returns an offset when its non-zero
/// signed displacement satisfies [`SymbolicOffset::matches_target`].
///
/// # Examples
///
/// ```
/// use regenerator2000_core::disassembler::resolve_symbolic_offset;
/// use regenerator2000_core::state::{Addr, AnnotationManager, OffsetRadix, SymbolicOffset};
///
/// let mut annotations = AnnotationManager::default();
/// let zp_hi = SymbolicOffset { base: Addr(0x00FB), offset: 1, radix: OffsetRadix::Decimal };
/// annotations.update(Addr(0x00FC), |e| e.address_offset = Some(zp_hi));
///
/// // Tier 2 resolves globally for any instruction referencing $00FC:
/// assert_eq!(resolve_symbolic_offset(Addr(0x1020), Addr(0x00FC), &annotations), Some(zp_hi));
///
/// // Tier 1 on a specific site overrides Tier 2 on that site only:
/// let site_override = SymbolicOffset { base: Addr(0x00FA), offset: 2, radix: OffsetRadix::Hex };
/// annotations.update(Addr(0x1020), |e| e.operand_offset = Some(site_override));
/// assert_eq!(
///     resolve_symbolic_offset(Addr(0x1020), Addr(0x00FC), &annotations),
///     Some(site_override)
/// );
/// assert_eq!(resolve_symbolic_offset(Addr(0x1040), Addr(0x00FC), &annotations), Some(zp_hi));
/// ```
#[must_use]
pub fn resolve_symbolic_offset(
    site_addr: Addr,
    effective_addr: Addr,
    annotations: &AnnotationManager,
) -> Option<SymbolicOffset> {
    tier1_symbolic_offset(site_addr, effective_addr, annotations)
        .or_else(|| tier2_symbolic_offset(effective_addr, annotations))
}

/// Resolves the active [`SymbolicOffset`] for an instruction mode (`is_zero_page_mode`).
///
/// Checks `Tier 1 > User > Tier 2` and skips any offset with `base > $00FF` in zero-page modes.
#[must_use]
pub(crate) fn resolve_effective_symbolic_offset_for_mode(
    site_addr: Addr,
    effective_addr: Addr,
    is_zero_page_mode: bool,
    labels: &BTreeMap<Addr, Vec<Label>>,
    annotations: &AnnotationManager,
) -> Option<SymbolicOffset> {
    tier1_symbolic_offset(site_addr, effective_addr, annotations)
        .filter(|sym| !is_zero_page_mode || sym.base.0 <= 0x00FF)
        .or_else(|| {
            has_active_address_offset(effective_addr, labels, annotations)
                .then(|| tier2_symbolic_offset(effective_addr, annotations))
                .flatten()
                .filter(|sym| !is_zero_page_mode || sym.base.0 <= 0x00FF)
        })
}

/// Resolves the active [`SymbolicOffset`] while honoring explicit [`LabelKind::User`] > Tier 2
/// precedence (`Tier 1 > User > Tier 2`).
#[must_use]
pub(crate) fn resolve_effective_symbolic_offset(
    site_addr: Addr,
    effective_addr: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    annotations: &AnnotationManager,
) -> Option<SymbolicOffset> {
    resolve_effective_symbolic_offset_for_mode(
        site_addr,
        effective_addr,
        false,
        labels,
        annotations,
    )
}

/// Resolves the active [`SymbolicOffset`] for a split byte-table pair `(first_addr, second_addr)`
/// targeting `effective_addr`.
///
/// Checks Tier 1 on both halves before falling back to unshadowed Tier 2.
#[must_use]
pub(crate) fn resolve_split_pair_effective_symbolic_offset(
    first_addr: Addr,
    second_addr: Addr,
    effective_addr: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    annotations: &AnnotationManager,
) -> Option<SymbolicOffset> {
    tier1_symbolic_offset(first_addr, effective_addr, annotations)
        .or_else(|| tier1_symbolic_offset(second_addr, effective_addr, annotations))
        .or_else(|| {
            has_active_address_offset(effective_addr, labels, annotations)
                .then(|| tier2_symbolic_offset(effective_addr, annotations))
                .flatten()
        })
}

/// Formats `sym` into an assembler expression while guarding against chained Tier 2 suppression.
///
/// If `sym.base` carries an unshadowed Tier 2 `address_offset`, its `System` or `Auto` label
/// definition is suppressed from the output, so `format_base_hex(sym.base)` renders a valid hex literal.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn format_symbolic_offset_expr(
    sym: SymbolicOffset,
    labels: &BTreeMap<Addr, Vec<Label>>,
    settings: &DocumentSettings,
    annotations: &AnnotationManager,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    label_scope_names: Option<&ScopeNameMap>,
    current_scope_name: Option<&str>,
    scope_separator: &str,
    local_prefix: Option<&str>,
    parens: OffsetParentheses,
    format_base_hex: impl FnOnce(Addr) -> String,
) -> String {
    let base_name = (!has_active_address_offset(sym.base, labels, annotations))
        .then(|| {
            resolve_label_name(
                sym.base,
                labels,
                settings,
                local_label_names,
                label_scope_names,
                current_scope_name,
                scope_separator,
                local_prefix,
            )
        })
        .flatten()
        .unwrap_or_else(|| format_base_hex(sym.base));
    sym.format_expression(&base_name, parens)
}

/// Shared 5-tier symbol and symbolic-offset expression resolver given a pre-resolved active [`SymbolicOffset`].
#[must_use]
#[allow(clippy::too_many_arguments)]
fn format_active_offset_or_label(
    active_offset: Option<SymbolicOffset>,
    effective_addr: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    settings: &DocumentSettings,
    annotations: &AnnotationManager,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    label_scope_names: Option<&ScopeNameMap>,
    current_scope_name: Option<&str>,
    scope_separator: &str,
    local_prefix: Option<&str>,
    parens: OffsetParentheses,
    format_base_hex: impl FnOnce(Addr) -> String,
) -> Option<String> {
    if let Some(sym) = active_offset {
        return Some(format_symbolic_offset_expr(
            sym,
            labels,
            settings,
            annotations,
            local_label_names,
            label_scope_names,
            current_scope_name,
            scope_separator,
            local_prefix,
            parens,
            format_base_hex,
        ));
    }

    resolve_label_name(
        effective_addr,
        labels,
        settings,
        local_label_names,
        label_scope_names,
        current_scope_name,
        scope_separator,
        local_prefix,
    )
}

/// Resolves a formatted symbol or [`SymbolicOffset`] expression for `effective_addr` at `site_addr`.
///
/// Enforces the 5-tier precedence order:
///
/// 1. Tier 1 (`operand_offset` on `site_addr`)
/// 2. Explicit [`LabelKind::User`] label at `effective_addr`
/// 3. Tier 2 (`address_offset` on `effective_addr`)
/// 4. [`LabelKind::System`] label at `effective_addr`
/// 5. [`LabelKind::Auto`] label at `effective_addr`
///
/// When `parens` is [`OffsetParentheses::Parenthesized`] (inside `#<`, `#>`, `<`, and `>` byte
/// extractors), non-zero offset expressions wrap in parentheses (`"(sub_C100 - 1)"`). When
/// `sym.base` is unlabelled, excluded, or suppressed by Tier 2, `format_base_hex` renders the
/// fallback hex base.
///
/// # Examples
///
/// ```
/// use regenerator2000_core::disassembler::resolve_symbolic_label_name;
/// use regenerator2000_core::state::{
///     Addr, AnnotationManager, DocumentSettings, Label, LabelKind, LabelType, OffsetParentheses,
///     OffsetRadix, SymbolicOffset,
/// };
/// use std::collections::BTreeMap;
///
/// let mut labels = BTreeMap::new();
/// labels.insert(
///     Addr(0xC100),
///     vec![Label {
///         name: "sub_C100".to_string(),
///         kind: LabelKind::User,
///         label_type: LabelType::Subroutine,
///     }],
/// );
/// let mut annotations = AnnotationManager::default();
/// annotations.update(Addr(0x2000), |e| {
///     e.operand_offset = Some(SymbolicOffset {
///         base: Addr(0xC100),
///         offset: -1,
///         radix: OffsetRadix::Decimal,
///     });
/// });
///
/// let settings = DocumentSettings::default();
/// let unparenthesized = resolve_symbolic_label_name(
///     Addr(0x2000),
///     Addr(0xC0FF),
///     &labels,
///     &settings,
///     &annotations,
///     None,
///     None,
///     None,
///     ".",
///     Some("_"),
///     OffsetParentheses::Bare,
///     |base| format!("${base:04x}"),
/// );
/// assert_eq!(unparenthesized.as_deref(), Some("sub_C100 - 1"));
///
/// let parenthesized = resolve_symbolic_label_name(
///     Addr(0x2000),
///     Addr(0xC0FF),
///     &labels,
///     &settings,
///     &annotations,
///     None,
///     None,
///     None,
///     ".",
///     Some("_"),
///     OffsetParentheses::Parenthesized,
///     |base| format!("${base:04x}"),
/// );
/// assert_eq!(parenthesized.as_deref(), Some("(sub_C100 - 1)"));
/// ```
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn resolve_symbolic_label_name(
    site_addr: Addr,
    effective_addr: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    settings: &DocumentSettings,
    annotations: &AnnotationManager,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    label_scope_names: Option<&ScopeNameMap>,
    current_scope_name: Option<&str>,
    scope_separator: &str,
    local_prefix: Option<&str>,
    parens: OffsetParentheses,
    format_base_hex: impl FnOnce(Addr) -> String,
) -> Option<String> {
    resolve_symbolic_label_name_for_mode(
        site_addr,
        effective_addr,
        false,
        labels,
        settings,
        annotations,
        local_label_names,
        label_scope_names,
        current_scope_name,
        scope_separator,
        local_prefix,
        parens,
        format_base_hex,
    )
}

/// Resolves a symbol or [`SymbolicOffset`] expression and enforces zero-page base bounds when `is_zero_page_mode` is `true`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_symbolic_label_name_for_mode(
    site_addr: Addr,
    effective_addr: Addr,
    is_zero_page_mode: bool,
    labels: &BTreeMap<Addr, Vec<Label>>,
    settings: &DocumentSettings,
    annotations: &AnnotationManager,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    label_scope_names: Option<&ScopeNameMap>,
    current_scope_name: Option<&str>,
    scope_separator: &str,
    local_prefix: Option<&str>,
    parens: OffsetParentheses,
    format_base_hex: impl FnOnce(Addr) -> String,
) -> Option<String> {
    let active_offset = resolve_effective_symbolic_offset_for_mode(
        site_addr,
        effective_addr,
        is_zero_page_mode,
        labels,
        annotations,
    );
    format_active_offset_or_label(
        active_offset,
        effective_addr,
        labels,
        settings,
        annotations,
        local_label_names,
        label_scope_names,
        current_scope_name,
        scope_separator,
        local_prefix,
        parens,
        format_base_hex,
    )
}

/// Resolves a formatted symbol or [`SymbolicOffset`] expression for a split byte-table pair
/// `(first_addr, second_addr)` targeting `effective_addr`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_split_pair_symbolic_label_name(
    first_addr: Addr,
    second_addr: Addr,
    effective_addr: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    settings: &DocumentSettings,
    annotations: &AnnotationManager,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    label_scope_names: Option<&ScopeNameMap>,
    current_scope_name: Option<&str>,
    scope_separator: &str,
    local_prefix: Option<&str>,
    parens: OffsetParentheses,
    format_base_hex: impl FnOnce(Addr) -> String,
) -> Option<String> {
    let active_offset = resolve_split_pair_effective_symbolic_offset(
        first_addr,
        second_addr,
        effective_addr,
        labels,
        annotations,
    );
    format_active_offset_or_label(
        active_offset,
        effective_addr,
        labels,
        settings,
        annotations,
        local_label_names,
        label_scope_names,
        current_scope_name,
        scope_separator,
        local_prefix,
        parens,
        format_base_hex,
    )
}

/// Computes local label names within `[start_pc, end_pc]` from raw state slices.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_local_label_names_in_range(
    opcodes: &[Option<Opcode>; 256],
    data: &[u8],
    origin: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    annotations: &AnnotationManager,
    start_pc: usize,
    end_pc: usize,
    formatter: &dyn Formatter,
) -> BTreeMap<Addr, String> {
    let mut local_names = BTreeMap::new();
    let mut local_count = 0;

    let mut current_pc = start_pc;
    while current_pc <= end_pc && current_pc < data.len() {
        let bytes_consumed = data
            .get(current_pc)
            .and_then(|&b| opcodes.get(b as usize).and_then(|op| op.as_ref()))
            .map_or(1, |op| op.size as usize);

        for offset in 0..bytes_consumed {
            let check_pc = current_pc + offset;
            if check_pc > end_pc {
                break;
            }
            if check_pc == start_pc {
                continue;
            }

            let current_addr = origin.wrapping_add(check_pc as u16);
            // Skip addresses suppressed by Tier 2 `address_offset` so sequential local label
            // numbering (`l00`, `l01`, ...) stays contiguous even when `auto_analyze == false`.
            if !has_active_address_offset(current_addr, labels, annotations)
                && let Some(labels_at_addr) = labels.get(&current_addr)
            {
                let has_user_or_system = labels_at_addr
                    .iter()
                    .any(|l| l.kind == LabelKind::User || l.kind == LabelKind::System);

                if !has_user_or_system && let Some(name) = formatter.format_local_label(local_count)
                {
                    local_names.insert(current_addr, name);
                }
                local_count += 1;
            }
        }
        current_pc += bytes_consumed;
    }

    local_names
}

/// Computes local label names within the byte-offset range `[start_pc, end_pc]`.
#[must_use]
pub fn compute_local_label_names(
    opcodes: &[Option<Opcode>; 256],
    ctx: &DisassemblyContext<'_>,
    start_pc: usize,
    end_pc: usize,
    formatter: &dyn Formatter,
) -> BTreeMap<Addr, String> {
    compute_local_label_names_in_range(
        opcodes,
        ctx.data,
        ctx.origin,
        ctx.labels,
        ctx.annotations,
        start_pc,
        end_pc,
        formatter,
    )
}

/// Maps annotated lexical scopes to their enclosing scope names in `O(S)` time.
#[must_use]
pub fn compute_scope_names(
    ctx: &DisassemblyContext<'_>,
    _formatter: &dyn Formatter,
) -> ScopeNameMap {
    let mut scope_names = ScopeNameMap::default();
    if ctx.data.is_empty() {
        return scope_names;
    }
    for (&start, &end) in ctx.annotations.scopes() {
        // Only register a named scope header when `start` has a visible label not suppressed by Tier 2.
        if !has_active_address_offset(start, ctx.labels, ctx.annotations)
            && let Some(v) = ctx.labels.get(&start)
            && let Some(label) = resolve_label(v, start.0, ctx.settings)
        {
            let Some(start_off) = start
                .0
                .checked_sub(ctx.origin.0)
                .filter(|&off| usize::from(off) < ctx.data.len())
            else {
                continue;
            };
            let Some(end_off) = end.0.checked_sub(ctx.origin.0) else {
                continue;
            };
            let clamped_end_off = usize::from(end_off).min(ctx.data.len() - 1);
            if usize::from(start_off) <= clamped_end_off {
                scope_names.insert_range(
                    ctx.origin.wrapping_add(start_off),
                    ctx.origin.wrapping_add(clamped_end_off as u16),
                    label.name.clone(),
                );
            }
        }
    }
    scope_names
}

/// Gets the formatted label name at `address`, if any.
#[must_use]
pub fn get_label_name(
    address: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    formatter: &dyn Formatter,
    settings: &DocumentSettings,
) -> Option<String> {
    let v = labels.get(&address)?;
    let l = resolve_label(v, address.0, settings)?;
    let prefixed = apply_local_prefix(&l.name, l.label_type, formatter.local_label_prefix());
    Some(formatter.format_label(&prefixed))
}

/// Resolves the line label at `address`, suppressing addresses with an active Tier 2 `address_offset`
/// and checking `local_label_names` before [`get_label_name`].
#[must_use]
pub(crate) fn resolve_line_label(
    address: Addr,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    labels: &BTreeMap<Addr, Vec<Label>>,
    annotations: &AnnotationManager,
    formatter: &dyn Formatter,
    settings: &DocumentSettings,
) -> Option<String> {
    if has_active_address_offset(address, labels, annotations) {
        return None;
    }
    if let Some(local_name) = local_label_names.and_then(|names| names.get(&address)) {
        return Some(local_name.clone());
    }
    get_label_name(address, labels, formatter, settings)
}

/// Gets the control-flow target address for a decoded `JMP`, `JSR`, or branch opcode.
#[must_use]
pub fn get_arrow_target_address_for_opcode(
    opcode: &Opcode,
    bytes: &[u8],
    address: Addr,
) -> Option<Addr> {
    use crate::cpu::AddressingMode;

    if !opcode.is_flow_control_with_target() {
        return None;
    }

    match opcode.mode {
        AddressingMode::Absolute => {
            let b1 = *bytes.get(1)?;
            let b2 = *bytes.get(2)?;
            Some(Addr(u16::from(b2) << 8 | u16::from(b1)))
        }
        AddressingMode::Relative => {
            let b1 = *bytes.get(1)?;
            let offset = b1 as i8;
            Some(address.wrapping_add(2).wrapping_add(offset as u16))
        }
        _ => None,
    }
}

/// Gets the memory or branch address that a decoded opcode references.
#[must_use]
pub fn get_referenced_address_for_opcode(
    opcode: &Opcode,
    bytes: &[u8],
    address: Addr,
) -> Option<Addr> {
    use crate::cpu::AddressingMode;

    match opcode.mode {
        AddressingMode::Absolute
        | AddressingMode::AbsoluteX
        | AddressingMode::AbsoluteY
        | AddressingMode::Indirect => {
            let b1 = *bytes.get(1)?;
            let b2 = *bytes.get(2)?;
            Some(Addr(u16::from(b2) << 8 | u16::from(b1)))
        }
        AddressingMode::ZeroPage
        | AddressingMode::ZeroPageX
        | AddressingMode::ZeroPageY
        | AddressingMode::IndirectX
        | AddressingMode::IndirectY => {
            let b1 = *bytes.get(1)?;
            Some(Addr(u16::from(b1)))
        }
        AddressingMode::Relative => {
            let b1 = *bytes.get(1)?;
            let offset = b1 as i8;
            Some(address.wrapping_add(2).wrapping_add(offset as u16))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::OffsetRadix;

    #[test]
    fn test_resolve_label_precedence() {
        let settings = DocumentSettings::default();
        let labels = vec![
            Label {
                name: "auto_label".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::Subroutine,
            },
            Label {
                name: "user_label".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            },
            Label {
                name: "sys_label".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Subroutine,
            },
        ];

        let resolved = resolve_label(&labels, 0x1000, &settings);
        assert_eq!(resolved.map(|l| l.name.as_str()), Some("user_label"));
    }

    #[test]
    fn test_resolve_symbolic_label_five_tier_precedence() {
        let settings = DocumentSettings::default();
        let site_addr = Addr(0x1020);
        let mut labels: BTreeMap<Addr, Vec<Label>> = BTreeMap::new();
        let mut annotations = AnnotationManager::default();

        // Base labels
        labels.insert(
            Addr(0x2000),
            vec![Label {
                name: "table_2000".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        // 1. Tier 1 (operand_offset) > Explicit User label AND Tier 2 (address_offset)
        labels.insert(
            Addr(0x1FFF),
            vec![Label {
                name: "rts_label".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        annotations.update(Addr(0x1FFF), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x1FFE),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        annotations.update(site_addr, |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x2000),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });

        let resolved_t1 = resolve_symbolic_label_name(
            site_addr,
            Addr(0x1FFF),
            &labels,
            &settings,
            &annotations,
            None,
            None,
            None,
            ".",
            None,
            OffsetParentheses::Bare,
            |a| format!("${a:04x}"),
        );
        assert_eq!(resolved_t1.as_deref(), Some("table_2000 - 1"));

        // 2. Explicit User label > Tier 2 (address_offset) when Tier 1 is None (defense-in-depth)
        labels.insert(
            Addr(0x00FC),
            vec![Label {
                name: "explicit_fc".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        annotations.update(Addr(0x00FC), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        let resolved_user_over_t2 = resolve_symbolic_label_name(
            Addr(0x1050),
            Addr(0x00FC),
            &labels,
            &settings,
            &annotations,
            None,
            None,
            None,
            ".",
            None,
            OffsetParentheses::Bare,
            |a| format!("${a:02x}"),
        );
        assert_eq!(resolved_user_over_t2.as_deref(), Some("explicit_fc"));

        // 3. Tier 2 (address_offset) > System and Auto labels
        labels.insert(
            Addr(0x00FC),
            vec![
                Label {
                    name: "SYS_FC".to_string(),
                    kind: LabelKind::System,
                    label_type: LabelType::Predefined,
                },
                Label {
                    name: "zpa_FC".to_string(),
                    kind: LabelKind::Auto,
                    label_type: LabelType::ZeroPageAbsoluteAddress,
                },
            ],
        );
        let resolved_t2 = resolve_symbolic_label_name(
            Addr(0x1050),
            Addr(0x00FC),
            &labels,
            &settings,
            &annotations,
            None,
            None,
            None,
            ".",
            None,
            OffsetParentheses::Bare,
            |a| format!("${a:02x}"),
        );
        assert_eq!(resolved_t2.as_deref(), Some("zp_ptr + 1"));

        // 4. System > Auto > None when no symbolic offsets exist
        annotations.update(Addr(0x00FC), |e| e.address_offset = None);
        let resolved_sys = resolve_symbolic_label_name(
            Addr(0x1050),
            Addr(0x00FC),
            &labels,
            &settings,
            &annotations,
            None,
            None,
            None,
            ".",
            None,
            OffsetParentheses::Bare,
            |a| format!("${a:02x}"),
        );
        assert_eq!(resolved_sys.as_deref(), Some("SYS_FC"));

        labels
            .get_mut(&Addr(0x00FC))
            .unwrap()
            .retain(|l| l.kind == LabelKind::Auto);
        let resolved_auto = resolve_symbolic_label_name(
            Addr(0x1050),
            Addr(0x00FC),
            &labels,
            &settings,
            &annotations,
            None,
            None,
            None,
            ".",
            None,
            OffsetParentheses::Bare,
            |a| format!("${a:02x}"),
        );
        assert_eq!(resolved_auto.as_deref(), Some("zpa_FC"));

        let resolved_none = resolve_symbolic_label_name(
            Addr(0x1050),
            Addr(0x00FE),
            &labels,
            &settings,
            &annotations,
            None,
            None,
            None,
            ".",
            None,
            OffsetParentheses::Bare,
            |a| format!("${a:02x}"),
        );
        assert_eq!(resolved_none, None);

        // 5. Mismatched target ignored ($3000 - 1 = $2FFF != $1FFF)
        annotations.update(site_addr, |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x3000),
                offset: -1,
                radix: OffsetRadix::Decimal,
            });
        });
        assert_eq!(
            resolve_symbolic_offset(site_addr, Addr(0x1FFF), &annotations),
            SymbolicOffset::from_base_and_effective(
                Addr(0x1FFE),
                Addr(0x1FFF),
                OffsetRadix::Decimal,
            )
        );
        annotations.update(Addr(0x1FFF), |e| e.address_offset = None);
        assert_eq!(
            resolve_symbolic_offset(site_addr, Addr(0x1FFF), &annotations),
            None
        );
    }

    #[test]
    fn test_resolve_symbolic_label_rename_propagation_scopes_and_unlabelled_base() {
        let settings = DocumentSettings::default();
        let mut labels: BTreeMap<Addr, Vec<Label>> = BTreeMap::new();
        let mut annotations = AnnotationManager::default();

        // 1. Dynamic rename propagation without touching $00FC
        labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        annotations.update(Addr(0x00FC), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x1000),
                Addr(0x00FC),
                &labels,
                &settings,
                &annotations,
                None,
                None,
                None,
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:02x}"),
            )
            .as_deref(),
            Some("zp_ptr + 1")
        );

        labels.get_mut(&Addr(0x00FB)).unwrap()[0].name = "screen_ptr".to_string();
        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x1000),
                Addr(0x00FC),
                &labels,
                &settings,
                &annotations,
                None,
                None,
                None,
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:02x}"),
            )
            .as_deref(),
            Some("screen_ptr + 1")
        );

        // 2. Scoped base from outside vs inside scope ("irq.ptr + 1", "irq::ptr + 1", "ptr + 1")
        labels.insert(
            Addr(0x1010),
            vec![Label {
                name: "ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        let mut scope_names = ScopeNameMap::default();
        assert!(scope_names.is_empty());
        scope_names.insert_range(Addr(0x1000), Addr(0x1020), "irq".to_string());
        assert!(!scope_names.is_empty());
        assert_eq!(scope_names.get(Addr(0x0FFF)), None);
        assert_eq!(scope_names.get(Addr(0x1000)), Some("irq"));
        assert_eq!(scope_names.get(Addr(0x1010)), Some("irq"));
        assert_eq!(scope_names.get(Addr(0x1020)), Some("irq"));
        assert_eq!(scope_names.get(Addr(0x1021)), None);
        annotations.update(Addr(0x1011), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x1010),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });

        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x2000),
                Addr(0x1011),
                &labels,
                &settings,
                &annotations,
                None,
                Some(&scope_names),
                None,
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:04x}"),
            )
            .as_deref(),
            Some("irq.ptr + 1")
        );
        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x2000),
                Addr(0x1011),
                &labels,
                &settings,
                &annotations,
                None,
                Some(&scope_names),
                None,
                "::",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:04x}"),
            )
            .as_deref(),
            Some("irq::ptr + 1")
        );
        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x1012),
                Addr(0x1011),
                &labels,
                &settings,
                &annotations,
                None,
                Some(&scope_names),
                Some("irq"),
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:04x}"),
            )
            .as_deref(),
            Some("ptr + 1")
        );

        // 3. LocalUserDefined base with assembler local prefixes (_, ., @, None)
        labels.insert(
            Addr(0x00FD),
            vec![Label {
                name: "tmp".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::LocalUserDefined,
            }],
        );
        annotations.update(Addr(0x00FE), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x00FD),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        for (prefix, expected) in [
            (Some("_"), "_tmp + 1"),
            (Some("."), ".tmp + 1"),
            (Some("@"), "@tmp + 1"),
            (None, "tmp + 1"),
        ] {
            assert_eq!(
                resolve_symbolic_label_name(
                    Addr(0x1000),
                    Addr(0x00FE),
                    &labels,
                    &settings,
                    &annotations,
                    None,
                    None,
                    None,
                    ".",
                    prefix,
                    OffsetParentheses::Bare,
                    |a| format!("${a:02x}"),
                )
                .as_deref(),
                Some(expected)
            );
        }

        // 4. Unlabelled / excluded base invokes format_base_hex
        annotations.update(Addr(0x00EE), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x00ED),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        annotations.update(Addr(0x0428), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x0400),
                offset: 40,
                radix: OffsetRadix::Decimal,
            });
        });
        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x1000),
                Addr(0x00EE),
                &labels,
                &settings,
                &annotations,
                None,
                None,
                None,
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:02x}"),
            )
            .as_deref(),
            Some("$ed + 1")
        );
        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x1000),
                Addr(0x0428),
                &labels,
                &settings,
                &annotations,
                None,
                None,
                None,
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:04x}"),
            )
            .as_deref(),
            Some("$0400 + 40")
        );
    }

    #[test]
    fn test_resolve_symbolic_label_suppressed_base_falls_back_to_hex_unless_user_label() {
        let settings = DocumentSettings::default();
        let mut labels: BTreeMap<Addr, Vec<Label>> = BTreeMap::new();
        let mut annotations = AnnotationManager::default();

        // 1. Tier 1 (`operand_offset`) referencing `$D021` (`System` "BGCOL0") when `$D021` itself
        //    has an active Tier 2 `address_offset` (`EXTCOL + 1`) — `BGCOL0` is suppressed from the
        //    preamble, so `sym.base == $D021` MUST fall back to `$d021`!
        labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "EXTCOL".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Predefined,
            }],
        );
        labels.insert(
            Addr(0xD021),
            vec![Label {
                name: "BGCOL0".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Predefined,
            }],
        );
        annotations.update(Addr(0xD021), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0xD020),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        annotations.update(Addr(0x1000), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0xD021),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });

        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x1000),
                Addr(0xD022),
                &labels,
                &settings,
                &annotations,
                None,
                None,
                None,
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:04x}"),
            )
            .as_deref(),
            Some("$d021 + 1")
        );
        assert_eq!(
            resolve_split_pair_symbolic_label_name(
                Addr(0x1000),
                Addr(0x1001),
                Addr(0xD022),
                &labels,
                &settings,
                &annotations,
                None,
                None,
                None,
                ".",
                None,
                OffsetParentheses::Parenthesized,
                |a| format!("${a:04x}"),
            )
            .as_deref(),
            Some("($d021 + 1)")
        );

        // 2. Tier 2 (`address_offset`) at `$00FD` referencing `$00FC` (`Auto` "zpa_FC") when `$00FC`
        //    itself has an active Tier 2 `address_offset` (`zp_ptr + 1`) — must fall back to `$fc + 1`!
        labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        labels.insert(
            Addr(0x00FC),
            vec![Label {
                name: "zpa_FC".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::ZeroPageAbsoluteAddress,
            }],
        );
        annotations.update(Addr(0x00FC), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });
        annotations.update(Addr(0x00FD), |e| {
            e.address_offset = Some(SymbolicOffset {
                base: Addr(0x00FC),
                offset: 1,
                radix: OffsetRadix::Decimal,
            });
        });

        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x1010),
                Addr(0x00FD),
                &labels,
                &settings,
                &annotations,
                None,
                None,
                None,
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:02x}"),
            )
            .as_deref(),
            Some("$fc + 1")
        );

        // 3. Defense-in-depth: if `$00FC` also carries an explicit `LabelKind::User` label,
        //    `has_active_address_offset($00FC)` is `false` (`User` > Tier 2), so the defined user
        //    label IS used as the base symbol.
        labels.get_mut(&Addr(0x00FC)).unwrap().push(Label {
            name: "ptr_mid".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::UserDefined,
        });
        assert_eq!(
            resolve_symbolic_label_name(
                Addr(0x1010),
                Addr(0x00FD),
                &labels,
                &settings,
                &annotations,
                None,
                None,
                None,
                ".",
                None,
                OffsetParentheses::Bare,
                |a| format!("${a:02x}"),
            )
            .as_deref(),
            Some("ptr_mid + 1")
        );
    }
}
