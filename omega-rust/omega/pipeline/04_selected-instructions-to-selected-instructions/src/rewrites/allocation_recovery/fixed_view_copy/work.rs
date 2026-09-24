//! Checked work-domain composition shared by production and independent replay.

use optimization_core::OptimizationWorkUsage;

use crate::FixedViewCopyError;

pub(super) fn combined_usage(
    evidence: OptimizationWorkUsage,
    transformation: OptimizationWorkUsage,
) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
    Ok(OptimizationWorkUsage {
        rule_evaluations: add(evidence.rule_evaluations, transformation.rule_evaluations)?,
        candidates: add(evidence.candidates, transformation.candidates)?,
        validation_steps: add(evidence.validation_steps, transformation.validation_steps)?,
        commits: add(evidence.commits, transformation.commits)?,
        iterations: add(evidence.iterations, transformation.iterations)?,
    })
}

fn add(left: u64, right: u64) -> Result<u64, FixedViewCopyError> {
    left.checked_add(right)
        .ok_or(FixedViewCopyError::WorkOverflow)
}
