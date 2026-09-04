use super::*;

#[allow(clippy::too_many_arguments)]
pub fn validate_pre_physical_optimization_manifest(
    candidate: &PrePhysicalOptimizationManifest,
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
    projection: ValidatedOptimizedAbstractPlanProjection,
) -> Result<ValidatedPrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError> {
    validate_joins(
        selections,
        psi_selections,
        budget_per_pass,
        usage,
        decisions,
        pass_manifests,
        ledger,
        bundle,
        projection,
    )?;
    let mut expected = projection::expected_record(
        input,
        final_unit,
        selections,
        psi_selections,
        budget_per_pass,
        usage,
        decisions,
        pass_manifests,
        ledger,
        bundle,
        projection,
    )?;
    expected.identity = expected.recomputed_identity();
    if candidate != &expected || candidate.identity != candidate.recomputed_identity() {
        return Err(PrePhysicalOptimizationManifestError::ContentMismatch);
    }
    Ok(ValidatedPrePhysicalOptimizationManifest {
        record: candidate.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_joins(
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
    projection: ValidatedOptimizedAbstractPlanProjection,
) -> Result<(), PrePhysicalOptimizationManifestError> {
    if bundle.selections() != selections.identity()
        || projection.selections() != selections.identity()
    {
        return Err(PrePhysicalOptimizationManifestError::SelectionMismatch);
    }
    if *psi_selections != selections.for_phase(OptimizationExecutionPhase::Psi)
        || projection.psi_selections() != psi_selections.identity()
    {
        return Err(PrePhysicalOptimizationManifestError::SelectionMismatch);
    }
    if bundle.decision_log() != Some(decisions.identity) {
        return Err(PrePhysicalOptimizationManifestError::DecisionLogMismatch);
    }
    if BaselineDecisionLog::decode(&decisions.encode())
        .ok()
        .as_ref()
        != Some(decisions)
    {
        return Err(PrePhysicalOptimizationManifestError::DecisionLogMismatch);
    }
    if bundle.transformation_ledger() != ledger.identity()
        || projection.ledger() != ledger.identity()
        || projection.bundle() != bundle.identity()
        || projection.initial_unit() != ledger.input()
        || projection.final_unit() != ledger.output()
        || projection.psi() != ledger.psi()
        || projection.fuel_schedule() != ledger.fuel_schedule()
    {
        return Err(PrePhysicalOptimizationManifestError::LedgerMismatch);
    }
    crate::projection::validate_manifests(pass_manifests, bundle.rule_set(), ledger)
        .map_err(|_| PrePhysicalOptimizationManifestError::PassRevisionMismatch)?;
    let mut revision = ledger.input();
    let mut aggregate = OptimizationWorkUsage::default();
    for pass in pass_manifests {
        if OptimizationPassManifestRecord::decode(&pass.encode())
            .ok()
            .as_ref()
            != Some(pass)
        {
            return Err(PrePhysicalOptimizationManifestError::PassManifestCodecMismatch);
        }
        if pass.input() != revision {
            return Err(PrePhysicalOptimizationManifestError::PassRevisionMismatch);
        }
        revision = pass.output();
        if !pass.work_usage().within(budget_per_pass) {
            return Err(PrePhysicalOptimizationManifestError::WorkBudgetExceeded);
        }
        aggregate = add_usage(aggregate, pass.work_usage())?;
    }
    if revision != ledger.output() {
        return Err(PrePhysicalOptimizationManifestError::PassRevisionMismatch);
    }
    if aggregate != usage {
        return Err(PrePhysicalOptimizationManifestError::WorkUsageMismatch);
    }
    Ok(())
}

fn add_usage(
    left: OptimizationWorkUsage,
    right: OptimizationWorkUsage,
) -> Result<OptimizationWorkUsage, PrePhysicalOptimizationManifestError> {
    Ok(OptimizationWorkUsage {
        rule_evaluations: left
            .rule_evaluations
            .checked_add(right.rule_evaluations)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
        candidates: left
            .candidates
            .checked_add(right.candidates)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
        validation_steps: left
            .validation_steps
            .checked_add(right.validation_steps)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
        commits: left
            .commits
            .checked_add(right.commits)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
        iterations: left
            .iterations
            .checked_add(right.iterations)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
    })
}
