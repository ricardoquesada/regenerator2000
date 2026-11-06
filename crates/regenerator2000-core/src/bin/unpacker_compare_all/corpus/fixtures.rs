//! Fixture lookup and byte loading.
//!
//! The integration test and the compare tool use [`DirFixtures`]. Unit tests
//! use an in-memory resolver so validation and ground-truth loading run
//! without file I/O.

use std::path::PathBuf;

use super::CorpusError;

/// Resolves fixture existence and reads fixture bytes.
pub trait FixtureResolver {
    /// Returns `true` if the fixture `file` exists.
    fn exists(&self, file: &str) -> bool;

    /// Returns the bytes of fixture `file`.
    ///
    /// # Errors
    ///
    /// Returns [`CorpusError::MissingFixture`] or [`CorpusError::Io`] when
    /// the fixture cannot be read.
    fn read(&self, file: &str) -> Result<Vec<u8>, CorpusError>;
}

/// Resolves fixtures as plain files inside one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirFixtures {
    root: PathBuf,
}

impl DirFixtures {
    /// Creates a fixture resolver rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl FixtureResolver for DirFixtures {
    fn exists(&self, file: &str) -> bool {
        is_plain_file_name(file) && self.root.join(file).is_file()
    }

    fn read(&self, file: &str) -> Result<Vec<u8>, CorpusError> {
        if !self.exists(file) {
            return Err(CorpusError::MissingFixture {
                file: file.to_string(),
            });
        }
        let path = self.root.join(file);
        std::fs::read(&path).map_err(|e| CorpusError::Io {
            path,
            message: e.to_string(),
        })
    }
}

/// Returns `true` if `name` is a bare file name without path separators.
///
/// # Example
///
/// `"c64_mule.dali.prg"` returns `true`. `""`, `".."`, `"../x.prg"`, and
/// `"dir\\x.prg"` return `false`.
pub(super) fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\'])
}
