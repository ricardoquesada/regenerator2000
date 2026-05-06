use crate::state::AppState;
use crate::utils::{petscii_to_unicode, screencode_to_petscii};
use regex::Regex;

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
    /// When `true`, the search query is interpreted as a regular expression.
    /// Hex-byte and PETSCII/Screencode byte scanning are disabled in regex mode.
    pub use_regex: bool,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            labels: true,
            comments: true,
            instructions: true,
            hex_bytes: true,
            text: true,
            use_regex: false,
        }
    }
}

impl SearchFilters {
    #[must_use]
    pub fn as_array(&self) -> [bool; 6] {
        [
            self.labels,
            self.comments,
            self.instructions,
            self.hex_bytes,
            self.text,
            self.use_regex,
        ]
    }

    pub fn toggle(&mut self, index: usize) {
        match index {
            0 => self.labels = !self.labels,
            1 => self.comments = !self.comments,
            2 => self.instructions = !self.instructions,
            3 => self.hex_bytes = !self.hex_bytes,
            4 => self.text = !self.text,
            5 => self.use_regex = !self.use_regex,
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
// SearchResult
// ---------------------------------------------------------------------------

/// A single disassembly line that matched a search query.
///
/// Returned by [`search_disassembly`] so callers can display context
/// without issuing a follow-up read request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    /// Address of the matching disassembly line.
    pub address: crate::state::Addr,
    /// Label at this address, if any.
    pub label: Option<String>,
    /// Mnemonic text (e.g. `"LDA"`).
    pub mnemonic: String,
    /// Operand text (e.g. `"$D020"`).
    pub operand: String,
    /// Side comment text, if any.
    pub comment: String,
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

/// Compile a case-insensitive regular expression from `query`.
///
/// Wraps the pattern with `(?i)` so matching is case-insensitive, consistent
/// with plain-text search mode.  Users who need case-sensitive matching can
/// override this by prefixing their pattern with `(?-i)`.
///
/// # Errors
/// Returns a [`regex::Error`] if `query` is not a valid regular expression.
pub fn compile_regex(query: &str) -> Result<Regex, regex::Error> {
    let pattern = format!("(?i){query}");
    Regex::new(&pattern)
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

/// Test whether `haystack` contains a match for the query.
///
/// In plain mode (`regex` is `None`) this performs a case-insensitive
/// substring search using the pre-lowercased `query_lower`.  In regex mode
/// (`regex` is `Some`) the compiled regex (already `(?i)`-wrapped) is used
/// directly on the original string.
#[must_use]
fn text_matches(haystack: &str, query_lower: &str, regex: Option<&Regex>) -> bool {
    match regex {
        Some(re) => re.is_match(haystack),
        None => haystack.to_lowercase().contains(query_lower),
    }
}

/// Returns `true` when the disassembly line's textual content matches the query.
///
/// Hex-byte matching uses alignment-aware byte-level scanning and is disabled
/// in regex mode (the query is a pattern, not a hex string).  Mnemonic,
/// operand, comment, and label fields are matched via [`text_matches`].
#[must_use]
pub fn match_instruction_content(
    line: &crate::disassembler::DisassemblyLine,
    query_lower: &str,
    regex: Option<&Regex>,
    filters: &SearchFilters,
) -> bool {
    // Hex-byte scan is plain-text only; skip when using regex.
    if filters.hex_bytes && regex.is_none() {
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

    if filters.instructions && text_matches(&line.mnemonic, query_lower, regex) {
        return true;
    }

    if filters.instructions && text_matches(&line.operand, query_lower, regex) {
        return true;
    }

    if filters.comments && text_matches(&line.comment, query_lower, regex) {
        return true;
    }

    if filters.labels
        && let Some(lbl) = &line.label
        && text_matches(lbl, query_lower, regex)
    {
        return true;
    }

    false
}

/// Returns the list of sub-indices within `line` that match the query.
///
/// Sub-indices map to: mid-address labels (relative offsets), line-comment
/// lines, and then the instruction itself (mnemonic / operand / comment /
/// label).  Used by the TUI to highlight individual matches within a row.
///
/// * `query_lower` – the query string lowercased; used for plain-text search.
/// * `hex_pattern` – pre-parsed hex byte pattern; `None` in regex mode.
/// * `regex`       – compiled regex; `None` in plain-text mode.
#[must_use]
pub fn get_line_matches(
    line: &crate::disassembler::DisassemblyLine,
    app_state: &AppState,
    query_lower: &str,
    hex_pattern: Option<&[Option<u8>]>,
    regex: Option<&Regex>,
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
                            .any(|l| text_matches(&l.name, query_lower, regex))
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
            if filters.comments && text_matches(comment_line, query_lower, regex) {
                matches.push(current_sub);
            }
            current_sub += 1;
        }
    }

    // 3. Instruction Content
    let mut instruction_match = match_instruction_content(line, query_lower, regex, filters);

    // 4. Hex pattern and string-encoding search (plain-text mode only)
    if !instruction_match && regex.is_none() {
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

/// Search the expanded content of a collapsed block for a match.
///
/// Returns `true` as soon as any line in the block matches, without
/// materialising the full result set.
#[must_use]
pub fn search_collapsed_content(
    app_state: &AppState,
    start: usize,
    end: usize,
    query_lower: &str,
    hex_pattern: Option<&[Option<u8>]>,
    regex: Option<&Regex>,
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
        system_comments: &app_state.system_comments,
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
        if !get_line_matches(&line, app_state, query_lower, hex_pattern, regex, filters).is_empty()
        {
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
// search_disassembly
// ---------------------------------------------------------------------------

/// Search all disassembly lines for `query`, returning structured results.
///
/// `filters.use_regex` controls whether the query is compiled as a
/// case-insensitive regular expression (via [`compile_regex`]) or treated as
/// a plain case-insensitive substring.  `filters.hex_bytes` and `filters.text`
/// are ignored here — raw-byte scanning is not meaningful for disassembly text.
///
/// Results are capped at `max_results`.
///
/// # Errors
/// Returns a `String` error if `filters.use_regex` is `true` and `query` is
/// not a valid regular expression.
#[must_use = "search results should be inspected or returned to the caller"]
pub fn search_disassembly(
    app_state: &AppState,
    query: &str,
    filters: &SearchFilters,
    max_results: usize,
) -> Result<Vec<SearchResult>, String> {
    let regex = if filters.use_regex {
        match compile_regex(query) {
            Ok(re) => Some(re),
            Err(e) => return Err(format!("Invalid regex: {e}")),
        }
    } else {
        None
    };

    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for line in &app_state.disassembly {
        if results.len() >= max_results {
            break;
        }
        let matches =
            get_line_matches(line, app_state, &query_lower, None, regex.as_ref(), filters);
        if !matches.is_empty() {
            results.push(SearchResult {
                address: line.address,
                label: line.label.clone(),
                mnemonic: line.mnemonic.clone(),
                operand: line.operand.clone(),
                comment: line.comment.clone(),
            });
        }
    }

    Ok(results)
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
            None,
            &SearchFilters::default()
        ));

        // "8d02" is in "8d0208" starting at index 0 -> Should PASS
        assert!(match_instruction_content(
            &line,
            "8d02",
            None,
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
        let matches = get_line_matches(&line, &app_state, "mid_label", None, None, &filters);
        assert_eq!(matches, vec![0]);

        // Test line comment matches
        assert_eq!(
            get_line_matches(&line, &app_state, "line 1", None, None, &filters),
            vec![1]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "line 2", None, None, &filters),
            vec![2]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "line 3", None, None, &filters),
            vec![3]
        );

        // Test instruction matches
        assert_eq!(
            get_line_matches(&line, &app_state, "lda", None, None, &filters),
            vec![4]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "side", None, None, &filters),
            vec![4]
        );

        // Test multiple matches (e.g. "line" matches all comment lines)
        assert_eq!(
            get_line_matches(&line, &app_state, "line", None, None, &filters),
            vec![1, 2, 3]
        );
    }
}

#[cfg(test)]
mod tests_regex {
    use super::*;
    use crate::disassembler::DisassemblyLine;

    fn make_line(
        mnemonic: &str,
        operand: &str,
        comment: &str,
        label: Option<&str>,
    ) -> DisassemblyLine {
        DisassemblyLine {
            address: crate::state::Addr(0x1000),
            bytes: vec![0xEA],
            mnemonic: mnemonic.to_string(),
            operand: operand.to_string(),
            comment: comment.to_string(),
            line_comment: None,
            label: label.map(str::to_string),
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        }
    }

    #[test]
    fn test_compile_regex_valid() {
        let re = compile_regex("lda.*sta").unwrap();
        assert!(re.is_match("LDA #$00 ; STA"));
    }

    #[test]
    fn test_compile_regex_invalid() {
        assert!(compile_regex("[invalid").is_err());
    }

    #[test]
    fn test_compile_regex_case_insensitive() {
        let re = compile_regex("LDA").unwrap();
        assert!(re.is_match("lda"));
        assert!(re.is_match("LDA"));
        assert!(re.is_match("Lda"));
    }

    #[test]
    fn test_regex_matches_mnemonic() {
        let line = make_line("LDA", "#$00", "", None);
        let re = compile_regex("ld.").unwrap();
        let filters = SearchFilters::default();
        assert!(match_instruction_content(&line, "", Some(&re), &filters));
    }

    #[test]
    fn test_regex_matches_comment() {
        let line = make_line("NOP", "", "TODO: fix this", None);
        let re = compile_regex("(TODO|FIXME)").unwrap();
        let filters = SearchFilters::default();
        assert!(match_instruction_content(&line, "", Some(&re), &filters));
    }

    #[test]
    fn test_regex_matches_label() {
        let line = make_line("RTS", "", "", Some("s_init_sprites"));
        let re = compile_regex("s_.*init").unwrap();
        let filters = SearchFilters::default();
        assert!(match_instruction_content(&line, "", Some(&re), &filters));
    }

    #[test]
    fn test_regex_does_not_match_hex_bytes() {
        // bytes = [0xA9] -> hex string "a9"; pattern matches "a" followed by any char
        // But hex scanning is disabled in regex mode, so this should NOT match
        // via hex bytes — only via mnemonic/operand/comment/label.
        let line = make_line("LDA", "#$09", "", None); // mnemonic won't match "a."
        let re = compile_regex("^a.$").unwrap(); // matches "a9" if hex scan were active
        let mut filters = SearchFilters::default();
        filters.instructions = false;
        filters.comments = false;
        filters.labels = false;
        assert!(!match_instruction_content(&line, "", Some(&re), &filters));
    }

    #[test]
    fn test_regex_no_match() {
        let line = make_line("STA", "$D020", "", None);
        let re = compile_regex("^lda$").unwrap();
        let filters = SearchFilters::default();
        assert!(!match_instruction_content(&line, "", Some(&re), &filters));
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
