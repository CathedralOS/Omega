//! All-candidate decision custody, descended by retained declaration, manifest,
//! and baseline-policy responsibility.

mod baseline;
mod declarations;
mod manifests;

use omega_psi_optimizer::{OptimizationRun, OrderedRuleRegistry};

use super::commits::ReplayedCommits;
use crate::OptimizedAbstractProjectionError;

pub(super) fn validate(
    run: &OptimizationRun,
    registries: &[OrderedRuleRegistry],
    commits: &ReplayedCommits,
) -> Result<(), OptimizedAbstractProjectionError> {
    manifests::validate(run, registries)?;
    declarations::validate(run, registries, commits)?;
    baseline::validate(run)
}
