use crate::state::AppState;
use crate::utils::{petscii_to_unicode, screencode_to_petscii};

// ---------------------------------------------------------------------------
// SearchFilters
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFilters {
    pub labels: bool,
    pub comments: bool,
    pub instructions: bool,
    pub hex_bytes: bool,
    pub text: bool,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            labels: true,
            comments: true,
            instructions: true,
            hex_bytes: true,
            text: true,
        }
    }
}

impl SearchFilters {
    #[must_use]
    pub fn as_array(&self) -> [bool; 5] {
        [
            self.labels,
            self.comments,
            self.instructions,
            self.hex_bytes,
            self.text,
        ]
    }

    pub fn toggle(&mut self, index: usize) {
        match index {
            0 => self.labels = !self.labels,
            1 => self.comments = !self.comments,
            2 => self.instructions = !self.instructions,
            3 => self.hex_bytes = !self.hex_bytes,
            4 => self.text = !self.text,
            _ => {}
        }
    }

    pub fn set_all(&mut self) {
        self.labels = true;
        self.comments = true;
        self.instructions = true;
        self.hex_bytes = true;
        self.text = true;
    }

    pub fn set_none(&mut self) {
        self.labels = false;
        self.comments = false;
        self.instructions = false;
        self.hex_bytes = false;
        self.text = false;
    }
}

// ---------------------------------------------------------------------------
// Hex pattern helpers
// ---------------------------------------------------------------------------

#[must_use]
pub fn parse_hex_pattern(query: &str) -> Option<Vec<Option<u8>>> {
    let mut pattern = Vec::new();

    // Safety check: ensure only contains hex chars, spaces, and '?'
    let allowed_chars = "0123456789abcdefABCDEF? ";
    if query.chars().any(|c| !allowed_chars.contains(c)) {
        return None;
    }

    // Remove spaces to handle both "A9 00" and "A900"
    let clean: String = query.chars().filter(|c| !c.is_whitespace()).collect();

    if clean.is_empty() {
        return None;
    }

    // Hex pattern must be pairs of characters (bytes)
    if !clean.len().is_multiple_of(2) {
        return None;
    }

    let chars: Vec<char> = clean.chars().collect();
    for chunk in chars.chunks(2) {
        let s: String = chunk.iter().collect();
        if s == "??" {
            pattern.push(None);
        } else {
            match u8::from_str_radix(&s, 16) {
                Ok(b) => pattern.push(Some(b)),
                Err(_) => return None,
            }
        }
    }
    Some(pattern)
}

#[must_use]
pub fn check_hex_pattern(
    address: crate::state::Addr,
    pattern: &[Option<u8>],
    app_state: &AppState,
) -> bool {
    let raw_len = app_state.raw_data.len();
    if raw_len == 0 {
        return false;
    }

    let start_offset = address.offset_from(app_state.origin);

    if start_offset >= raw_len {
        return false;
    }

    // Check if the pattern fits in the remaining data
    if start_offset + pattern.len() > raw_len {
        return false;
    }

    for (i, &byte_pat) in pattern.iter().enumerate() {
        let idx = start_offset + i;
        if let Some(target) = byte_pat
            && (idx >= raw_len || app_state.raw_data[idx] != target)
        {
            return false;
        }
    }

    true
}

// ---------------------------------------------------------------------------
// String pattern helpers (PETSCII / Screencode)
// ---------------------------------------------------------------------------

/// Check if the query string matches the raw bytes at the given address.
/// Checks both PETSCII and Screencode encodings, case-insensitively.
#[must_use]
pub fn check_string_pattern(
    address: crate::state::Addr,
    query: &str,
    app_state: &AppState,
    filters: &SearchFilters,
) -> bool {
    let raw_len = app_state.raw_data.len();
    if raw_len == 0 || query.is_empty() {
        return false;
    }

    let start_offset = address.offset_from(app_state.origin);
    if start_offset >= raw_len {
        return false;
    }

    let query_chars: Vec<char> = query.chars().collect();
    let query_len = query_chars.len();

    // Check if the pattern fits in the remaining data
    if start_offset + query_len > raw_len {
        return false;
    }

    if !filters.text {
        return false;
    }

    // Check PETSCII encoding (both shifted and unshifted)
    let petscii_match = (0..=1).any(|shift| {
        let shifted = shift == 1;
        query_chars.iter().enumerate().all(|(i, &query_char)| {
            let idx = start_offset + i;
            let byte = app_state.raw_data[idx];
            let petscii_char = petscii_to_unicode(byte, shifted);
            petscii_char.eq_ignore_ascii_case(&query_char)
        })
    });

    if petscii_match {
        return true;
    }

    // Check Screencode encoding (convert to PETSCII first, then to Unicode)
    let screencode_match = (0..=1).any(|shift| {
        let shifted = shift == 1;
        query_chars.iter().enumerate().all(|(i, &query_char)| {
            let idx = start_offset + i;
            let screencode_byte = app_state.raw_data[idx];
            let petscii_byte = screencode_to_petscii(screencode_byte);
            let sc_char = petscii_to_unicode(petscii_byte, shifted);
            sc_char.eq_ignore_ascii_case(&query_char)
        })
    });

    if screencode_match {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Instruction / disassembly-line matching
// ---------------------------------------------------------------------------

#[must_use]
pub fn match_instruction_content(
    line: &crate::disassembler::DisassemblyLine,
    query_lower: &str,
    filters: &SearchFilters,
) -> bool {
    if filters.hex_bytes {
        let bytes_hex = line
            .bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if bytes_hex
            .match_indices(query_lower)
            .any(|(idx, _)| idx % 2 == 0)
        {
            return true;
        }

        let bytes_hex_spaces = line
            .bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        if bytes_hex_spaces.contains(query_lower) {
            return true;
        }
    }

    if filters.instructions && line.mnemonic.to_lowercase().contains(query_lower) {
        return true;
    }

    if filters.instructions && line.operand.to_lowercase().contains(query_lower) {
        return true;
    }

    if filters.comments && line.comment.to_lowercase().contains(query_lower) {
        return true;
    }

    if filters.labels
        && let Some(lbl) = &line.label
        && lbl.to_lowercase().contains(query_lower)
    {
        return true;
    }

    false
}

#[must_use]
pub fn get_line_matches(
    line: &crate::disassembler::DisassemblyLine,
    app_state: &AppState,
    query_lower: &str,
    hex_pattern: Option<&[Option<u8>]>,
    filters: &SearchFilters,
) -> Vec<usize> {
    let mut matches = Vec::new();
    let mut current_sub = 0;

    // 1. Relative Labels
    if line.bytes.len() > 1 {
        for offset in 1..line.bytes.len() {
            let mid_addr = line.address.wrapping_add(offset as u16);
            if let Some(labels) = app_state.labels.get(&mid_addr) {
                for _ in labels {
                    if filters.labels
                        && labels
                            .iter()
                            .any(|l| l.name.to_lowercase().contains(query_lower))
                    {
                        matches.push(current_sub);
                    }
                    current_sub += 1;
                }
            }
        }
    }

    // 2. Line Comment
    if let Some(lc) = &line.line_comment {
        for comment_line in lc.lines() {
            if filters.comments && comment_line.to_lowercase().contains(query_lower) {
                matches.push(current_sub);
            }
            current_sub += 1;
        }
    }

    // 3. Instruction Content
    let mut instruction_match = match_instruction_content(line, query_lower, filters);

    // 4. Hex pattern search and String pattern search
    if !instruction_match {
        for offset in 0..line.bytes.len() {
            let addr = line.address.wrapping_add(offset as u16);

            // Hex pattern search
            if let Some(pattern) = hex_pattern
                && check_hex_pattern(addr, pattern, app_state)
            {
                instruction_match = true;
                break;
            }

            // String pattern search (PETSCII / Screencode)
            if check_string_pattern(addr, query_lower, app_state, filters) {
                instruction_match = true;
                break;
            }
        }
    }

    if instruction_match {
        matches.push(current_sub);
    }

    matches
}

#[must_use]
pub fn search_collapsed_content(
    app_state: &AppState,
    start: usize,
    end: usize,
    query_lower: &str,
    hex_pattern: Option<&[Option<u8>]>,
    filters: &SearchFilters,
) -> bool {
    if start >= app_state.raw_data.len() || end >= app_state.raw_data.len() {
        return false;
    }

    let origin = app_state.origin.wrapping_add(start as u16);
    let data_slice = &app_state.raw_data[start..=end];

    if start >= app_state.block_types.len() || end >= app_state.block_types.len() {
        return false;
    }
    let block_slice = &app_state.block_types[start..=end];

    let ctx = crate::disassembler::context::DisassemblyContext {
        data: data_slice,
        block_types: block_slice,
        labels: &app_state.labels,
        origin,
        settings: &app_state.settings,
        platform_comments: &app_state.platform_comments,
        user_side_comments: &app_state.user_side_comments,
        user_line_comments: &app_state.user_line_comments,
        immediate_value_formats: &app_state.immediate_value_formats,
        cross_refs: &app_state.cross_refs,
        collapsed_blocks: &[],
        splitters: &app_state.splitters,
        scopes: &app_state.scopes,
    };
    let expanded_lines = app_state.disassembler.disassemble_ctx(&ctx);

    for line in expanded_lines {
        if !get_line_matches(&line, app_state, query_lower, hex_pattern, filters).is_empty() {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Raw-memory byte scan (used by MCP r2000_search_memory)
// ---------------------------------------------------------------------------

/// Search raw binary data for a byte pattern, supporting multiple encodings.
///
/// Returns a list of matching addresses (capped at `max_results`).
///
/// # Errors
/// Returns an error if an unknown encoding is provided.
pub fn search_memory_raw(
    app_state: &AppState,
    query: &str,
    encoding: Option<&str>,
    max_results: usize,
) -> Result<Vec<u16>, String> {
    // Determine mode
    let mode = if let Some(enc) = encoding {
        enc
    } else {
        // Simple heuristic: if query contains space and hex-like chars, try hex
        if query.contains(' ')
            && query
                .split_whitespace()
                .all(|s| u8::from_str_radix(s, 16).is_ok())
        {
            "hex"
        } else {
            "text"
        }
    };

    // Build one or more byte-patterns to scan for.
    let patterns: Vec<Vec<u8>> = match mode {
        "hex" => {
            let mut bytes = Vec::new();
            for part in query.split_whitespace() {
                let clean_part = part
                    .trim_start_matches("0x")
                    .trim_start_matches("0X")
                    .trim_start_matches('$');
                if let Ok(b) = u8::from_str_radix(clean_part, 16) {
                    bytes.push(b);
                }
            }
            vec![bytes]
        }
        "text" => {
            // Search both PETSCII and Screencode encodings.
            let petscii_bytes: Vec<u8> = query.chars().map(ascii_char_to_petscii).collect();
            let screencode_bytes: Vec<u8> = petscii_bytes
                .iter()
                .map(|&p| petscii_to_screencode_simple(p))
                .collect();
            if petscii_bytes == screencode_bytes {
                vec![petscii_bytes]
            } else {
                vec![petscii_bytes, screencode_bytes]
            }
        }
        _ => {
            return Err(format!("Unknown encoding: {mode}"));
        }
    };

    if patterns.iter().all(std::vec::Vec::is_empty) {
        return Ok(Vec::new());
    }

    let data = &app_state.raw_data;
    let origin = app_state.origin;

    // Collect matching addresses from all patterns, deduplicated and sorted.
    let mut found_set = std::collections::BTreeSet::new();

    for pattern in &patterns {
        if pattern.is_empty() || data.len() < pattern.len() {
            continue;
        }
        for i in 0..=data.len() - pattern.len() {
            if data[i..i + pattern.len()] == pattern[..] {
                found_set.insert(origin.wrapping_add(i as u16));
                if found_set.len() >= max_results {
                    break;
                }
            }
        }
        if found_set.len() >= max_results {
            break;
        }
    }

    Ok(found_set.into_iter().map(|a| a.0).collect())
}

// ---------------------------------------------------------------------------
// Encoding helpers (moved from mcp/handler.rs)
// ---------------------------------------------------------------------------

fn ascii_char_to_petscii(c: char) -> u8 {
    let b = c as u8;
    match b {
        b'a'..=b'z' => b - 32, // 'a' (97) -> 'A' (65) (Unshifted PETSCII)
        b'A'..=b'Z' => b + 32, // 'A' (65) -> 'a' (97) (Shifted PETSCII / Graphics)
        _ => b,                // Numbers, punctuation mostly map 1:1 for basic ASCII
    }
}

fn petscii_to_screencode_simple(petscii: u8) -> u8 {
    match petscii {
        0x40..=0x5F => petscii - 0x40,
        0x20..=0x3F => petscii,
        0x60..=0x7F => petscii - 0x20,
        0xA0..=0xBF => petscii - 0x40,
        _ => petscii, // Fallback
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::DisassemblyLine;

    #[test]
    fn test_match_instruction_content_bytes_alignment() {
        let line = DisassemblyLine {
            address: crate::state::Addr(0x1000),
            bytes: vec![0x8D, 0x02, 0x08], // 8d0208
            mnemonic: "STA".to_string(),
            operand: "$0802".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };

        // "d020" is in "8d0208" starting at index 1 -> Should FAIL
        assert!(!match_instruction_content(
            &line,
            "d020",
            &SearchFilters::default()
        ));

        // "8d02" is in "8d0208" starting at index 0 -> Should PASS
        assert!(match_instruction_content(
            &line,
            "8d02",
            &SearchFilters::default()
        ));
    }

    #[test]
    fn test_get_line_matches_sub_indices() {
        use crate::state::Label;
        let mut app_state = AppState::new();
        // Mid-address label at 0x1001
        app_state.labels.insert(
            crate::state::Addr(0x1001),
            vec![Label {
                name: "mid_label".to_string(),
                label_type: crate::state::LabelType::AbsoluteAddress,
                kind: crate::state::LabelKind::User,
            }],
        );

        let line = DisassemblyLine {
            address: crate::state::Addr(0x1000),
            bytes: vec![0xA9, 0x00], // LDA #$00
            mnemonic: "LDA".to_string(),
            operand: "#$00".to_string(),
            comment: "side comment".to_string(),
            line_comment: Some("line 1\nline 2\nline 3".to_string()),
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };

        // Sub-indices mapping:
        // 0: label at 0x1001
        // 1: line comment "line 1"
        // 2: line comment "line 2"
        // 3: line comment "line 3"
        // 4: Instruction (LDA #$00 ; side comment)

        // Test label match
        let filters = SearchFilters::default();
        let matches = get_line_matches(&line, &app_state, "mid_label", None, &filters);
        assert_eq!(matches, vec![0]);

        // Test line comment matches
        assert_eq!(
            get_line_matches(&line, &app_state, "line 1", None, &filters),
            vec![1]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "line 2", None, &filters),
            vec![2]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "line 3", None, &filters),
            vec![3]
        );

        // Test instruction matches
        assert_eq!(
            get_line_matches(&line, &app_state, "lda", None, &filters),
            vec![4]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "side", None, &filters),
            vec![4]
        );

        // Test multiple matches (e.g. "line" matches all comment lines)
        assert_eq!(
            get_line_matches(&line, &app_state, "line", None, &filters),
            vec![1, 2, 3]
        );
    }
}

#[cfg(test)]
mod tests_hex {
    use super::*;

    #[test]
    fn test_parse_hex_pattern() {
        // Valid patterns
        assert_eq!(
            parse_hex_pattern("A9 00"),
            Some(vec![Some(0xA9), Some(0x00)])
        );
        assert_eq!(
            parse_hex_pattern("A9 00 ?? D0"),
            Some(vec![Some(0xA9), Some(0x00), None, Some(0xD0)])
        );
        assert_eq!(
            parse_hex_pattern("a900??d0"),
            Some(vec![Some(0xA9), Some(0x00), None, Some(0xD0)])
        );
        assert_eq!(parse_hex_pattern("??"), Some(vec![None]));

        // Invalid patterns
        assert_eq!(parse_hex_pattern("A"), None); // odd length
        assert_eq!(parse_hex_pattern("A9 0"), None); // odd length
        assert_eq!(parse_hex_pattern("G0"), None); // invalid char
        assert_eq!(parse_hex_pattern("LDA"), None); // invalid chars
        assert_eq!(parse_hex_pattern(""), None); // empty
        assert_eq!(parse_hex_pattern("A?"), None); // invalid wildcard (must be ??)
    }
}

#[cfg(test)]
mod tests_string {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn test_check_string_pattern() {
        let mut app_state = AppState::new();
        app_state.raw_data = vec![
            0x48, 0x45, 0x4C, 0x4C, 0x4F, // "HELLO" in PETSCII (unshifted)
            0x08, 0x05, 0x0C, 0x0C, 0x0F, // "hello" in Screencodes (Shifted/Lowercase)
        ];
        app_state.origin = crate::state::Addr(0x1000);

        // 1. PETSCII match (unshifted)
        let filters = SearchFilters::default();
        assert!(check_string_pattern(
            crate::state::Addr(0x1000),
            "HELLO",
            &app_state,
            &filters
        ));
        assert!(check_string_pattern(
            crate::state::Addr(0x1000),
            "hello",
            &app_state,
            &filters
        )); // Case-insensitive
        assert!(check_string_pattern(
            crate::state::Addr(0x1000),
            "HellO",
            &app_state,
            &filters
        ));

        // 2. Screencode match
        // Note: screencode_to_petscii(0x08) -> 0x48 ('H' or 'h' shifted)
        assert!(check_string_pattern(
            crate::state::Addr(0x1005),
            "hello",
            &app_state,
            &filters
        ));
        assert!(check_string_pattern(
            crate::state::Addr(0x1005),
            "HELLO",
            &app_state,
            &filters
        ));

        // 3. No match
        assert!(!check_string_pattern(
            crate::state::Addr(0x1000),
            "WORLD",
            &app_state,
            &filters
        ));
        assert!(!check_string_pattern(
            crate::state::Addr(0x1008),
            "HELLO",
            &app_state,
            &filters
        )); // Out of bounds/Too long
    }

    #[test]
    fn test_string_search_at_offset() {
        use crate::disassembler::DisassemblyLine;

        let mut app_state = AppState::new();
        // Screencode for "LANDING": 0C 01 0E 04 09 0E 07
        // Let's put a dummy byte at the start to force an offset
        app_state.raw_data = vec![0x00, 0x0C, 0x01, 0x0E, 0x04, 0x09, 0x0E, 0x07];
        app_state.origin = crate::state::Addr(0x1000);

        let line = DisassemblyLine {
            address: crate::state::Addr(0x1000),
            bytes: app_state.raw_data.clone(),
            mnemonic: String::new(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };

        // This is what get_line_matches CURRENTLY does (simplified)
        let query = "landing";
        let filters = SearchFilters::default();
        let found_at_start = check_string_pattern(line.address, query, &app_state, &filters);
        assert!(
            !found_at_start,
            "Should not find at start due to leading 0x00"
        );

        // The bug is that we don't check offsets. This test will help me verify the fix.
        let mut found_at_any_offset = false;
        for offset in 0..line.bytes.len() {
            if check_string_pattern(
                line.address.wrapping_add(offset as u16),
                query,
                &app_state,
                &filters,
            ) {
                found_at_any_offset = true;
                break;
            }
        }
        assert!(found_at_any_offset, "Should find 'landing' at offset 1");
    }
}
