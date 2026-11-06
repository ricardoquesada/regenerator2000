//! Error type for the corpus module.

use std::fmt;
use std::path::PathBuf;

use super::kinds::{DivergenceKind, Field, join_fields};
use super::values::HexAddr;

/// Error in the corpus manifest, its fixtures, or a case filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusError {
    /// File I/O error on a manifest, baseline, or fixture file.
    Io {
        /// The file path.
        path: PathBuf,
        /// The I/O error message.
        message: String,
    },
    /// The TOML text is malformed or violates the schema.
    Parse {
        /// The parser message, including line and key.
        message: String,
    },
    /// TOML serialization failed.
    Serialize {
        /// The serializer message.
        message: String,
    },
    /// A file name contains a path separator or is empty.
    InvalidFileName {
        /// The rejected name.
        file: String,
    },
    /// Two cases list the same file name.
    DuplicateCase {
        /// The duplicate file name.
        file: String,
    },
    /// Two groups share an `id`.
    DuplicateGroup {
        /// The duplicate group ID.
        group: String,
    },
    /// A case fixture file is missing.
    MissingFixture {
        /// The missing file name.
        file: String,
    },
    /// A group original file is missing.
    MissingGroupOriginal {
        /// The group ID.
        group: String,
        /// The missing original file name.
        file: String,
    },
    /// A group original file is not a valid PRG.
    InvalidGroupOriginal {
        /// The group ID.
        group: String,
        /// The original file name.
        file: String,
        /// Reason why the file is invalid.
        reason: String,
    },
    /// A case references an unknown group ID.
    UnknownGroup {
        /// The case file name.
        file: String,
        /// The unknown group ID.
        group: String,
    },
    /// A case has no `[case.unp64]` table.
    MissingUnp64 {
        /// The case file name.
        file: String,
    },
    /// The recorded `len` does not equal `end - start + 1`.
    Unp64LenMismatch {
        /// The case file name.
        file: String,
        /// The recorded length.
        recorded: u32,
        /// The length from the range, or `None` if `end < start`.
        derived: Option<usize>,
    },
    /// The recorded overlap window extends outside the `unp64` range.
    OverlapOutsideUnp64Range {
        /// The case file name.
        file: String,
        /// The recorded window.
        overlap: (HexAddr, HexAddr),
        /// The `unp64` range.
        unp64: (HexAddr, HexAddr),
    },
    /// `unp64` failed, but the case has no `unexplained` or
    /// `unp64_missing_opcode` exception.
    FailedUnp64WithoutDivergence {
        /// The case file name.
        file: String,
    },
    /// The divergence kind requires a failed `unp64` record.
    DivergenceKindNeedsFailedUnp64 {
        /// The case file name.
        file: String,
        /// The divergence kind.
        kind: DivergenceKind,
    },
    /// The divergence lists fields that do not match the required fields for
    /// its kind.
    DivergenceFieldsMismatch {
        /// The case file name.
        file: String,
        /// The divergence kind.
        kind: DivergenceKind,
        /// The required field list.
        required: &'static [Field],
    },
    /// A ground-truth case carries an `accepted_divergence`.
    DivergenceOnGroundTruthCase {
        /// The case file name.
        file: String,
        /// The group ID.
        group: String,
    },
    /// A case filter matched no cases or groups.
    FilterMatchedNothing {
        /// The filter text.
        filter: String,
    },
    /// A case fixture file exists but is not a valid PRG.
    InvalidFixture {
        /// The fixture file name.
        file: String,
        /// Reason why the file is invalid.
        reason: String,
    },
}

impl CorpusError {
    /// Returns `true` if the error makes the case list unusable.
    ///
    /// Structural errors include unreadable manifests, invalid or duplicate
    /// names, missing or invalid fixtures, and unknown group references.
    /// Non-structural errors concern `[case.unp64]` and `accepted_divergence`,
    /// which `unpacker_compare_all --update` rewrites.
    ///
    /// # Example
    ///
    /// [`CorpusError::MissingFixture`] is structural.
    /// [`CorpusError::MissingUnp64`] is not structural.
    #[must_use]
    pub fn is_structural(&self) -> bool {
        match self {
            Self::Io { .. }
            | Self::Parse { .. }
            | Self::Serialize { .. }
            | Self::InvalidFileName { .. }
            | Self::DuplicateCase { .. }
            | Self::DuplicateGroup { .. }
            | Self::MissingFixture { .. }
            | Self::MissingGroupOriginal { .. }
            | Self::InvalidGroupOriginal { .. }
            | Self::UnknownGroup { .. }
            | Self::FilterMatchedNothing { .. }
            | Self::InvalidFixture { .. } => true,
            Self::MissingUnp64 { .. }
            | Self::Unp64LenMismatch { .. }
            | Self::OverlapOutsideUnp64Range { .. }
            | Self::FailedUnp64WithoutDivergence { .. }
            | Self::DivergenceKindNeedsFailedUnp64 { .. }
            | Self::DivergenceFieldsMismatch { .. }
            | Self::DivergenceOnGroundTruthCase { .. } => false,
        }
    }

    /// Returns the file, group, or path that the error describes.
    fn subject(&self) -> String {
        match self {
            Self::Io { path, .. } => path.display().to_string(),
            Self::Parse { .. } | Self::Serialize { .. } => String::new(),
            Self::DuplicateGroup { group } => group.clone(),
            Self::FilterMatchedNothing { filter } => filter.clone(),
            Self::InvalidFileName { file }
            | Self::DuplicateCase { file }
            | Self::MissingFixture { file }
            | Self::MissingGroupOriginal { file, .. }
            | Self::InvalidGroupOriginal { file, .. }
            | Self::UnknownGroup { file, .. }
            | Self::MissingUnp64 { file }
            | Self::Unp64LenMismatch { file, .. }
            | Self::OverlapOutsideUnp64Range { file, .. }
            | Self::FailedUnp64WithoutDivergence { file }
            | Self::DivergenceKindNeedsFailedUnp64 { file, .. }
            | Self::DivergenceFieldsMismatch { file, .. }
            | Self::DivergenceOnGroundTruthCase { file, .. }
            | Self::InvalidFixture { file, .. } => file.clone(),
        }
    }

    /// Returns the rank of the error variant for secondary sorting.
    fn rank(&self) -> u8 {
        match self {
            Self::Io { .. } => 0,
            Self::Parse { .. } => 1,
            Self::Serialize { .. } => 2,
            Self::InvalidFileName { .. } => 3,
            Self::DuplicateCase { .. } => 4,
            Self::DuplicateGroup { .. } => 5,
            Self::MissingFixture { .. } => 6,
            Self::MissingGroupOriginal { .. } => 7,
            Self::InvalidGroupOriginal { .. } => 8,
            Self::UnknownGroup { .. } => 9,
            Self::MissingUnp64 { .. } => 10,
            Self::Unp64LenMismatch { .. } => 11,
            Self::OverlapOutsideUnp64Range { .. } => 12,
            Self::FailedUnp64WithoutDivergence { .. } => 13,
            Self::DivergenceKindNeedsFailedUnp64 { .. } => 14,
            Self::DivergenceFieldsMismatch { .. } => 15,
            Self::DivergenceOnGroundTruthCase { .. } => 16,
            Self::FilterMatchedNothing { .. } => 17,
            Self::InvalidFixture { .. } => 18,
        }
    }

    /// Returns the sort key `(subject, rank)` for error lists.
    pub(super) fn sort_key(&self) -> (String, u8) {
        (self.subject(), self.rank())
    }
}

impl fmt::Display for CorpusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Parse { message } => write!(f, "invalid corpus TOML: {message}"),
            Self::Serialize { message } => write!(f, "cannot serialize corpus TOML: {message}"),
            Self::InvalidFileName { file } => write!(
                f,
                "`{file}`: file names must be bare names without path separators"
            ),
            Self::DuplicateCase { file } => write!(f, "`{file}`: listed in more than one [[case]]"),
            Self::DuplicateGroup { group } => {
                write!(f, "group `{group}`: listed in more than one [[group]]")
            }
            Self::MissingFixture { file } => {
                write!(f, "`{file}`: fixture is missing (cases are never skipped)")
            }
            Self::MissingGroupOriginal { group, file } => {
                write!(f, "group `{group}`: original `{file}` is missing")
            }
            Self::InvalidGroupOriginal {
                group,
                file,
                reason,
            } => write!(
                f,
                "group `{group}`: original `{file}` is unusable: {reason}"
            ),
            Self::UnknownGroup { file, group } => {
                write!(f, "`{file}`: references unknown group `{group}`")
            }
            Self::MissingUnp64 { file } => write!(
                f,
                "`{file}`: has no [case.unp64]; record it with `unpacker_compare_all --update`"
            ),
            Self::Unp64LenMismatch {
                file,
                recorded,
                derived,
            } => match derived {
                Some(derived) => write!(
                    f,
                    "`{file}`: [case.unp64] len = {recorded}, but end - start + 1 = {derived}"
                ),
                None => write!(f, "`{file}`: [case.unp64] end is below start"),
            },
            Self::OverlapOutsideUnp64Range {
                file,
                overlap,
                unp64,
            } => write!(
                f,
                "`{file}`: overlap {}-{} is not inside unp64's range {}-{}",
                overlap.0, overlap.1, unp64.0, unp64.1
            ),
            Self::FailedUnp64WithoutDivergence { file } => write!(
                f,
                "`{file}`: unp64 failed, so the case needs an accepted_divergence with kind \
                 `unexplained` or `unp64_missing_opcode`"
            ),
            Self::DivergenceKindNeedsFailedUnp64 { file, kind } => write!(
                f,
                "`{file}`: accepted_divergence kind `{kind}` requires `failed` in [case.unp64]"
            ),
            Self::DivergenceFieldsMismatch {
                file,
                kind,
                required,
            } => write!(
                f,
                "`{file}`: accepted_divergence kind `{kind}` requires fields = [{}]",
                join_fields(required)
            ),
            Self::DivergenceOnGroundTruthCase { file, group } => write!(
                f,
                "`{file}`: ground truth (group `{group}`) decides this case; remove its \
                 accepted_divergence"
            ),
            Self::FilterMatchedNothing { filter } => {
                write!(f, "case filter `{filter}` matches no case or group")
            }
            Self::InvalidFixture { file, reason } => {
                write!(f, "`{file}`: fixture is not a usable PRG: {reason}")
            }
        }
    }
}

impl std::error::Error for CorpusError {}
