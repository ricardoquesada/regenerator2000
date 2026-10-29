//! Tests for the unp64 parser and invocation helpers (T1.A2.1–T1.A2.19,
//! T1.A2.42–T1.A2.44, T1.A2.46). Console captures are real unp64 v2.42
//! output shapes (lower-case hex, `\r` progress, `-v` diagnostics).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use super::*;
use crate::unp64::parse::{
    PackerLine, ParseError, SavedLine, cross_check, derive_saved_payload, interpret_run,
    parse_console, parse_version_banner,
};
use crate::unp64::{
    ProbeOutput, Unp64Error, binary_candidates, interpret_version_probe, tmp_output_path,
    unp64_args,
};

/// unp64 v2.42 on `c64_traveller.tiny_crunch.prg`.
const TRAVELLER: &str = "\nTinyCrunch v1.0/ChristopherJam, unpacker=$010e\n\
                         Entry point: $080d\n\
                         pass1, find unpacker: $010e\n\
                         pass2, return to mem: $0911\n\
                         saved $0801-$fffd as /tmp/out.prg\n";

/// A run that gave up: no `saved` line, although unp64 exits with 0 here.
const MAX_ITERATIONS: &str = "\nExomizer, unpacker=$0134\n\
                              Entry point: $080d\n\
                              pass1, find unpacker: $0134\n\
                              pass2, return to mem: \n\
                              Max Iterations 0x01000000 Reached, quitting...\n";

/// The no-argument usage banner (ruling R2).
const BANNER: &str = "\nUNP64 v2.42 - Generic C64 prg unpacker - (C)2008-2026 iAN CooG [May 11 2026]\n\
                      Based on Exomizer 2.0b7 sources by Magnus Lind.\n\
                      usage: UNP64 <packed.prg>{[,|@]Addr} [-parameters]\n";

/// The entry of a console capture with the given `pass2` tail.
fn pass2_entry(tail: &str) -> Option<Unp64Entry> {
    parse_console(&format!("Entry point: $080d\npass2, return to mem: {tail}")).entry
}

/// A PRG: load address followed by `len` payload bytes.
fn prg(load_addr: u16, len: usize) -> Vec<u8> {
    let mut bytes = load_addr.to_le_bytes().to_vec();
    bytes.extend(std::iter::repeat_n(0xEA, len));
    bytes
}

/// T1.A2.1
#[test]
fn parse_saved_line_basic() {
    assert_eq!(
        parse_console(TRAVELLER).saved,
        Some(SavedLine {
            start: 0x0801,
            end: 0xFFFD
        })
    );
}

/// T1.A2.2
#[test]
fn parse_saved_line_lowercase_hex() {
    let console = parse_console("saved $c0de-$e7ff as /tmp/x.prg\n");
    assert_eq!(
        console.saved,
        Some(SavedLine {
            start: 0xC0DE,
            end: 0xE7FF
        })
    );
}

/// T1.A2.3: `$ffff` is printed as such (`(EndAdr - 1) & $FFFF` with
/// `EndAdr = $10000`).
#[test]
fn parse_saved_line_end_ffff() {
    assert_eq!(
        parse_console("saved $0800-$ffff as out\n").saved,
        Some(SavedLine {
            start: 0x0800,
            end: 0xFFFF
        })
    );
}

/// T1.A2.4: progress output uses bare `\r`, which must split lines too.
#[test]
fn parse_splits_on_carriage_return_and_newline() {
    let text = "Exomizer, unpacker=$0134\rpass2, return to mem: $0810\rsaved $0810-$fff9 as o\r";
    let console = parse_console(text);
    assert_eq!(console.entry, Some(Unp64Entry::Address(0x0810)));
    assert_eq!(
        console.saved,
        Some(SavedLine {
            start: 0x0810,
            end: 0xFFF9
        })
    );
    assert_eq!(
        console.packer.map(|packer| packer.name),
        Some("Exomizer".to_string())
    );
}

/// T1.A2.5
#[test]
fn parse_crlf_line_endings() {
    let console = parse_console(&TRAVELLER.replace('\n', "\r\n"));
    assert_eq!(console, parse_console(TRAVELLER));
    assert_eq!(console.entry, Some(Unp64Entry::Address(0x0911)));
}

/// T1.A2.6: without a `saved` line the run failed, whatever the exit code;
/// the diagnostic becomes the reason.
#[test]
fn parse_without_saved_line_is_unp64_failure() {
    let console = parse_console(MAX_ITERATIONS);
    assert_eq!(console.saved, None);
    assert_eq!(
        interpret_run(&console, None),
        Ok(Unp64Outcome::Failed {
            reason: "Max Iterations 0x01000000 Reached, quitting...".to_string(),
            missing_opcode: false,
        })
    );
    // A missing-opcode stop is flagged for the classifier, and a run with no
    // diagnostic at all still fails with a fixed reason.
    let opcode = parse_console("unimplemented opcode $02 @ $0812\n");
    assert!(opcode.missing_opcode);
    assert!(matches!(
        interpret_run(&opcode, None),
        Ok(Unp64Outcome::Failed { missing_opcode: true, reason }) if reason.contains("$0812")
    ));
    assert!(matches!(
        interpret_run(&parse_console("Entry point: $080d\n"), None),
        Ok(Unp64Outcome::Failed { missing_opcode: false, reason }) if reason.contains("saved")
    ));
}

/// T1.A2.7
#[test]
fn parse_pass2_plain_address() {
    assert_eq!(pass2_entry("$0911\n"), Some(Unp64Entry::Address(0x0911)));
}

/// T1.A2.8
#[test]
fn parse_pass2_basic_run_with_sys() {
    assert_eq!(
        pass2_entry("($a7ae)->$080d\n"),
        Some(Unp64Entry::Address(0x080D))
    );
}

/// T1.A2.9
#[test]
fn parse_pass2_run_without_sys() {
    assert_eq!(pass2_entry("($a7ae)->RUN\n"), Some(Unp64Entry::BasicRun));
}

/// T1.A2.10
#[test]
fn parse_pass2_new_basic_start_run() {
    assert_eq!(
        pass2_entry("($a7ae)->(new basic start $1001)->RUN\n"),
        Some(Unp64Entry::BasicRun)
    );
}

/// T1.A2.11: in verbose mode a `forced` diagnostic sits between the marker
/// and the value.
#[test]
fn parse_pass2_forced_run() {
    assert_eq!(
        pass2_entry("\n$a7b1 -> forced RUN\n($a7ae)->$080d\n"),
        Some(Unp64Entry::Address(0x080D))
    );
    assert_eq!(
        pass2_entry("\rMonitored end pointer $00ae+$00af -> $c000\n$2000\n"),
        Some(Unp64Entry::Address(0x2000))
    );
}

/// T1.A2.12
#[test]
fn parse_pass2_missing_is_none() {
    assert_eq!(parse_console("saved $0801-$0810 as x\n").entry, None);
    assert_eq!(parse_console(MAX_ITERATIONS).entry, None);
}

/// T1.A2.13
#[test]
fn parse_packer_name_and_unpacker_address() {
    assert_eq!(
        parse_console(TRAVELLER).packer,
        Some(PackerLine {
            name: "TinyCrunch v1.0/ChristopherJam".to_string(),
            unpacker: 0x010E,
        })
    );
    // A comma inside the name is kept (the separator is the last one).
    assert_eq!(
        parse_console("Dali v0.3.3 / fast, beta, unpacker=$0003\n")
            .packer
            .map(|packer| (packer.name, packer.unpacker)),
        Some(("Dali v0.3.3 / fast, beta".to_string(), 0x0003))
    );
}

/// T1.A2.14
#[test]
fn parse_version_banner_gives_version() {
    assert_eq!(parse_version_banner(BANNER), Ok("2.42".to_string()));
    assert_eq!(
        parse_version_banner("usage: something else\n"),
        Err(ParseError::MissingBanner)
    );
    assert_eq!(
        parse_version_banner("UNP64 vX - broken\n"),
        Err(ParseError::MissingBanner)
    );
}

/// T1.A2.15: the file is authoritative for start, length and bytes.
#[test]
fn derive_from_output_file() {
    let saved = derive_saved_payload(&prg(0x0801, 16)).unwrap();
    assert_eq!(saved.range, range(0x0801, 0x0810));
    assert_eq!(saved.payload, vec![0xEA; 16]);
}

/// T1.A2.16
#[test]
fn derive_from_output_file_wrap_to_ffff() {
    let saved = derive_saved_payload(&prg(0x0800, 0xF800)).unwrap();
    assert_eq!(saved.range, range(0x0800, 0xFFFF));
    assert_eq!(saved.range.len(), 63_488);
}

/// T1.A2.17
#[test]
fn derive_from_output_file_too_short_is_error() {
    for bytes in [vec![], vec![0x01], vec![0x01, 0x08]] {
        assert_eq!(
            derive_saved_payload(&bytes),
            Err(ParseError::OutputTooShort { len: bytes.len() }),
            "{bytes:?}"
        );
    }
}

/// T1.A2.18
#[test]
fn derive_payload_exceeding_address_space_is_error() {
    assert_eq!(
        derive_saved_payload(&prg(0xFF00, 257)),
        Err(ParseError::OutputExceedsAddressSpace {
            start: 0xFF00,
            len: 257
        })
    );
}

/// T1.A2.19: the `saved` line must agree with the file, and a run that
/// claims a payload needs the file and an entry.
#[test]
fn cross_check_saved_line_against_file() {
    let printed = SavedLine {
        start: 0x0801,
        end: 0x0810,
    };
    assert_eq!(cross_check(printed, range(0x0801, 0x0810)), Ok(()));
    assert_eq!(
        cross_check(printed, range(0x0801, 0x0811)),
        Err(ParseError::SavedLineMismatch {
            printed,
            derived: range(0x0801, 0x0811)
        })
    );

    let console = parse_console("pass2, return to mem: $080d\nsaved $0801-$0810 as x\n");
    assert!(matches!(
        interpret_run(&console, Some(&prg(0x0801, 16))),
        Ok(Unp64Outcome::Saved {
            entry: Unp64Entry::Address(0x080D),
            ..
        })
    ));
    assert_eq!(
        interpret_run(&console, Some(&prg(0x0801, 17))),
        Err(ParseError::SavedLineMismatch {
            printed,
            derived: range(0x0801, 0x0811)
        })
    );
    assert_eq!(
        interpret_run(&console, None),
        Err(ParseError::MissingOutputFile)
    );
    let no_entry = parse_console("saved $0801-$0810 as x\n");
    assert_eq!(
        interpret_run(&no_entry, Some(&prg(0x0801, 16))),
        Err(ParseError::MissingPass2Entry)
    );
}

/// T1.A2.42: an explicit `UNP64` is the only candidate; otherwise `PATH`
/// and `~/.local/bin`. No candidate names a user directory of its own.
#[test]
fn unp64_binary_candidates_have_no_hardcoded_user_paths() {
    assert_eq!(
        binary_candidates(
            Some(OsString::from("/opt/unp64")),
            Some(OsString::from("/home/a"))
        ),
        vec![PathBuf::from("/opt/unp64")]
    );
    assert_eq!(
        binary_candidates(None, Some(OsString::from("/home/a"))),
        vec![
            PathBuf::from("unp64"),
            PathBuf::from("/home/a/.local/bin/unp64")
        ]
    );
    assert_eq!(
        binary_candidates(Some(OsString::new()), None),
        vec![PathBuf::from("unp64")]
    );
    let home = "/nonexistent-home-for-test";
    let mentions_foreign_home = binary_candidates(None, Some(OsString::from(home)))
        .iter()
        .any(|candidate| {
            let text = candidate.to_string_lossy();
            text.contains("/Users/") || (text.starts_with('/') && !text.starts_with(home))
        });
    assert!(!mentions_foreign_home);
}

/// T1.A2.43
#[test]
fn tmp_output_path_is_unique_per_case() {
    let dir = Path::new("/tmp");
    let paths: std::collections::BTreeSet<PathBuf> = (0..64)
        .map(|index| tmp_output_path(dir, 4242, index))
        .collect();
    assert_eq!(paths.len(), 64);
    assert!(paths.iter().all(|path| path.parent() == Some(dir)));
    assert_ne!(tmp_output_path(dir, 1, 0), tmp_output_path(dir, 2, 0));
    assert_eq!(
        tmp_output_path(dir, 42, 7),
        PathBuf::from("/tmp/unpacker_compare_all-42-007.prg")
    );
}

/// T1.A2.44: the output always goes through `-o` (a positional output name
/// makes unp64 write next to the fixture).
#[test]
fn unp64_command_line_uses_dash_o() {
    assert_eq!(
        unp64_args(Path::new("/f/in put.prg"), Path::new("/tmp/out.prg")),
        vec![
            OsString::from("/f/in put.prg"),
            OsString::from("-o"),
            OsString::from("/tmp/out.prg")
        ]
    );
}

/// T1.A2.46: a non-zero exit with a banner is the normal probe result; a
/// missing banner or a spawn error is a tool failure.
#[test]
fn version_probe_nonzero_exit_is_not_tool_failure() {
    let program = Path::new("unp64");
    let probe = |exit_code, text: &str| {
        Ok(ProbeOutput {
            exit_code,
            text: text.to_string(),
        })
    };
    assert_eq!(
        interpret_version_probe(program, probe(Some(1), BANNER)),
        Ok("2.42".to_string())
    );
    assert_eq!(
        interpret_version_probe(program, probe(Some(0), BANNER)),
        Ok("2.42".to_string())
    );
    assert_eq!(
        interpret_version_probe(program, probe(Some(1), "command not understood\n")),
        Err(Unp64Error::NoBanner {
            program: program.to_path_buf(),
            exit_code: Some(1)
        })
    );
    assert!(matches!(
        interpret_version_probe(
            program,
            Err(io::Error::new(io::ErrorKind::NotFound, "no such file"))
        ),
        Err(Unp64Error::Spawn { message, .. }) if message.contains("no such file")
    ));
}
