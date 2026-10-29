//! The error type of the corpus library.

use std::fmt;
use std::path::PathBuf;

use super::kinds::{DivergenceKind, Field, join_fields};
use super::values::HexAddr;

/// A problem with the corpus manifest, its fixtures, or a case selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusError {
    /// A manifest or baseline file could not be read or written.
    Io {
        /// The file that failed.
        path: PathBuf,
        /// The I/O error message.
        message: String,
    },
    /// The TOML text is malformed or violates the schema.
    Parse {
        /// The parser message, including line and key.
        message: String,
    },
    /// Serialization to TOML failed.
    Serialize {
        /// The serializer message.
        message: String,
    },
    /// A file name contains a path separator or is empty.
    InvalidFileName {
        /// The rejected name.
        file: String,
    },
    /// Two cases name the same file.
    DuplicateCase {
        /// The duplicated file.
        file: String,
    },
    /// Two groups share an `id`.
    DuplicateGroup {
        /// The duplicated id.
        group: String,
    },
    /// A case's fixture does not exist.
    MissingFixture {
        /// The missing file.
        file: String,
    },
    /// A group's original does not exist.
    MissingGroupOriginal {
        /// The group.
        group: String,
        /// The missing original.
        file: String,
    },
    /// A group's original is not a usable PRG.
    InvalidGroupOriginal {
        /// The group.
        group: String,
        /// The original file.
        file: String,
        /// Why it is unusable.
        reason: String,
    },
    /// A case references a group that does not exist.
    UnknownGroup {
        /// The case.
        file: String,
        /// The unknown group id.
        group: String,
    },
    /// A case has no `[case.unp64]` table.
    MissingUnp64 {
        /// The case.
        file: String,
    },
    /// The recorded `len` does not equal `end - start + 1`.
    Unp64LenMismatch {
        /// The case.
        file: String,
        /// The recorded length.
        recorded: u32,
        /// The length derived from the range, `None` if `end < start`.
        derived: Option<usize>,
    },
    /// The recorded overlap window is not inside unp64's range.
    OverlapOutsideUnp64Range {
        /// The case.
        file: String,
        /// The recorded window.
        overlap: (HexAddr, HexAddr),
        /// unp64's range.
        unp64: (HexAddr, HexAddr),
    },
    /// unp64 failed but the case has no `unexplained` or
    /// `unp64_missing_opcode` exception.
    FailedUnp64WithoutDivergence {
        /// The case.
        file: String,
    },
    /// The divergence kind requires a failed unp64 record.
    DivergenceKindNeedsFailedUnp64 {
        /// The case.
        file: String,
        /// The kind.
        kind: DivergenceKind,
    },
    /// The divergence lists other fields than its kind requires.
    DivergenceFieldsMismatch {
        /// The case.
        file: String,
        /// The kind.
        kind: DivergenceKind,
        /// The exact list required.
        required: &'static [Field],
    },
    /// A ground-truth case carries an `accepted_divergence`.
    DivergenceOnGroundTruthCase {
        /// The case.
        file: String,
        /// The group.
        group: String,
    },
    /// A case filter selected no case.
    FilterMatchedNothing {
        /// The filter text.
        filter: String,
    },
    /// A case's fixture exists but is not a usable PRG.
    InvalidFixture {
        /// The fixture.
        file: String,
        /// Why it is unusable.
        reason: String,
    },
}

impl CorpusError {
    /// Returns `true` if the error makes the case list itself unusable: the
    /// manifest cannot be read, a name is invalid or duplicated, a fixture or
    /// group original is missing or unusable, or a group reference dangles.
    ///
    /// Every other validation error concerns the recorded oracle data
    /// (`[case.unp64]` and `accepted_divergence`), which
    /// `unpacker_compare_all --update` rewrites. The tool therefore refuses
    /// to run only on structural errors.
    ///
    /// # Example
    ///
    /// [`CorpusError::MissingFixture`] is structural;
    /// [`CorpusError::MissingUnp64`] is not.
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

    /// The file, group or path the error is about.
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

    /// Stable rank of the error kind, used as the secondary sort key.
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

    /// Sort key for error lists: subject first, then error kind.
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
