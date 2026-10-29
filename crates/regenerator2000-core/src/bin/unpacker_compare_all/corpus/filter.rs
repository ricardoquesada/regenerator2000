//! Case selection while debugging (`CORPUS_FILTER`).
//!
//! The filter is built from the variable's *value*; reading the environment
//! is the integration test's job, so this module stays pure (ruling R10).

use std::collections::BTreeSet;

use super::{Case, Corpus, CorpusError};

/// Selects a subset of cases while debugging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusFilter {
    /// Every case.
    All,
    /// Cases whose file name, or whose group id, contains the text.
    Substring(String),
}

impl CorpusFilter {
    /// Builds a filter from the value of the filter environment variable.
    /// A missing, empty or whitespace-only value selects everything.
    ///
    /// # Example
    ///
    /// `from_env_value(Some("mule"))` gives `Substring("mule")`;
    /// `from_env_value(Some("  "))` gives `All`.
    #[must_use]
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            None | Some("") => Self::All,
            Some(text) => Self::Substring(text.to_string()),
        }
    }

    /// Returns `true` if the filter restricts the selection.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Substring(_))
    }

    /// Returns the selected cases, sorted by file.
    ///
    /// A group is selected as a whole when its id matches or any member's
    /// file matches, so consensus always sees the complete group.
    ///
    /// # Errors
    ///
    /// Returns [`CorpusError::FilterMatchedNothing`] if an active filter
    /// selects no case.
    pub fn select<'c>(&self, corpus: &'c Corpus) -> Result<Vec<&'c Case>, CorpusError> {
        let Self::Substring(text) = self else {
            let mut all: Vec<&Case> = corpus.cases.iter().collect();
            all.sort_by(|a, b| a.file.cmp(&b.file));
            return Ok(all);
        };

        let file_matches = |case: &Case| case.file.contains(text.as_str());
        let groups: BTreeSet<&str> = corpus
            .groups
            .iter()
            .filter(|group| {
                group.id.contains(text.as_str()) || corpus.members(&group.id).any(file_matches)
            })
            .map(|group| group.id.as_str())
            .collect();

        let mut selected: Vec<&Case> = corpus
            .cases
            .iter()
            .filter(|case| {
                file_matches(case) || case.group.as_deref().is_some_and(|id| groups.contains(id))
            })
            .collect();
        if selected.is_empty() {
            return Err(CorpusError::FilterMatchedNothing {
                filter: text.clone(),
            });
        }
        selected.sort_by(|a, b| a.file.cmp(&b.file));
        Ok(selected)
    }
}
