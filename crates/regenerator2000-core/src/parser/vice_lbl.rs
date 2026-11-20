//! VICE monitor label file (`.lbl`) parser and generator.

use std::fmt::Write;

/// A label entry parsed from or formatted for a VICE monitor `.lbl` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViceLabel {
    /// 16-bit memory address of the label.
    pub address: u16,
    /// Label symbol name without any leading `.` prefix.
    pub name: String,
}

/// Parses VICE label commands (`al [C:][$]<hex_addr> .[label]`) into [`ViceLabel`] entries.
///
/// Lines that do not match the `al` command format or whose hexadecimal address exceeds
/// `u16::MAX` are ignored. Leading `.` characters on label names are stripped.
///
/// # Errors
///
/// This function is infallible and always returns `Ok`.
pub fn parse_vice_labels(content: &str) -> Result<Vec<ViceLabel>, String> {
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
            && let Ok(address) = u16::from_str_radix(addr_str, 16)
        {
            labels.push(ViceLabel {
                address,
                name: name.to_string(),
            });
        }
    }

    Ok(labels)
}

/// Formats [`ViceLabel`] entries into VICE monitor `al C:<addr> .<name>` lines.
#[must_use]
pub fn generate_vice_labels(labels: &[ViceLabel]) -> String {
    let mut content = String::new();
    for label in labels {
        let clean_name = label.name.strip_prefix('.').unwrap_or(&label.name);
        let _ = writeln!(content, "al C:{:04x} .{clean_name}", label.address);
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
        let labels = parse_vice_labels(content).expect("valid VICE label file should parse");
        assert_eq!(labels.len(), 4);
        assert_eq!(
            labels[0],
            ViceLabel {
                address: 0x1000,
                name: "start".to_string()
            }
        );
        assert_eq!(
            labels[1],
            ViceLabel {
                address: 0x1003,
                name: "loop".to_string()
            }
        );
        assert_eq!(
            labels[2],
            ViceLabel {
                address: 0x2000,
                name: "data_start".to_string()
            }
        );
        assert_eq!(
            labels[3],
            ViceLabel {
                address: 0x3000,
                name: "no_dot_label".to_string()
            }
        );
    }

    #[test]
    fn test_generate_vice_labels() {
        let labels = vec![
            ViceLabel {
                address: 0x1000,
                name: "start".to_string(),
            },
            ViceLabel {
                address: 0x2000,
                name: ".loop".to_string(),
            },
        ];
        let content = generate_vice_labels(&labels);
        assert!(content.contains("al C:1000 .start\n"));
        assert!(content.contains("al C:2000 .loop\n"));
    }
}
