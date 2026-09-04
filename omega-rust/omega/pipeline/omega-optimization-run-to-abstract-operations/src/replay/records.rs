//! Transformation-ledger, pass-manifest, and aggregate-usage replay.

use omega_abstract_operations_optimizer::OptimizationRun;
use omega_optimization_core::{
    OptimizationPassManifestRecord, OptimizationRuleSetIdentity, OptimizationWorkUsage,
};
use omega_optimization_unit::PsiTransformationRecord;

use super::work_usage;
use crate::OptimizedAbstractProjectionError;

pub(super) fn validate(
    run: &OptimizationRun,
    expected_rule_set: OptimizationRuleSetIdentity,
) -> Result<(), OptimizedAbstractProjectionError> {
    let expected_records = run
        .commits()
        .iter()
        .map(|commit| PsiTransformationRecord {
            rule: commit.rule,
            candidate: commit.candidate,
            validator: commit.validator,
            input: commit.input,
            output: commit.output,
            pruned_machines: commit.pruned_machines.clone(),
            provenance: commit.provenance.clone(),
        })
        .collect::<Vec<_>>();
    if run.transformation_ledger().records() != expected_records {
        return Err(OptimizedAbstractProjectionError::LedgerCommitMismatch);
    }
    let flattened_rules = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.ordered_rules().iter().copied())
        .collect::<Vec<_>>();
    if OptimizationRuleSetIdentity::from_ordered_rules(&flattened_rules).ok()
        != Some(expected_rule_set)
    {
        return Err(OptimizedAbstractProjectionError::ManifestUsageMismatch);
    }
    let mut manifest_usage = OptimizationWorkUsage::default();
    for manifest in run.pass_manifests() {
        manifest_usage = add_work_usage(manifest_usage, manifest.work_usage())
            .ok_or(OptimizedAbstractProjectionError::ManifestUsageMismatch)?;
        OptimizationPassManifestRecord::decode(&manifest.encode())
            .map_err(|_| OptimizedAbstractProjectionError::ManifestUsageMismatch)?;
    }
    if manifest_usage != work_usage(run.usage()) {
        return Err(OptimizedAbstractProjectionError::ManifestUsageMismatch);
    }
    Ok(())
}

fn add_work_usage(
    left: OptimizationWorkUsage,
    right: OptimizationWorkUsage,
) -> Option<OptimizationWorkUsage> {
    Some(OptimizationWorkUsage {
        rule_evaluations: left.rule_evaluations.checked_add(right.rule_evaluations)?,
        candidates: left.candidates.checked_add(right.candidates)?,
        validation_steps: left.validation_steps.checked_add(right.validation_steps)?,
        commits: left.commits.checked_add(right.commits)?,
        iterations: left.iterations.checked_add(right.iterations)?,
    })
}
