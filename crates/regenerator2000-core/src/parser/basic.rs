//! Commodore BASIC V2 tokenized stub parser utilities.

use crate::state::types::System;

/// BASIC V2 token for `SYS` on Commodore 8-bit systems.
const SYS_TOKEN: u8 = 0x9E;

/// BASIC V2 tokens for arithmetic operators.
const TOKEN_PLUS: u8 = 0xAA;
const TOKEN_MINUS: u8 = 0xAB;
const TOKEN_MULTIPLY: u8 = 0xAC;
const TOKEN_DIVIDE: u8 = 0xAD;

/// Maximum byte window scanned from `start_offset` when searching for a `SYS` stub.
const MAX_SCAN_WINDOW: usize = 0x100;

/// Parses a Commodore BASIC `SYS` line from `mem` to find the machine-code entry point.
///
/// Supports:
///
/// - Simple targets: `SYS 2061`
/// - Spaces and parentheses: `SYS (2061)` or `SYS  2061`
/// - Tokenized arithmetic: `SYS 2048+16`, `SYS 2048 + 16`, `SYS (2048)+(16)`
/// - Multi-line BASIC headers via the 2-byte next-line pointer chain
///
/// # System Scope
///
/// This parser targets Commodore 8-bit BASIC V2 tokenized stubs (C64, C128, VIC-20, PET, Plus/4).
/// Non-Commodore systems use different token layouts or entry keywords.
/// If `system` is present and does not use Commodore BASIC, this function returns `None`.
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
    let limit = start_offset.saturating_add(MAX_SCAN_WINDOW);

    while offset.saturating_add(4) < mem.len() && offset < limit {
        let next_ptr = u16::from_le_bytes([mem[offset], mem[offset + 1]]);
        if next_ptr == 0 && offset > start_offset {
            break;
        }

        let next_offset = if let Some(origin) = load_origin {
            (next_ptr as usize)
                .checked_sub(origin as usize)
                .map_or(0, |off| off.saturating_add(start_offset))
        } else {
            next_ptr as usize
        };

        let line_body_start = offset + 4;
        if next_offset <= line_body_start || next_offset > mem.len() {
            // If the next-line pointer is out of bounds for `mem`, scan a single line up to NUL.
            let fallback_end = offset.saturating_add(MAX_SCAN_WINDOW).min(mem.len());
            return scan_line_for_sys(mem, line_body_start, fallback_end, true);
        }

        let line_end = next_offset.min(mem.len());
        if let Some(addr) = scan_line_for_sys(mem, line_body_start, line_end, false) {
            return Some(addr);
        }

        offset = next_offset;
    }

    None
}

/// Scans `mem[start..end]` for a `SYS` token (`0x9E`) and parses its target expression.
fn scan_line_for_sys(mem: &[u8], start: usize, end: usize, stop_at_null: bool) -> Option<u16> {
    let mut pos = start;
    while pos < end {
        match mem[pos] {
            0x00 if stop_at_null => break,
            SYS_TOKEN => return parse_sys_expression(mem, pos + 1, end),
            _ => {}
        }
        pos += 1;
    }
    None
}

/// Parses an unsigned decimal integer from `mem[*pos..line_end]` after skipping spaces and `(`.
fn parse_decimal(mem: &[u8], pos: &mut usize, line_end: usize) -> Option<u32> {
    while *pos < line_end && (mem[*pos] == b' ' || mem[*pos] == b'(') {
        *pos += 1;
    }

    let mut value: u32 = 0;
    let mut found_digit = false;
    while *pos < line_end && mem[*pos].is_ascii_digit() {
        value = value
            .wrapping_mul(10)
            .wrapping_add(u32::from(mem[*pos] - b'0'));
        found_digit = true;
        *pos += 1;
    }

    found_digit.then_some(value)
}

/// Parses a numeric literal or tokenized arithmetic expression after a `SYS` token.
fn parse_sys_expression(mem: &[u8], mut pos: usize, line_end: usize) -> Option<u16> {
    let mut value = parse_decimal(mem, &mut pos, line_end)?;

    while pos < line_end {
        while pos < line_end && (mem[pos] == b' ' || mem[pos] == b')') {
            pos += 1;
        }
        if pos >= line_end {
            break;
        }

        let op = mem[pos];
        if op != TOKEN_PLUS && op != TOKEN_MINUS && op != TOKEN_MULTIPLY && op != TOKEN_DIVIDE {
            break;
        }
        pos += 1;

        let Some(operand) = parse_decimal(mem, &mut pos, line_end) else {
            break;
        };

        match op {
            TOKEN_PLUS => value = value.wrapping_add(operand),
            TOKEN_MINUS => value = value.wrapping_sub(operand),
            TOKEN_MULTIPLY => value = value.wrapping_mul(operand),
            TOKEN_DIVIDE => value = value.checked_div(operand)?,
            _ => break,
        }
    }

    u16::try_from(value).ok()
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
    fn test_parse_sys_arithmetic_with_spaces_and_parens() {
        // SYS (2048) + (16) -> 2064
        let mem = vec![
            0x14, 0x08, 0x0A, 0x00, 0x9E, b' ', b'(', b'2', b'0', b'4', b'8', b')', b' ',
            TOKEN_PLUS, b' ', b'(', b'1', b'6', b')', 0x00,
        ];
        assert_eq!(find_sys_address(&mem, 0, None, None), Some(2064));
    }

    #[test]
    fn test_parse_sys_divide_by_zero_returns_none() {
        // SYS 2061/0
        let mem = vec![
            0x0D,
            0x08,
            0x0A,
            0x00,
            0x9E,
            b'2',
            b'0',
            b'6',
            b'1',
            TOKEN_DIVIDE,
            b'0',
            0x00,
        ];
        assert_eq!(find_sys_address(&mem, 0, None, None), None);
    }

    #[test]
    fn test_stops_at_end_of_basic_program_marker() {
        // Line 10 has PRINT (0x99), followed by 00 00 end-of-BASIC marker,
        // followed by machine code bytes that contain 0x9E ("SYS") and "2061".
        let mem = vec![
            0x07, 0x08, // Next line at $0807 (offset 6)
            0x0A, 0x00, // Line 10
            0x99, 0x00, // PRINT + EOL
            0x00, 0x00, // End of BASIC program at $0807
            0x00, 0x00, 0x9E, b'2', b'0', b'6', b'1', 0x00,
        ];
        assert_eq!(find_sys_address(&mem, 0, Some(0x0801), None), None);
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
