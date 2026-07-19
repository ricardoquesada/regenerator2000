//! Commodore BASIC tokenized stub parser utilities.

use crate::state::types::System;

/// BASIC token for `SYS` on Commodore 8-bit systems (V2).
const SYS_TOKEN: u8 = 0x9E;

/// BASIC tokens for arithmetic operators.
const TOKEN_PLUS: u8 = 0xAA;
const TOKEN_MINUS: u8 = 0xAB;
const TOKEN_MULTIPLY: u8 = 0xAC;
const TOKEN_DIVIDE: u8 = 0xAD;

/// Parses a Commodore BASIC `SYS` line from memory/file data to find the entry point.
///
/// Handles:
/// - Simple: `SYS 2061`
/// - With spaces/parens: `SYS (2061)` or `SYS  2061`
/// - With arithmetic: `SYS 2048+16`, `SYS 2048*1+13`
/// - Multi-line BASIC programs (iterates line pointer chain)
///
/// # System Scope & Non-Commodore BASIC Guard
/// This parser specifically targets Commodore 8-bit BASIC V2 tokenized stubs (C64, C128, VIC-20, PET, Plus/4).
/// Non-Commodore BASIC systems (e.g. Atari, Apple II, Oric) use different token structures or keywords (`RUN`, `CALL`, `USR`).
/// If a `system` is provided and it is not a Commodore BASIC system, this function returns `None`.
#[must_use]
pub fn find_sys_address(
    mem: &[u8],
    start_offset: usize,
    load_origin: Option<u16>,
    system: Option<&System>,
) -> Option<u16> {
    if let Some(sys) = system
        && !sys.is_commodore_basic()
    {
        return None;
    }

    let mut offset = start_offset;
    let limit = start_offset.saturating_add(0x100);

    while offset + 4 < mem.len() && offset < limit {
        let next_ptr = u16::from_le_bytes([mem[offset], mem[offset + 1]]);

        // Calculate offset of next line
        let next_offset = if let Some(origin) = load_origin {
            if let Some(off) = (next_ptr as usize).checked_sub(origin as usize) {
                off.saturating_add(start_offset)
            } else {
                0
            }
        } else {
            next_ptr as usize
        };

        if next_ptr == 0 || next_offset <= offset + 4 || next_offset > mem.len() {
            // Fallback: search for SYS token in single-line window if line pointer is missing/invalid
            let fallback_end = (offset + 0x100).min(mem.len());
            let mut pos = offset + 4;
            while pos < fallback_end {
                if mem[pos] == 0x00 {
                    break;
                }
                if mem[pos] == SYS_TOKEN {
                    return parse_sys_expression(mem, pos + 1, fallback_end);
                }
                pos += 1;
            }
            break;
        }

        // Search for SYS token in current line
        let line_end = next_offset.min(mem.len());
        let mut pos = offset + 4;

        while pos < line_end && pos < mem.len() {
            if mem[pos] == SYS_TOKEN {
                pos += 1;
                if let Some(addr) = parse_sys_expression(mem, pos, line_end) {
                    return Some(addr);
                }
                break;
            }
            pos += 1;
        }

        offset = next_offset;
    }

    None
}

/// Helper to parse numeric value or arithmetic expression following SYS token.
fn parse_sys_expression(mem: &[u8], mut pos: usize, line_end: usize) -> Option<u16> {
    // Skip spaces and opening parentheses
    while pos < line_end && (mem[pos] == b' ' || mem[pos] == b'(') {
        pos += 1;
    }

    let mut value: u32 = 0;
    let mut found_digit = false;
    while pos < line_end && mem[pos].is_ascii_digit() {
        value = value
            .wrapping_mul(10)
            .wrapping_add(u32::from(mem[pos] - b'0'));
        found_digit = true;
        pos += 1;
    }

    if !found_digit {
        return None;
    }

    // Handle arithmetic operators (tokenized BASIC)
    while pos < line_end {
        let op = mem[pos];
        if op != TOKEN_PLUS && op != TOKEN_MINUS && op != TOKEN_MULTIPLY && op != TOKEN_DIVIDE {
            break;
        }
        pos += 1;

        while pos < line_end && mem[pos] == b' ' {
            pos += 1;
        }

        let mut operand: u32 = 0;
        let mut found_operand = false;
        while pos < line_end && mem[pos].is_ascii_digit() {
            operand = operand
                .wrapping_mul(10)
                .wrapping_add(u32::from(mem[pos] - b'0'));
            found_operand = true;
            pos += 1;
        }

        if !found_operand {
            break;
        }

        match op {
            TOKEN_PLUS => value = value.wrapping_add(operand),
            TOKEN_MINUS => value = value.wrapping_sub(operand),
            TOKEN_MULTIPLY => value = value.wrapping_mul(operand),
            TOKEN_DIVIDE => {
                if let Some(result) = value.checked_div(operand) {
                    value = result;
                }
            }
            _ => break,
        }
    }

    if value <= 0xFFFF {
        Some(value as u16)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sys_simple() {
        // SYS 2061
        let mem = vec![
            0x0B, 0x08, 0x0A, 0x00, 0x9E, b' ', b'2', b'0', b'6', b'1', 0x00,
        ];
        assert_eq!(find_sys_address(&mem, 0, None, None), Some(2061));
    }

    #[test]
    fn test_parse_sys_arithmetic() {
        // SYS 2048+16 -> 2064
        let mem = vec![
            0x0E, 0x08, 0x0A, 0x00, 0x9E, b' ', b'2', b'0', b'4', b'8', TOKEN_PLUS, b'1', b'6',
            0x00,
        ];
        assert_eq!(find_sys_address(&mem, 0, None, None), Some(2064));
    }

    #[test]
    fn test_non_commodore_system_returns_none() {
        let mem = vec![
            0x0B, 0x08, 0x0A, 0x00, 0x9E, b' ', b'2', b'0', b'6', b'1', 0x00,
        ];
        let atari = System::new("Atari 8-bit");
        assert_eq!(find_sys_address(&mem, 0, None, Some(&atari)), None);
    }
}
