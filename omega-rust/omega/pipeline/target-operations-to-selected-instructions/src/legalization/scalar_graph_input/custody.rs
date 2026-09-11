//! Admit control cycles only after exact independent verified-source replay.
use super::*;
#[cfg(test)]
mod tests;
pub(in crate::legalization) fn validate_unit_custody(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    verified_input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if abstract_plan.functions.len() != target.functions.len() {
        return Err(invalid);
    }
    for (source, function) in abstract_plan.functions.iter().zip(&target.functions) {
        if source.machine != function.machine {
            return Err(invalid);
        }
    }
    let mut admitted = Vec::new();
    if let Some(input) = verified_input {
        let validated = abstract_operations_to_abstract_operations::validation::validate_transformed_psi_cycle_components(input, unit)
            .map_err(|_| invalid.clone())?;
        for component in validated.components() {
            if input.context().module().machines.iter().any(|machine| {
                machine.id == component.id.machine
                    && matches!(
                        machine.ranked_scc,
                        None | Some(terminal_psi::TerminalRankedScc::Natural(_))
                    )
            }) && !admitted.contains(&component.id.machine)
            {
                // The checked component binds the exact verified source graph.
                // An unranked source supplies safety custody, not a rank or work bound.
                admitted.push(component.id.machine);
            }
        }
    }
    optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
        unit, &admitted,
    )
    .map_err(|_| invalid)
}
