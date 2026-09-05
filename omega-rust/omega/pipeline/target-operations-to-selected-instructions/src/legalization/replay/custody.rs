use super::shared::*;

pub(super) fn validate_replay_custody(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
) -> Result<(), LegalizationError> {
    if optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
    {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed.psi != target.psi
        || proposed.optimization_unit != unit.identity
        || proposed.fuel_schedule != unit.fuel_schedule
        || proposed.target != target.target
        || proposed.entry != target.entry
        || proposed.functions.len()
            + proposed.unit_functions.len()
            + proposed.scalar_call_unit_functions.len()
            + proposed.structural_unit_functions.len()
            + proposed.projected_structural_call_returns.len() * 2
            != target.functions.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}
