use crate::cpu::Opcode;
use crate::disassembler::context::DisassemblyContext;
use crate::disassembler::formatter::Formatter;
use crate::state::{Addr, DocumentSettings, Label, LabelKind, LabelType};
use std::collections::BTreeMap;

/// Resolves the single label with the highest priority score at a given address.
///
/// Precedence:
/// 1. User label (score 100)
/// 2. System label (score 50)
/// 3. Auto label (score 0)
///
/// Tie-break: alphabetically smaller label name.
#[must_use]
pub fn resolve_label<'a>(
    labels: &'a [Label],
    _address: u16,
    _settings: &DocumentSettings,
) -> Option<&'a Label> {
    if labels.is_empty() {
        return None;
    }

    let get_priority = |k: &LabelKind| -> u8 {
        match k {
            LabelKind::User => 100,
            LabelKind::System => 50,
            LabelKind::Auto => 0,
        }
    };

    let mut best_label: Option<&Label> = None;

    for label in labels {
        if let Some(curr) = best_label {
            let curr_prio = get_priority(&curr.kind);
            let new_prio = get_priority(&label.kind);

            if new_prio > curr_prio || (new_prio == curr_prio && label.name < curr.name) {
                best_label = Some(label);
            }
        } else {
            best_label = Some(label);
        }
    }
    best_label
}

/// Resolves the label name for a given address considering scopes, local labels, and prefixes.
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
    let mut base_name_opt = None;
    let mut is_local_user_defined = false;

    // 1. Local label names (from formatter, e.g. l15)
    if let Some(names) = local_label_names
        && let Some(name) = names.get(&address)
    {
        base_name_opt = Some(name.clone());
    }

    // 2. Standard label resolution
    if base_name_opt.is_none()
        && let Some(v) = labels.get(&address)
        && let Some(l) = resolve_label(v, address.0, settings)
    {
        base_name_opt = Some(l.name.clone());
        is_local_user_defined = l.label_type == LabelType::LocalUserDefined;
    }

    let mut base_name = base_name_opt?;

    if is_local_user_defined
        && let Some(p) = local_prefix
        && !base_name.starts_with(p)
    {
        base_name = format!("{}{}", p, base_name);
    }

    // 3. Scope scoping
    if let Some(scope_names) = label_scope_names
        && let Some(scope_name) = scope_names.get(&address)
    {
        let same_scope = current_scope_name.is_some_and(|curr| curr == scope_name);
        if !same_scope && &base_name != scope_name {
            return Some(format!("{}{}{}", scope_name, scope_separator, base_name));
        }
    }

    Some(base_name)
}

/// Computes local label names within a given PC range `[start_pc, end_pc]`.
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
            let current_addr = ctx.origin.wrapping_add(check_pc as u16);

            if check_pc == start_pc {
                continue;
            }

            if let Some(labels_at_addr) = ctx.labels.get(&current_addr) {
                let has_user_or_system = labels_at_addr.iter().any(|l| {
                    l.kind == crate::state::LabelKind::User
                        || l.kind == crate::state::LabelKind::System
                });

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

/// Maps all addresses within annotated scopes to their scope names.
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

/// Helper for retrieving a formatted label name at `address`.
#[must_use]
pub fn get_label_name(
    address: Addr,
    labels: &BTreeMap<Addr, Vec<Label>>,
    formatter: &dyn Formatter,
    settings: &DocumentSettings,
) -> Option<String> {
    labels.get(&address).and_then(|v| {
        resolve_label(v, address.0, settings).map(|l| {
            let mut name = l.name.clone();
            if l.label_type == LabelType::LocalUserDefined
                && let Some(p) = formatter.local_label_prefix()
                && !name.starts_with(p)
            {
                name = format!("{}{}", p, name);
            }
            formatter.format_label(&name)
        })
    })
}

/// Returns the target address of flow control instructions (JMP, JSR, branches).
#[must_use]
pub fn get_arrow_target_address(
    opcodes: &[Option<Opcode>; 256],
    data: &[u8],
    pc: usize,
    origin: Addr,
) -> Option<Addr> {
    let opcode_byte = *data.get(pc)?;
    let opcode = opcodes.get(opcode_byte as usize)?.as_ref()?;
    let address = origin.wrapping_add(pc as u16);
    let end_idx = (pc + opcode.size as usize).min(data.len());
    let bytes = data.get(pc..end_idx)?;
    get_arrow_target_address_for_opcode(opcode, bytes, address)
}

/// Helper returning arrow target address for a decoded opcode.
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

/// Returns the address referenced by the instruction (memory access, jump, etc.).
#[must_use]
pub fn get_referenced_address(
    opcodes: &[Option<Opcode>; 256],
    data: &[u8],
    pc: usize,
    origin: Addr,
) -> Option<Addr> {
    let opcode_byte = *data.get(pc)?;
    let opcode = opcodes.get(opcode_byte as usize)?.as_ref()?;
    let address = origin.wrapping_add(pc as u16);
    let end_idx = (pc + opcode.size as usize).min(data.len());
    let bytes = data.get(pc..end_idx)?;
    get_referenced_address_for_opcode(opcode, bytes, address)
}

/// Helper returning referenced address for a decoded opcode.
#[must_use]
pub fn get_referenced_address_for_opcode(
    opcode: &Opcode,
    bytes: &[u8],
    address: Addr,
) -> Option<Addr> {
    use crate::cpu::AddressingMode;

    match opcode.mode {
        AddressingMode::Absolute | AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => {
            let b1 = *bytes.get(1)?;
            let b2 = *bytes.get(2)?;
            Some(Addr(u16::from(b2) << 8 | u16::from(b1)))
        }
        AddressingMode::ZeroPage | AddressingMode::ZeroPageX | AddressingMode::ZeroPageY => {
            let b1 = *bytes.get(1)?;
            Some(Addr(u16::from(b1)))
        }
        AddressingMode::Relative => {
            let b1 = *bytes.get(1)?;
            let offset = b1 as i8;
            Some(address.wrapping_add(2).wrapping_add(offset as u16))
        }
        AddressingMode::Indirect => {
            let b1 = *bytes.get(1)?;
            let b2 = *bytes.get(2)?;
            Some(Addr(u16::from(b2) << 8 | u16::from(b1)))
        }
        AddressingMode::IndirectX | AddressingMode::IndirectY => {
            let b1 = *bytes.get(1)?;
            Some(Addr(u16::from(b1)))
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
