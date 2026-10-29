//! One corpus case as observed by the compare tool: R2000's result and
//! unp64's outcome, side by side.

use crate::corpus::{Case, CaseResult, Divergence, classify};
use crate::unp64::Unp64Outcome;

/// R2000's and unp64's results for one case.
#[derive(Debug, Clone)]
pub struct Observation {
    /// The manifest case (a copy, so it can be replaced by its updated form).
    pub case: Case,
    /// What R2000 produced (with the case's own budget).
    pub result: CaseResult,
    /// What unp64 produced.
    pub unp64: Unp64Outcome,
}

impl Observation {
    /// How R2000 and unp64 disagree, by the shared corpus classifier.
    #[must_use]
    pub fn divergence(&self) -> Divergence {
        classify(&self.result, &self.unp64.observation())
    }
}
