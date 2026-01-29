use regex::Regex;

pub fn parse_vice_labels(content: &str) -> Result<Vec<(u16, String)>, String> {
    let mut labels = Vec::new();
    // Regex for "al $<hex_addr> .<label>"
    // Also handle possible variations if necessary, but standard is `al C:address .label` or just `al address .label`?
    // User example: `al $C000 .main_loop`
    // So looking for `al $<hex> <name>`
    // The dot might be part of the name in the file, we need to strip it.
    let re = Regex::new(r"(?m)^\s*al\s+(?:C:)?\$?([0-9a-fA-F]+)\s+(\.?)(.+)\s*$")
        .map_err(|e| e.to_string())?;

    for line in content.lines() {
        if let Some(caps) = re.captures(line) {
            let addr_str = caps.get(1).map_or("", |m| m.as_str());
            let _dot = caps.get(2).map_or("", |m| m.as_str());
            let name = caps.get(3).map_or("", |m| m.as_str());

            if let Ok(addr) = u16::from_str_radix(addr_str, 16) {
                // We keep the name as is, since we captured the dot separately (group 2) and name (group 3)
                // If the regex matched `(\.?)(.+)`, group 2 matches the dot if present, group 3 matches the rest.
                // So `name` is already stripped of the leading dot if it was there.
                labels.push((addr, name.to_string()));
            }
        }
    }

    Ok(labels)
}

pub fn generate_vice_labels(labels: &[(u16, String)]) -> String {
    let mut content = String::new();
    for (addr, name) in labels {
        // format: al C:address .label
        content.push_str(&format!("al C:{:04x} .{}\n", addr, name));
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vice_labels() {
        let content = r#"
al $1000 .start
al $1003 .loop
al C:2000 .data_start
al $3000 no_dot_label
"#;
        let labels = parse_vice_labels(content).unwrap();
        assert_eq!(labels.len(), 4);
        assert_eq!(labels[0], (0x1000, "start".to_string()));
        assert_eq!(labels[1], (0x1003, "loop".to_string()));
        assert_eq!(labels[2], (0x2000, "data_start".to_string()));
        assert_eq!(labels[3], (0x3000, "no_dot_label".to_string()));
    }

    #[test]
    fn test_generate_vice_labels() {
        let labels = vec![(0x1000, "start".to_string()), (0x2000, "loop".to_string())];
        let content = generate_vice_labels(&labels);
        assert!(content.contains("al C:1000 .start"));
        assert!(content.contains("al C:2000 .loop"));
    }
}
