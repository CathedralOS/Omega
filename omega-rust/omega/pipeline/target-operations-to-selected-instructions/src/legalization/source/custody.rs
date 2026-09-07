use super::shared::*;

pub(super) fn validate_source_custody(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    if crate::legalization::scalar_graph_input::validate_unit_custody(target, abstract_plan, unit)
        .is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(())
}
