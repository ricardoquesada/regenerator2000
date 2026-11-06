//! One corpus case observed by the compare tool: the R2000 result and the
//! `unp64` outcome side by side.

use crate::corpus::{Case, CaseResult, Divergence, classify};
use crate::unp64::Unp64Outcome;

/// R2000 and `unp64` results for one case.
#[derive(Debug, Clone)]
pub struct Observation {
    /// The manifest case. `--update` replaces this copy with the updated case.
    pub case: Case,
    /// The R2000 result with the case budget.
    pub result: CaseResult,
    /// The `unp64` outcome.
    pub unp64: Unp64Outcome,
}

impl Observation {
    /// Classifies how R2000 and `unp64` disagree.
    #[must_use]
    pub fn divergence(&self) -> Divergence {
        classify(&self.result, &self.unp64.observation())
    }
}
