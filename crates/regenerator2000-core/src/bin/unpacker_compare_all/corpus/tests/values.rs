//! Tests for `values`: FNV-1a-64 (T1.A1.1–T1.A1.4) and `HexAddr`
//! (T1.A1.5–T1.A1.7), plus the address-range helpers they rely on.

use super::*;
use serde::Serialize;

/// T1.A1.1
#[test]
fn fnv1a64_empty_input_is_offset_basis() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
}

/// T1.A1.2
#[test]
fn fnv1a64_known_vectors() {
    let vectors: [(&[u8], u64); 3] = [
        (b"a", 0xaf63_dc4c_8601_ec8c),
        (b"foobar", 0x8594_4171_f739_67e8),
        (&[0x00], 0xaf63_bd4c_8601_b7df),
    ];
    let failures: Vec<String> = vectors
        .iter()
        .filter(|(input, expected)| fnv1a64(input) != *expected)
        .map(|(input, expected)| {
            format!(
                "{input:?}: got {:016x}, expected {expected:016x}",
                fnv1a64(input)
            )
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.3
#[test]
fn fnv1a64_is_order_and_length_sensitive() {
    assert_ne!(fnv1a64(b"ab"), fnv1a64(b"ba"));
    assert_ne!(fnv1a64(&[0]), fnv1a64(&[0, 0]));
}

/// T1.A1.4
#[test]
fn fnv64_hex_string_roundtrip() {
    for value in [0u64, 1, 0xcbf2_9ce4_8422_2325, u64::MAX] {
        let text = Fnv64(value).to_string();
        assert_eq!(text.len(), 16, "{text}");
        assert!(
            text.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_eq!(Fnv64::parse(&text), Ok(Fnv64(value)));
    }
    assert_eq!(Fnv64(1).to_string(), "0000000000000001");

    for bad in [
        "CBF29CE484222325",
        "cbf29ce48422232",
        "0xcbf29ce484222325",
        "cbf29ce4842223255",
        "cbf29ce48422232g",
        "",
    ] {
        assert_eq!(
            Fnv64::parse(bad),
            Err(ValueFormatError::Fnv64 {
                input: bad.to_string()
            }),
            "{bad:?} must be rejected"
        );
    }
}

/// T1.A1.5
#[test]
fn hex_addr_serializes_as_dollar_four_upper_hex() {
    #[derive(Serialize)]
    struct Wrapper {
        addr: HexAddr,
    }
    let rows = [
        (0x0000, "$0000"),
        (0x0801, "$0801"),
        (0xFFFF, "$FFFF"),
        (0xabcd, "$ABCD"),
    ];
    let mut failures = Vec::new();
    for (value, expected) in rows {
        let display = HexAddr(value).to_string();
        let toml_text = toml::to_string(&Wrapper {
            addr: HexAddr(value),
        })
        .unwrap();
        if display != expected || toml_text.trim() != format!("addr = \"{expected}\"") {
            failures.push(format!("{value:#06x}: display {display}, toml {toml_text}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.6
#[test]
fn hex_addr_rejects_malformed() {
    let bad_strings = ["0800", "$800", "$10000", "$GG00", "", "$"];
    let mut failures = Vec::new();
    for bad in bad_strings {
        if HexAddr::parse(bad).is_ok() {
            failures.push(format!("HexAddr::parse({bad:?}) accepted"));
        }
        let manifest =
            manifest_with_unp64("").replace("start = \"$0801\"", &format!("start = \"{bad}\""));
        let message = parse_error(&manifest);
        if !message.contains("start") {
            failures.push(format!("{bad:?}: error does not name the field: {message}"));
        }
    }
    // An integer is not an address string.
    let manifest = manifest_with_unp64("").replace("start = \"$0801\"", "start = 2049");
    let message = parse_error(&manifest);
    if !message.contains("start") || !message.contains("address") {
        failures.push(format!(
            "integer: error must name the field and form: {message}"
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// T1.A1.7
#[test]
fn hex_addr_parse_accepts_lowercase_and_normalizes() {
    let addr = HexAddr::parse("$e7ff").unwrap();
    assert_eq!(addr, HexAddr(0xE7FF));
    assert_eq!(addr.to_string(), "$E7FF");

    let manifest = manifest_with_unp64("").replace("$080D", "$080d");
    let written = Corpus::parse(&manifest).unwrap().to_toml_string().unwrap();
    assert!(written.contains("entry = \"$080D\""), "{written}");
}

/// `AddrRange` construction, length (including the full 64 KB and wrap
/// edges), containment and intersection.
#[test]
fn addr_range_arithmetic() {
    assert_eq!(AddrRange::new(0x0810, 0x0801), None);
    assert_eq!(range(0x0800, 0xFFFF).len(), 63_488);
    assert_eq!(range(0x0000, 0xFFFF).len(), 65_536);
    assert_eq!(range(0x1234, 0x1234).len(), 1);

    assert_eq!(
        AddrRange::from_start_len(0xFF00, 256),
        Some(range(0xFF00, 0xFFFF))
    );
    assert_eq!(AddrRange::from_start_len(0xFF00, 257), None);
    assert_eq!(AddrRange::from_start_len(0x0801, 0), None);

    let outer = range(0x0800, 0x2000);
    assert!(outer.contains_range(range(0x0800, 0x2000)));
    assert!(outer.contains_range(range(0x1000, 0x1001)));
    assert!(!outer.contains_range(range(0x07FF, 0x1000)));
    assert!(!outer.contains_range(range(0x1000, 0x2001)));

    assert_eq!(
        outer.intersect(range(0x1000, 0x3000)),
        Some(range(0x1000, 0x2000))
    );
    assert_eq!(outer.intersect(range(0x2001, 0x3000)), None);
    assert_eq!(outer.to_string(), "$0800-$2000");
}

/// `hash_window` hashes exactly the window's bytes and refuses a window the
/// payload does not fully cover (it never reads outside the payload).
#[test]
fn hash_window_hashes_only_covered_windows() {
    let payload = [1u8, 2, 3];
    assert_eq!(
        hash_window(0x0800, &payload, range(0x0801, 0x0802)),
        Some(Fnv64::of(&[2, 3]))
    );
    assert_eq!(
        hash_window(0x0800, &payload, range(0x0800, 0x0802)),
        Some(Fnv64::of(&payload))
    );
    assert_eq!(hash_window(0x0800, &payload, range(0x0802, 0x0803)), None);
    assert_eq!(hash_window(0x0800, &payload, range(0x07FF, 0x0800)), None);
    assert_eq!(hash_window(0x0800, &[], range(0x0800, 0x0800)), None);
}
