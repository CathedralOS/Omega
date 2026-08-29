//! Selection, rule-set, cost-model, decision-log, and ledger identities.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_identity_bundle(
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    expected_rule_set: OptimizationRuleSetIdentity,
    expected_cost_model: TargetCostModelIdentity,
    decisions: &BaselineDecisionLog,
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    if bundle.selections() != selections.identity() {
        return Err(OptimizedAbstractPlanProjectionError::SelectionIdentityMismatch);
    }
    if *psi_selections != selections.for_phase(OptimizationExecutionPhase::Psi) {
        return Err(OptimizedAbstractPlanProjectionError::PsiSelectionProjectionMismatch);
    }
    if bundle.rule_set() != expected_rule_set {
        return Err(OptimizedAbstractPlanProjectionError::RuleSetIdentityMismatch);
    }
    if bundle.target_cost_model() != expected_cost_model {
        return Err(OptimizedAbstractPlanProjectionError::CostModelIdentityMismatch);
    }
    if bundle.decision_log() != Some(decisions.identity) {
        return Err(OptimizedAbstractPlanProjectionError::DecisionLogIdentityMismatch);
    }
    if bundle.workload_profile().is_some() {
        return Err(OptimizedAbstractPlanProjectionError::WorkloadProfileNotSupported);
    }
    if bundle.transformation_ledger() != ledger.identity() {
        return Err(OptimizedAbstractPlanProjectionError::LedgerIdentityMismatch);
    }
    if BaselineDecisionLog::decode(&decisions.encode())
        .map_err(OptimizedAbstractPlanProjectionError::DecisionLogReplay)?
        != *decisions
    {
        return Err(OptimizedAbstractPlanProjectionError::DecisionLogIdentityMismatch);
    }
    Ok(())
}
