//! Admit control cycles only after exact independent verified-source replay.
use super::{AbstractOperationPlan, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError;
#[cfg(test)]
mod tests;
pub(in crate::legalization) fn validate_unit_custody(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    verified_input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::custody();
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
            // The checked component binds the exact verified source graph:
            // Natural ranking evidence and unranked safety custody both admit
            // through the ordinary replay. An unranked source supplies no
            // termination or fixed-work authority.
            if !admitted.contains(&component.id.machine) {
                admitted.push(component.id.machine);
            }
        }
    }
    optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
        unit, &admitted,
    )
    .map_err(|_| invalid)
}
