//! VICE monitor label file (`.lbl`) parser and generator.

use std::fmt::Write;

/// Parses VICE label commands (`al [C:][$]<hex_addr> .[label]`) into `(address, name)` pairs.
///
/// Lines that do not match the `al` command format or whose hexadecimal address exceeds
/// `u16::MAX` are ignored. Leading `.` characters on label names are stripped.
///
/// # Errors
///
/// This function is infallible and always returns `Ok`.
pub fn parse_vice_labels(content: &str) -> Result<Vec<(u16, String)>, String> {
    let mut labels = Vec::new();

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let Some("al") = parts.next() else {
            continue;
        };
        let (Some(addr_token), Some(name_token), None) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };

        let addr_str = addr_token.strip_prefix("C:").unwrap_or(addr_token);
        let addr_str = addr_str.strip_prefix('$').unwrap_or(addr_str);
        let name = name_token.strip_prefix('.').unwrap_or(name_token);

        if !name.is_empty()
            && let Ok(addr) = u16::from_str_radix(addr_str, 16)
        {
            labels.push((addr, name.to_string()));
        }
    }

    Ok(labels)
}

/// Formats `(address, name)` pairs into VICE monitor `al C:<addr> .<name>` lines.
#[must_use]
pub fn generate_vice_labels(labels: &[(u16, String)]) -> String {
    let mut content = String::new();
    for (addr, name) in labels {
        let clean_name = name.strip_prefix('.').unwrap_or(name);
        let _ = writeln!(content, "al C:{addr:04x} .{clean_name}");
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vice_labels() {
        let content = "\
al $1000 .start   \n\
al $1003 .loop\n\
al C:2000 .data_start\n\
al $3000 no_dot_label\n\
al $4000 .\n\
";
        let labels = parse_vice_labels(content).unwrap();
        assert_eq!(labels.len(), 4);
        assert_eq!(labels[0], (0x1000, "start".to_string()));
        assert_eq!(labels[1], (0x1003, "loop".to_string()));
        assert_eq!(labels[2], (0x2000, "data_start".to_string()));
        assert_eq!(labels[3], (0x3000, "no_dot_label".to_string()));
    }

    #[test]
    fn test_generate_vice_labels() {
        let labels = vec![(0x1000, "start".to_string()), (0x2000, ".loop".to_string())];
        let content = generate_vice_labels(&labels);
        assert!(content.contains("al C:1000 .start\n"));
        assert!(content.contains("al C:2000 .loop\n"));
    }
}
