//! Optimizer module role: executable entrance. All-candidate decision custody, descended by retained declaration, manifest,
//! and baseline-policy responsibility.

mod baseline;
mod declarations;
mod manifests;

use crate::{OptimizationRun, PsiPassStep};

use super::commits::ReplayedCommits;
use crate::OptimizedAbstractProjectionError;

pub(super) fn validate(
    run: &OptimizationRun,
    steps: &[PsiPassStep],
    commits: &ReplayedCommits,
) -> Result<(), OptimizedAbstractProjectionError> {
    manifests::validate(run, steps)?;
    declarations::validate(run, steps, commits)?;
    baseline::validate(run)
}
