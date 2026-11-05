//! Symbol precedence resolution, lexical scope mapping, and operand target extraction.
//!
//! This module resolves the highest-priority [`Label`] at an address, formats scoped and local
//! symbol names, and extracts target addresses from decoded 6502 instructions.

use crate::cpu::Opcode;
use crate::disassembler::context::DisassemblyContext;
use crate::disassembler::formatter::Formatter;
use crate::state::{Addr, DocumentSettings, Label, LabelKind, LabelType};
use std::collections::BTreeMap;

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
    label_scope_names: Option<&BTreeMap<Addr, String>>,
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
    if let Some(scope_name) = label_scope_names.and_then(|scopes| scopes.get(&address)) {
        let same_scope = current_scope_name.is_some_and(|curr| curr == scope_name);
        if !same_scope && &base_name != scope_name {
            return Some(format!("{scope_name}{scope_separator}{base_name}"));
        }
    }

    Some(base_name)
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
    let mut local_names = BTreeMap::new();
    let mut local_count = 0;

    let mut current_pc = start_pc;
    while current_pc <= end_pc && current_pc < ctx.data.len() {
        let bytes_consumed = ctx
            .data
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

            let current_addr = ctx.origin.wrapping_add(check_pc as u16);
            if let Some(labels_at_addr) = ctx.labels.get(&current_addr) {
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

/// Maps all addresses inside annotated lexical scopes to their enclosing scope names.
#[must_use]
pub fn compute_scope_names(
    ctx: &DisassemblyContext<'_>,
    _formatter: &dyn Formatter,
) -> BTreeMap<Addr, String> {
    let mut scope_names = BTreeMap::new();
    for (start, end) in ctx
        .annotations
        .iter()
        .filter_map(|(s, e)| e.scope.map(|end| (s, end)))
    {
        if let Some(v) = ctx.labels.get(&start)
            && let Some(label) = resolve_label(v, start.0, ctx.settings)
        {
            let name = label.name.clone();
            let start_pc = match start.0.checked_sub(ctx.origin.0) {
                Some(off) if (off as usize) < ctx.data.len() => off as usize,
                _ => continue,
            };
            let end_pc = match end.0.checked_sub(ctx.origin.0) {
                Some(off) => (off as usize).min(ctx.data.len().saturating_sub(1)),
                _ => continue,
            };

            for i in start_pc..=end_pc {
                let addr = ctx.origin.wrapping_add(i as u16);
                scope_names.insert(addr, name.clone());
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

/// Resolves the line label at `address`, checking `local_label_names` before [`get_label_name`].
#[must_use]
pub(crate) fn resolve_line_label(
    address: Addr,
    local_label_names: Option<&BTreeMap<Addr, String>>,
    labels: &BTreeMap<Addr, Vec<Label>>,
    formatter: &dyn Formatter,
    settings: &DocumentSettings,
) -> Option<String> {
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
}
