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
    // All ordinary scalar bodies use the graph, including when a caller
    // hand-assembles target records. Atomic structural-family admission must
    // not reopen a retired scalar return or expression-tree route.
    if abstract_plan.functions.len() != target.functions.len() {
        return Err(invalid);
    }
    for (source, function) in abstract_plan.functions.iter().zip(&target.functions) {
        if source.machine != function.machine
            || (source.result.scalar().is_some()
                && !matches!(
                    function.operation,
                    TargetOperation::ControlGraph(_) | TargetOperation::RankedU32Countdown(_)
                ))
        {
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
    for function in &target.functions {
        if !matches!(function.operation, TargetOperation::RankedU32Countdown(_)) {
            continue;
        }
        let abstract_matches = abstract_plan
            .functions
            .iter()
            .filter(|source| source.machine == function.machine)
            .collect::<Vec<_>>();
        let optimized_matches = unit
            .functions
            .iter()
            .filter(|source| source.machine == function.machine)
            .collect::<Vec<_>>();
        let ([abstracted], [optimized]) =
            (abstract_matches.as_slice(), optimized_matches.as_slice())
        else {
            return Err(invalid);
        };
        // This is the input-only rank/proof/ABI and complete current graph join,
        // not a producer or a self-issued cycle assertion.
        super::match_input(function, abstracted, optimized, target, abstract_plan, unit)?;
        if admitted.contains(&function.machine) {
            return Err(invalid);
        }
        admitted.push(function.machine);
    }
    optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
        unit, &admitted,
    )
    .map_err(|_| invalid)
}
