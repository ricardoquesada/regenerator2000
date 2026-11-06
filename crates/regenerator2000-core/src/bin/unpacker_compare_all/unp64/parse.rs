//! Pure parsing for `unp64` output: console text, usage banner, and saved PRG
//! files.
//!
//! `unp64` returns exit code `0` even when it saves no file. Therefore, a run
//! succeeds only when `unp64` prints a `saved $SSSS-$EEEE as …` line and
//! writes a matching output file. Progress output uses bare carriage returns,
//! so the parser splits lines on both `\r` and `\n`.

use std::fmt;

use crate::corpus::{AddrRange, HexAddr, PRG_HEADER_LEN, Unp64Entry, Unp64Observation};

/// Prefix of the line that reports the saved range, such as
/// `saved $0801-$fffd as out.prg`.
const SAVED_PREFIX: &str = "saved ";

/// Separator between the saved range and the output file name.
const SAVED_NAME_SEPARATOR: &str = " as ";

/// Marker of the line that reports the entry point after the second pass,
/// such as `pass2, return to mem: $0911`.
const PASS2_MARKER: &str = "pass2, return to mem:";

/// Arrow between steps in an `unp64` entry chain, such as `($a7ae)->$080d` or
/// `($a7ae)->(new basic start $1001)->RUN`.
const CHAIN_ARROW: &str = "->";

/// Token that `unp64` prints when control returns to BASIC `RUN` without a
/// `SYS` target.
const BASIC_RUN_VALUE: &str = "RUN";

/// Marker for a verbose-mode diagnostic such as `$a7ae -> forced RUN`.
const DEBUG_FORCED_MARKER: &str = "-> forced ";

/// Prefix for a verbose-mode pointer-monitor diagnostic.
const DEBUG_MONITORED_PREFIX: &str = "Monitored ";

/// Separator between the packer name and the depacker address, such as
/// `TinyCrunch v1.0/ChristopherJam, unpacker=$010e`.
const UNPACKER_SEPARATOR: &str = ", unpacker=";

/// Prefix of the usage banner line, such as
/// `UNP64 v2.42 - Generic C64 prg unpacker - …`.
const BANNER_PREFIX: &str = "UNP64 v";

/// Diagnostics that explain why `unp64` saved nothing (`unp64.c:738`,
/// `unp64.c:868`, `unp64.c:551`, `6502emu.c:1241`).
const FAILURE_MARKERS: [&str; 3] = [
    "Max Iterations",
    "can't find entry point",
    MISSING_OPCODE_MARKER,
];

/// Diagnostic for an opcode that the `unp64` CPU core does not implement
/// (`NULL_OP`, `6502emu.c:1241`).
const MISSING_OPCODE_MARKER: &str = "unimplemented opcode";

/// Failure reason when `unp64` prints neither a `saved` line nor a known
/// diagnostic.
const NO_SAVED_LINE_REASON: &str = "no `saved` line in unp64 output";

// =============================================================================
// Errors
// =============================================================================

/// Unrecordable `unp64` result.
///
/// These variants represent tool errors rather than normal unpack failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The output contains no `UNP64 vX.YY …` usage banner.
    MissingBanner,
    /// `unp64` printed a `saved` line but wrote no output file.
    MissingOutputFile,
    /// The output file contains no payload bytes after its PRG header.
    OutputTooShort {
        /// The file length in bytes.
        len: usize,
    },
    /// The output file payload extends past `$FFFF`.
    OutputExceedsAddressSpace {
        /// The load address from the header.
        start: u16,
        /// The payload length.
        len: usize,
    },
    /// The `saved` line and the output file disagree on the address range.
    SavedLineMismatch {
        /// The range from the `saved` line.
        printed: SavedLine,
        /// The range from the output file.
        derived: AddrRange,
    },
    /// `unp64` saved a payload but printed no parsable `pass2` entry.
    MissingPass2Entry,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBanner => write!(
                f,
                "no `{BANNER_PREFIX}…` usage banner in the output of a no-argument unp64 call"
            ),
            Self::MissingOutputFile => {
                f.write_str("unp64 printed a `saved` line but wrote no output file")
            }
            Self::OutputTooShort { len } => write!(
                f,
                "unp64's output file has {len} byte(s), but a PRG needs a {PRG_HEADER_LEN}-byte \
                 header and at least one payload byte"
            ),
            Self::OutputExceedsAddressSpace { start, len } => write!(
                f,
                "unp64's output file loads at {} with {len} payload byte(s), which passes $FFFF",
                HexAddr(*start)
            ),
            Self::SavedLineMismatch { printed, derived } => write!(
                f,
                "unp64 printed `saved {}-{}`, but its output file covers {derived}",
                HexAddr(printed.start),
                HexAddr(printed.end)
            ),
            Self::MissingPass2Entry => f.write_str(
                "unp64 saved a payload but printed no parsable `pass2, return to mem:` entry",
            ),
        }
    }
}

impl std::error::Error for ParseError {}

// =============================================================================
// Console output
// =============================================================================

/// Address range from a `saved $SSSS-$EEEE as …` line.
///
/// `end` is inclusive (`(EndAdr - 1) & $FFFF`, `unp64.c:1238`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedLine {
    /// First saved address.
    pub start: u16,
    /// Last saved address.
    pub end: u16,
}

/// Packer identification from a `<Name>, unpacker=$XXXX` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackerLine {
    /// The packer name, such as `TinyCrunch v1.0/ChristopherJam`.
    pub name: String,
    /// The depacker address that `unp64` found.
    pub unpacker: u16,
}

/// Parsed fields from the console output of one `unp64` run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Console {
    /// The `saved` line, if `unp64` saved a payload.
    pub saved: Option<SavedLine>,
    /// The entry point from the second pass.
    pub entry: Option<Unp64Entry>,
    /// The `unp64` packer identification.
    pub packer: Option<PackerLine>,
    /// The first failure diagnostic, such as `Max Iterations 0x01000000
    /// Reached, quitting...`.
    pub diagnostic: Option<String>,
    /// `true` if `unp64` stopped on an unimplemented opcode.
    pub missing_opcode: bool,
}

/// Splits console text into trimmed, non-empty lines on `\r`, `\n`, and `\r\n`.
fn console_lines(text: &str) -> Vec<&str> {
    text.split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

/// Parses `$hhhh` (four hexadecimal digits in any case).
fn parse_dollar_hex(token: &str) -> Option<u16> {
    HexAddr::parse(token.trim()).ok().map(|addr| addr.0)
}

/// Parses a `saved $0801-$fffd as out.prg` line.
///
/// # Example
///
/// `saved $0801-$fffd as x` returns `SavedLine { start: 0x0801, end: 0xFFFD }`.
fn parse_saved_line(line: &str) -> Option<SavedLine> {
    let rest = line.strip_prefix(SAVED_PREFIX)?;
    let (range, _name) = rest.split_once(SAVED_NAME_SEPARATOR)?;
    let (start, end) = range.split_once('-')?;
    Some(SavedLine {
        start: parse_dollar_hex(start)?,
        end: parse_dollar_hex(end)?,
    })
}

/// Parses the final step of an entry chain: `$0911`, `($a7ae)->$080d`,
/// `($a7ae)->RUN`, or `($a7ae)->(new basic start $1001)->RUN`.
fn parse_entry_chain(line: &str) -> Option<Unp64Entry> {
    let last = line.rsplit(CHAIN_ARROW).next()?.trim();
    if last == BASIC_RUN_VALUE {
        return Some(Unp64Entry::BasicRun);
    }
    parse_dollar_hex(last).map(Unp64Entry::Address)
}

/// Returns `true` for verbose-mode diagnostics that `unp64` prints between the
/// `pass2` marker and the entry value.
fn is_debug_diagnostic(line: &str) -> bool {
    line.contains(DEBUG_FORCED_MARKER) || line.starts_with(DEBUG_MONITORED_PREFIX)
}

/// Parses the entry value after `pass2, return to mem:`.
///
/// The value normally appears on the same line after the marker. In verbose
/// mode, `unp64` can print debug diagnostics before the entry value, so this
/// function skips those diagnostic lines.
fn parse_pass2_entry(lines: &[&str]) -> Option<Unp64Entry> {
    let (index, line) = lines
        .iter()
        .enumerate()
        .find(|(_, line)| line.contains(PASS2_MARKER))?;
    let (_, remainder) = line.split_once(PASS2_MARKER)?;
    std::iter::once(remainder.trim())
        .chain(lines.iter().skip(index + 1).copied())
        .find(|candidate| !candidate.is_empty() && !is_debug_diagnostic(candidate))
        .and_then(parse_entry_chain)
}

/// Parses a `<Name>, unpacker=$XXXX` line.
fn parse_packer_line(line: &str) -> Option<PackerLine> {
    let (name, address) = line.rsplit_once(UNPACKER_SEPARATOR)?;
    Some(PackerLine {
        name: name.trim().to_string(),
        unpacker: parse_dollar_hex(address)?,
    })
}

/// Parses the console output of one `unp64` run.
///
/// # Example
///
/// ```text
/// TinyCrunch v1.0/ChristopherJam, unpacker=$010e
/// pass2, return to mem: $0911
/// saved $0801-$fffd as /tmp/x.prg
/// ```
///
/// returns the saved range `$0801-$FFFD`, the entry `$0911`, and the packer
/// `TinyCrunch v1.0/ChristopherJam` at `$010E`.
#[must_use]
pub fn parse_console(text: &str) -> Console {
    let lines = console_lines(text);
    Console {
        saved: lines.iter().find_map(|line| parse_saved_line(line)),
        entry: parse_pass2_entry(&lines),
        packer: lines.iter().find_map(|line| parse_packer_line(line)),
        diagnostic: lines
            .iter()
            .find(|line| FAILURE_MARKERS.iter().any(|marker| line.contains(marker)))
            .map(|line| (*line).to_string()),
        missing_opcode: lines
            .iter()
            .any(|line| line.contains(MISSING_OPCODE_MARKER)),
    }
}

// =============================================================================
// Version banner
// =============================================================================

/// Extracts the version string from the `unp64` usage banner.
///
/// # Example
///
/// `UNP64 v2.42 - Generic C64 prg unpacker - (C)2008-2026 iAN CooG` returns
/// `"2.42"`.
///
/// # Errors
///
/// Returns [`ParseError::MissingBanner`] if no line starts with `UNP64 v`
/// followed by digits and dots.
pub fn parse_version_banner(text: &str) -> Result<String, ParseError> {
    console_lines(text)
        .into_iter()
        .find_map(|line| {
            let version = line
                .strip_prefix(BANNER_PREFIX)?
                .split_whitespace()
                .next()?;
            let well_formed = version.starts_with(|c: char| c.is_ascii_digit())
                && version.chars().all(|c| c.is_ascii_digit() || c == '.');
            well_formed.then(|| version.to_string())
        })
        .ok_or(ParseError::MissingBanner)
}

// =============================================================================
// Output file and whole-run interpretation
// =============================================================================

/// Saved payload from an `unp64` output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedPayload {
    /// Address range from the header load address to the last payload byte.
    pub range: AddrRange,
    /// Payload bytes without the PRG header.
    pub payload: Vec<u8>,
}

/// Derives the saved range and payload from an `unp64` output file.
///
/// The output file defines `start` from its header, `len` from its payload
/// length, and `end = start + len - 1`.
///
/// # Example
///
/// A file that starts with `00 08` followed by `$F800` bytes covers
/// `$0800-$FFFF`.
///
/// # Errors
///
/// Returns [`ParseError::OutputTooShort`] for a file without payload bytes,
/// or [`ParseError::OutputExceedsAddressSpace`] if the payload extends past
/// `$FFFF`.
pub fn derive_saved_payload(file: &[u8]) -> Result<SavedPayload, ParseError> {
    let Some((header, payload)) = file.split_first_chunk::<PRG_HEADER_LEN>() else {
        return Err(ParseError::OutputTooShort { len: file.len() });
    };
    if payload.is_empty() {
        return Err(ParseError::OutputTooShort { len: file.len() });
    }
    let start = u16::from_le_bytes(*header);
    let range = AddrRange::from_start_len(start, payload.len()).ok_or(
        ParseError::OutputExceedsAddressSpace {
            start,
            len: payload.len(),
        },
    )?;
    Ok(SavedPayload {
        range,
        payload: payload.to_vec(),
    })
}

/// Checks that the `saved` line and the output file describe the same range.
///
/// # Errors
///
/// Returns [`ParseError::SavedLineMismatch`] if the start or end addresses
/// differ.
pub fn cross_check(printed: SavedLine, derived: AddrRange) -> Result<(), ParseError> {
    if printed.start == derived.start() && printed.end == derived.end() {
        return Ok(());
    }
    Err(ParseError::SavedLineMismatch { printed, derived })
}

/// Outcome of one `unp64` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unp64Outcome {
    /// `unp64` saved a payload.
    Saved {
        /// The saved range and payload bytes.
        saved: SavedPayload,
        /// The entry point from the second pass.
        entry: Unp64Entry,
        /// The `unp64` packer identification, if present.
        packer: Option<PackerLine>,
    },
    /// `unp64` saved nothing.
    Failed {
        /// The `unp64` diagnostic, or a default reason if none appeared.
        reason: String,
        /// `true` if `unp64` stopped on an unimplemented opcode.
        missing_opcode: bool,
    },
}

impl Unp64Outcome {
    /// Converts the outcome into [`Unp64Observation`] for the classifier.
    #[must_use]
    pub fn observation(&self) -> Unp64Observation {
        match self {
            Self::Saved { saved, entry, .. } => Unp64Observation::Saved {
                start: saved.range.start(),
                entry: *entry,
                payload: saved.payload.clone(),
            },
            Self::Failed {
                reason,
                missing_opcode,
            } => Unp64Observation::Failed {
                reason: reason.clone(),
                missing_opcode: *missing_opcode,
            },
        }
    }

    /// Returns the `unp64` packer name, if present.
    #[must_use]
    pub fn packer_name(&self) -> Option<&str> {
        match self {
            Self::Saved {
                packer: Some(packer),
                ..
            } => Some(&packer.name),
            Self::Saved { packer: None, .. } | Self::Failed { .. } => None,
        }
    }
}

/// Interprets one `unp64` run from its console output and optional output file.
///
/// When the console output contains no `saved` line, the run is a failure
/// regardless of the exit code. When a `saved` line is present, the output
/// file and `pass2` entry must both be valid.
///
/// # Errors
///
/// Returns [`ParseError`] if `unp64` printed a `saved` line but the output
/// file is missing, malformed, inconsistent with the `saved` line, or missing
/// a `pass2` entry.
pub fn interpret_run(console: &Console, output: Option<&[u8]>) -> Result<Unp64Outcome, ParseError> {
    let Some(printed) = console.saved else {
        return Ok(Unp64Outcome::Failed {
            reason: console
                .diagnostic
                .clone()
                .unwrap_or_else(|| NO_SAVED_LINE_REASON.to_string()),
            missing_opcode: console.missing_opcode,
        });
    };
    let file = output.ok_or(ParseError::MissingOutputFile)?;
    let saved = derive_saved_payload(file)?;
    cross_check(printed, saved.range)?;
    let entry = console.entry.ok_or(ParseError::MissingPass2Entry)?;
    Ok(Unp64Outcome::Saved {
        saved,
        entry,
        packer: console.packer.clone(),
    })
}
