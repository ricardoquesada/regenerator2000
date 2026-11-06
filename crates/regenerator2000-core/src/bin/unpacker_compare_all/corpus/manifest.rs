//! The corpus manifest model, its strict TOML parser, and its deterministic
//! writer.
//!
//! Every table sets `deny_unknown_fields`, so a typo (for example
//! `acepted_divergence`) produces a parse error instead of dropping a
//! reviewed exception.

use std::num::NonZeroU64;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::kinds::{DivergenceKind, ExpectedFailReason, FieldSet};
use super::values::{AddrRange, Fnv64, HexAddr, Unp64Entry};
use super::{CorpusError, DEFAULT_MAX_INSTRUCTIONS, INITIAL_REVIEW_MARKER};

/// Header text at the top of the manifest. The writer does not preserve TOML
/// comments, so the header tells humans where to put notes.
const MANIFEST_HEADER: &str = "\
# Unpacker corpus manifest. `[case.unp64]` tables are written by
# `cargo run -p regenerator2000-core --bin unpacker_compare_all -- --update`.
# Comments are not preserved: put human notes in `note` and sign reviewed
# exceptions with `reviewed_by`.

";

// =============================================================================
// Recorded unp64 results
// =============================================================================

/// The window over which the `unp64` and R2000 payloads overlap, with the hash
/// of the `unp64` bytes over that window.
///
/// The `--update` command records this value whenever the two ranges differ,
/// so CI can check payload agreement over the overlap without `unp64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlap {
    /// First address of the window.
    pub start: HexAddr,
    /// Last address of the window (inclusive).
    pub end: HexAddr,
    /// FNV-1a-64 of the `unp64` bytes over the window.
    pub fnv64: Fnv64,
}

impl Overlap {
    /// Returns the window as a range, or `None` if `end < start`.
    #[must_use]
    pub fn range(&self) -> Option<AddrRange> {
        AddrRange::new(self.start.0, self.end.0)
    }
}

/// A successful `unp64` run from `--update`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unp64Saved {
    /// First saved address.
    pub start: HexAddr,
    /// Last saved address (inclusive).
    pub end: HexAddr,
    /// The entry point from `unp64`.
    pub entry: Unp64Entry,
    /// Saved payload length (`end - start + 1`).
    pub len: u32,
    /// FNV-1a-64 of the saved payload.
    pub payload_fnv64: Fnv64,
    /// Overlap window when the R2000 range differed at record time.
    pub overlap: Option<Overlap>,
    /// The `unp64` version that produced the record.
    pub version: String,
}

impl Unp64Saved {
    /// Returns the saved range, or `None` if the record is inconsistent.
    #[must_use]
    pub fn range(&self) -> Option<AddrRange> {
        AddrRange::new(self.start.0, self.end.0)
    }
}

/// An `unp64` run that saved no output (for example "Max Iterations … quitting").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unp64Failed {
    /// Short description of why `unp64` failed.
    pub reason: String,
    /// The `unp64` version that produced the record.
    pub version: String,
}

/// The recorded `unp64` result of one case: either saved output or a failure.
///
/// The manifest form is one `[case.unp64]` table. A failed run contains only
/// `failed` and `version`. Combining `failed` with any result key produces a
/// parse error.
///
/// # Example
///
/// ```toml
/// [case.unp64]
/// failed = "Max Iterations"
/// version = "2.42"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawUnp64Record", into = "RawUnp64Record")]
pub enum Unp64Record {
    /// `unp64` saved a payload.
    Saved(Unp64Saved),
    /// `unp64` saved no output.
    Failed(Unp64Failed),
}

/// Flat TOML shape of [`Unp64Record`] for serialization and deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnp64Record {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start: Option<HexAddr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end: Option<HexAddr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entry: Option<Unp64Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    len: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    payload_fnv64: Option<Fnv64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    failed: Option<String>,
    version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlap: Option<Overlap>,
}

impl TryFrom<RawUnp64Record> for Unp64Record {
    type Error = String;

    fn try_from(raw: RawUnp64Record) -> Result<Self, Self::Error> {
        let result_keys = [
            ("start", raw.start.is_some()),
            ("end", raw.end.is_some()),
            ("entry", raw.entry.is_some()),
            ("len", raw.len.is_some()),
            ("payload_fnv64", raw.payload_fnv64.is_some()),
            ("overlap", raw.overlap.is_some()),
        ];

        if let Some(reason) = raw.failed {
            let conflicting: Vec<&str> = result_keys
                .iter()
                .filter(|(_, present)| *present)
                .map(|(key, _)| *key)
                .collect();
            if !conflicting.is_empty() {
                return Err(format!(
                    "`failed` cannot be combined with result keys ({}): a failed unp64 run \
                     saved nothing",
                    conflicting.join(", ")
                ));
            }
            return Ok(Self::Failed(Unp64Failed {
                reason,
                version: raw.version,
            }));
        }

        let missing =
            |key: &str| format!("[case.unp64] is missing `{key}` (or use `failed = \"<reason>\"`)");
        Ok(Self::Saved(Unp64Saved {
            start: raw.start.ok_or_else(|| missing("start"))?,
            end: raw.end.ok_or_else(|| missing("end"))?,
            entry: raw.entry.ok_or_else(|| missing("entry"))?,
            len: raw.len.ok_or_else(|| missing("len"))?,
            payload_fnv64: raw.payload_fnv64.ok_or_else(|| missing("payload_fnv64"))?,
            overlap: raw.overlap,
            version: raw.version,
        }))
    }
}

impl From<Unp64Record> for RawUnp64Record {
    fn from(record: Unp64Record) -> Self {
        match record {
            Unp64Record::Saved(saved) => Self {
                start: Some(saved.start),
                end: Some(saved.end),
                entry: Some(saved.entry),
                len: Some(saved.len),
                payload_fnv64: Some(saved.payload_fnv64),
                failed: None,
                version: saved.version,
                overlap: saved.overlap,
            },
            Unp64Record::Failed(failed) => Self {
                start: None,
                end: None,
                entry: None,
                len: None,
                payload_fnv64: None,
                failed: Some(failed.reason),
                version: failed.version,
                overlap: None,
            },
        }
    }
}

// =============================================================================
// Reviewed exceptions
// =============================================================================

/// The review state of an [`AcceptedDivergence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    /// `reviewed_by` is missing or blank: a proposal that does not count.
    Unsigned,
    /// `reviewed_by` equals [`INITIAL_REVIEW_MARKER`]: counts, but awaits
    /// user approval.
    PendingUser,
    /// A reviewer signed the exception.
    Signed,
}

/// A reviewed exception where R2000 intentionally differs from `unp64` in the
/// listed fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedDivergence {
    /// Why the tools differ.
    pub kind: DivergenceKind,
    /// The only fields that may differ from `unp64`.
    pub fields: FieldSet,
    /// Reviewer name. Missing or blank means a proposed, unsigned exception.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    /// Free-form human context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl AcceptedDivergence {
    /// Classifies `reviewed_by`.
    ///
    /// # Example
    ///
    /// `None`, `""`, and `"  "` map to [`ReviewState::Unsigned`].
    /// [`INITIAL_REVIEW_MARKER`] maps to [`ReviewState::PendingUser`].
    /// `"ricardoq"` maps to [`ReviewState::Signed`].
    #[must_use]
    pub fn review_state(&self) -> ReviewState {
        match self.reviewed_by.as_deref().map(str::trim) {
            None | Some("") => ReviewState::Unsigned,
            Some(INITIAL_REVIEW_MARKER) => ReviewState::PendingUser,
            Some(_) => ReviewState::Signed,
        }
    }
}

// =============================================================================
// Groups, cases and the corpus
// =============================================================================

/// A set of cases that pack the same program, with the original binary as
/// ground truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    /// Unique group identifier for [`Case::group`].
    pub id: String,
    /// File name (in the fixture directory) of the original unpacked PRG.
    pub original: String,
    /// Entry point of the original program.
    pub entry: HexAddr,
}

/// One packed fixture and its oracle data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// File name of the packed PRG in the fixture directory.
    pub file: String,
    /// Ground-truth group this case belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Instruction budget for the corpus test, the compare tool, and the
    /// baseline. Defaults to [`DEFAULT_MAX_INSTRUCTIONS`]. The parser rejects
    /// zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_instructions: Option<NonZeroU64>,
    /// Marks a case that must currently fail its oracle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_fail: Option<ExpectedFailReason>,
    /// The recorded `unp64` result (`Corpus::validate` requires this field).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unp64: Option<Unp64Record>,
    /// A reviewed exception to the `unp64` reference rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_divergence: Option<AcceptedDivergence>,
}

impl Case {
    /// Returns the instruction budget for this case.
    #[must_use]
    pub fn effective_max_instructions(&self) -> u64 {
        self.max_instructions
            .map_or(DEFAULT_MAX_INSTRUCTIONS, NonZeroU64::get)
    }
}

/// The whole corpus manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Ground-truth groups (`[[group]]`).
    #[serde(rename = "group", default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    /// Cases (`[[case]]`).
    #[serde(rename = "case", default, skip_serializing_if = "Vec::is_empty")]
    pub cases: Vec<Case>,
}

impl Corpus {
    /// Parses a manifest from TOML text and runs structural checks.
    /// Call [`Corpus::validate`] to check cross-references and fixtures.
    ///
    /// # Errors
    ///
    /// Returns [`CorpusError::Parse`] for invalid TOML, unknown keys, unknown
    /// enum values, malformed addresses or hashes, or an invalid
    /// `[case.unp64]` shape. The message names the offending key and line.
    pub fn parse(toml_text: &str) -> Result<Self, CorpusError> {
        toml::from_str(toml_text).map_err(|e| CorpusError::Parse {
            message: e.to_string(),
        })
    }

    /// Reads and parses the manifest at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`CorpusError::Io`] on a file read error, or any error from
    /// [`Corpus::parse`].
    pub fn load(path: &Path) -> Result<Self, CorpusError> {
        let text = std::fs::read_to_string(path).map_err(|e| CorpusError::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        Self::parse(&text)
    }

    /// Writes the manifest in its canonical form.
    ///
    /// The output sorts groups by `id` and cases by `file`, formats addresses
    /// as `"$XXXX"`, and formats hashes as 16 lowercase hex digits. Writing a
    /// parsed canonical manifest reproduces it exactly.
    ///
    /// # Errors
    ///
    /// Returns [`CorpusError::Serialize`] if TOML serialization fails.
    pub fn to_toml_string(&self) -> Result<String, CorpusError> {
        let mut canonical = self.clone();
        canonical.groups.sort_by(|a, b| a.id.cmp(&b.id));
        canonical.cases.sort_by(|a, b| a.file.cmp(&b.file));
        let body = toml::to_string(&canonical).map_err(|e| CorpusError::Serialize {
            message: e.to_string(),
        })?;
        Ok(format!("{MANIFEST_HEADER}{body}"))
    }

    /// Returns the case for `file`.
    #[must_use]
    pub fn case(&self, file: &str) -> Option<&Case> {
        self.cases.iter().find(|case| case.file == file)
    }

    /// Returns the group with `id`.
    #[must_use]
    pub fn group(&self, id: &str) -> Option<&Group> {
        self.groups.iter().find(|group| group.id == id)
    }

    /// Returns every case that belongs to group `id`.
    pub fn members<'c>(&'c self, id: &'c str) -> impl Iterator<Item = &'c Case> + 'c {
        self.cases
            .iter()
            .filter(move |case| case.group.as_deref() == Some(id))
    }
}
